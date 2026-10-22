//! The retained-aware materializer of the demand closure (Go:
//! `semantic_retained.go`): files outside every accepted update's affected set
//! whose demand lists hash identically reuse their previous contributions;
//! only changed files run against the compiler.
//!
//! A contribution is stored only when every identity it carries is durable
//! (the source-fact memo's rule), so retained facts stay resolvable and wire
//! output is byte-identical to a fresh whole-batch run. Descriptor suppression
//! keeps whole-batch semantics via the structural-accessor union: recomputed
//! files always run under the exact current union, and when the union differs
//! from the previous generation's, the retained files whose descriptor demands
//! touch the difference are refreshed under it.

use crate::backend::BackendError;
use crate::closure::{ClosureRetention, DemandClosure, SemanticDemandStages};
use crate::context::Context;
use crate::contributions::prepare_retained_contribution;
use crate::demands::DemandGroup;
use crate::gocompat::{clean, quote};
use crate::model::{
    SemanticDemandRun, SemanticDemandRunResult, SemanticScope, SymbolId, durable_symbol_id,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use typefacts::producer::{EntityRows, ProducerTable, rust_owned_transport_manifest};
use typefacts::v3::EntityDemand;
use typefacts::{AsyncFunctionFact, FileFact, SourceDigest, TypeDescriptor};

/// Digests one file's demand run. The hash only ever compares runs within one
/// process (retained state never crosses processes), so a fixed-key hasher is
/// sufficient and fast. Paths are excluded: every demand in a run belongs to
/// the group's file, and cross-file query locations do not occur (Go:
/// `demandListHash`).
#[must_use]
pub fn demand_list_hash(demands: &[EntityDemand]) -> u64 {
    let mut digest = DefaultHasher::new();
    for demand in demands {
        demand.location.start_byte.hash(&mut digest);
        demand.location.end_byte.hash(&mut digest);
        match &demand.query_location {
            Some(query) => {
                1_u8.hash(&mut digest);
                query.start_byte.hash(&mut digest);
                query.end_byte.hash(&mut digest);
            }
            None => 0_u8.hash(&mut digest),
        }
        [
            demand.symbol,
            demand.type_descriptor,
            demand.resolved_call,
            demand.callability,
            demand.constructability,
            demand.runtime_value_domain,
            demand.primitive_value_domain,
            demand.primitive_literal_candidates,
            demand.parameter_object_shape,
            demand.call_result_domain,
            demand.constant_value,
            demand.array_shape,
            demand.tuple_shape,
            demand.library_types,
            demand.references,
            demand.r#async,
            demand.structural_accessor,
            demand.reference_space,
            demand.runtime_identity,
        ]
        .hash(&mut digest);
    }
    digest.finish()
}

/// Every identity in the facts is durable (Go: `durableAsyncFunctions`).
#[must_use]
pub fn durable_async_functions(facts: &[AsyncFunctionFact]) -> bool {
    facts
        .iter()
        .all(|fact| durable_symbol_id(&fact.symbol) && durable_symbol_id(&fact.target))
}

/// The async facts for the demands that asked for them, keyed by clean path.
/// The backend is always asked, even for an empty demand list (Go:
/// `asyncFunctionsForDemands`).
fn async_functions_for_demands(
    closure: &mut DemandClosure,
    ctx: &Context,
    demands: &[EntityDemand],
) -> Result<HashMap<String, Vec<AsyncFunctionFact>>, BackendError> {
    let locations: Vec<_> = demands
        .iter()
        .filter(|demand| demand.r#async)
        .map(|demand| {
            let mut location = demand.location.clone();
            location.path = clean(&location.path).into();
            location
        })
        .collect();
    let mut by_path: HashMap<String, Vec<AsyncFunctionFact>> = HashMap::new();
    // Demand-shaped async lookup only: the per-file whole-table walk it
    // replaced served the retired seed-based materializer.
    for mut fact in closure.backend.async_functions_at(ctx, &locations)? {
        let path = clean(&fact.expression.path);
        fact.expression.path = path.as_str().into();
        by_path.entry(path).or_default().push(fact);
    }
    Ok(by_path)
}

fn cancelled(ctx: &Context) -> Result<(), BackendError> {
    ctx.err()
        .map_or(Ok(()), |reason| Err(BackendError(reason.to_owned())))
}

/// Merges an aligned set of results into their groups (the `rebuildContributions`
/// closure of Go's materializer).
fn rebuild_contributions(
    groups: &mut [DemandGroup],
    results: Vec<SemanticDemandRunResult>,
    indices: &[usize],
) -> Result<(), BackendError> {
    if results.len() != indices.len() {
        return Err(BackendError(format!(
            "semantic demand run results = {}, want {}",
            results.len(),
            indices.len()
        )));
    }
    for (result, &index) in results.into_iter().zip(indices) {
        let group = &mut groups[index];
        let demands = group
            .demands
            .clone()
            .unwrap_or_else(|| Arc::from(Vec::new()));
        group.contribution = Some(
            prepare_retained_contribution(&group.path, group.hash, &demands, result)
                .map_err(BackendError)?,
        );
    }
    Ok(())
}

impl DemandClosure {
    /// The digests the table retains instead of source bodies, checked to be
    /// clean and strictly path-ordered (Go: `semanticSourceDigests`).
    fn semantic_source_digests(
        &mut self,
        ctx: &Context,
    ) -> Result<Vec<SourceDigest>, BackendError> {
        let digests = self.backend.source_digests(ctx)?;
        for (index, digest) in digests.iter().enumerate() {
            let path = digest.path.as_ref();
            if path.is_empty() || clean(path) != path {
                return Err(BackendError(format!(
                    "source digest path {} is not clean and non-empty",
                    quote(path)
                )));
            }
            if index != 0 && digests[index - 1].path.as_ref() >= path {
                return Err(BackendError(format!(
                    "source digests are not strictly path-ordered at {}",
                    quote(path)
                )));
            }
        }
        Ok(digests)
    }

    /// Materializes one generation's table from canonical, path-ordered groups
    /// (Go: `materializeSemanticDemandRetained`). It mutates the groups (their
    /// hashes and contributions) and the closure's retained state only past
    /// the point where nothing can fail; a failure leaves the installed
    /// contributions and the suppression union as they were.
    pub(crate) fn materialize_semantic_demand_retained(
        &mut self,
        ctx: &Context,
        groups: &mut [DemandGroup],
        generation: u64,
    ) -> Result<(ProducerTable, usize, SemanticDemandStages, ClosureRetention), BackendError> {
        let mut retention = ClosureRetention::default();
        let result = self.materialize_groups(ctx, groups, generation, &mut retention);
        for group in groups.iter_mut() {
            group.release_expanded();
        }
        let (table, stages) = result?;
        Ok((table, 0, stages, retention))
    }

    #[allow(clippy::too_many_lines)]
    fn materialize_groups(
        &mut self,
        ctx: &Context,
        groups: &mut [DemandGroup],
        generation: u64,
        retention: &mut ClosureRetention,
    ) -> Result<(ProducerTable, SemanticDemandStages), BackendError> {
        let source_digests = self.semantic_source_digests(ctx)?;
        let mut table = ProducerTable::new(generation, "semantic-demand");
        table.sources.clone_from(&source_digests);

        // The async runs are rebuilt every generation; a group's run holds only
        // the rows that asked for async.
        let mut async_groups: Vec<(String, Vec<EntityDemand>)> = Vec::new();
        for group in groups.iter() {
            let mut demands = Vec::new();
            group
                .append_async_demands(&mut demands)
                .map_err(BackendError)?;
            if !demands.is_empty() {
                async_groups.push((group.path.clone(), demands));
            }
        }
        let mut stages = SemanticDemandStages::default();
        let started = Instant::now();
        let refresh_all_async =
            self.async_files.is_none() || self.transport_changed_paths.is_none();
        let mut refresh_async_paths: HashSet<String> = HashSet::new();
        let mut refresh_async_demands: Vec<EntityDemand> = Vec::new();
        let mut async_by_path: HashMap<String, Vec<AsyncFunctionFact>> =
            HashMap::with_capacity(async_groups.len());
        for (path, demands) in &async_groups {
            let changed = self
                .transport_changed_paths
                .as_ref()
                .is_some_and(|paths| paths.contains(path));
            let cached = self.async_files.as_ref().and_then(|files| files.get(path));
            match cached {
                Some(cached) if !refresh_all_async && !changed => {
                    async_by_path.insert(path.clone(), cached.clone());
                    retention.retained_async_files += 1;
                }
                _ => {
                    refresh_async_paths.insert(path.clone());
                    refresh_async_demands.extend(demands.iter().cloned());
                }
            }
        }
        let mut refreshed = async_functions_for_demands(self, ctx, &refresh_async_demands)?;
        let cross_path_async = refreshed
            .keys()
            .any(|path| !refresh_async_paths.contains(path));
        if cross_path_async && !refresh_all_async {
            refresh_async_paths = HashSet::with_capacity(async_groups.len());
            refresh_async_demands = Vec::new();
            async_by_path = HashMap::with_capacity(async_groups.len());
            retention.retained_async_files = 0;
            for (path, demands) in &async_groups {
                refresh_async_paths.insert(path.clone());
                refresh_async_demands.extend(demands.iter().cloned());
            }
            refreshed = async_functions_for_demands(self, ctx, &refresh_async_demands)?;
        }
        for path in &refresh_async_paths {
            async_by_path.insert(path.clone(), Vec::new());
        }
        for (path, facts) in refreshed {
            async_by_path.insert(path, facts);
        }
        retention.recomputed_async_files = refresh_async_paths.len();
        let mut next_async_files: HashMap<String, Vec<AsyncFunctionFact>> =
            HashMap::with_capacity(async_groups.len());
        let mut cacheable_async = true;
        for (path, _) in &async_groups {
            let facts = async_by_path.get(path).cloned().unwrap_or_default();
            if !durable_async_functions(&facts) {
                cacheable_async = false;
                continue;
            }
            next_async_files.insert(path.clone(), facts);
        }
        if cross_path_async {
            cacheable_async = false;
        }
        self.async_files = cacheable_async.then_some(next_async_files);
        for source in &source_digests {
            cancelled(ctx)?;
            let facts = async_by_path
                .get(source.path.as_ref())
                .cloned()
                .unwrap_or_default();
            table.files.push(FileFact {
                path: source.path.clone(),
                calls: Arc::from(Vec::new()),
                bindings: Arc::from(Vec::new()),
                functions: Arc::from(Vec::new()),
                async_functions: facts.into(),
            });
        }
        stages.async_stage = started.elapsed();

        let started = Instant::now();
        let mut union: HashSet<SymbolId> = HashSet::new();
        let mut descriptor_seed: HashMap<SymbolId, Arc<TypeDescriptor>> = HashMap::new();
        let mut changed: Vec<usize> = Vec::new();
        for (index, group) in groups.iter_mut().enumerate() {
            let contribution = self.retained.get(&group.path);
            let path_changed = self
                .transport_changed_paths
                .as_ref()
                .is_some_and(|paths| paths.contains(&group.path));
            // Update and demand-delta handling already name every path whose
            // demand run may differ. Hash unchanged runs only when no exact
            // changed-path set is available (initial/full materialization).
            match &contribution {
                Some(retained) if self.transport_changed_paths.is_some() && !path_changed => {
                    group.hash = retained.borrow().demand_hash;
                }
                _ => {
                    group.ensure_expanded().map_err(BackendError)?;
                    group.hash = demand_list_hash(group.demands.as_deref().unwrap_or(&[]));
                }
            }
            if let Some(retained) = contribution
                && retained.borrow().demand_hash == group.hash
            {
                {
                    let contribution = retained.borrow();
                    union.extend(contribution.structural.iter().cloned());
                    // Batch-wide first-wins descriptor dedup: descriptors the
                    // retained files already carry are what a whole-batch run
                    // would have cached before reaching the recomputed demands.
                    if let Some(entities) = &contribution.entities {
                        for entity in entities.iter() {
                            if !entity.symbol.is_empty()
                                && let Some(descriptor) = &entity.type_descriptor
                            {
                                descriptor_seed
                                    .entry(entity.symbol.clone())
                                    .or_insert_with(|| Arc::clone(descriptor));
                            }
                        }
                    } else {
                        for kept in &contribution.descriptors {
                            descriptor_seed
                                .entry(kept.symbol.clone())
                                .or_insert_with(|| Arc::clone(&kept.descriptor));
                        }
                    }
                }
                group.contribution = Some(retained);
                continue;
            }
            changed.push(index);
        }
        if !changed.is_empty() {
            let demands: Vec<Arc<[EntityDemand]>> = changed
                .iter()
                .map(|&index| {
                    groups[index]
                        .demands
                        .clone()
                        .unwrap_or_else(|| Arc::from(Vec::new()))
                })
                .collect();
            let runs: Vec<SemanticDemandRun<'_>> = changed
                .iter()
                .zip(&demands)
                .map(|(&index, demands)| SemanticDemandRun {
                    path: &groups[index].path,
                    demands,
                })
                .collect();
            let results = self.backend.semantic_demand_runs(
                ctx,
                &runs,
                &SemanticScope {
                    suppression: &union,
                    descriptor_seed: &descriptor_seed,
                },
            )?;
            drop(runs);
            rebuild_contributions(groups, results, &changed)?;
            for &index in &changed {
                if let Some(contribution) = &groups[index].contribution {
                    for symbol in &contribution.borrow().structural {
                        // Non-durable structural symbols re-mint each
                        // generation and can only ever suppress entities in
                        // files that recompute alongside them; comparing them
                        // across generations would force a spurious
                        // whole-batch recompute.
                        if durable_symbol_id(symbol) {
                            union.insert(symbol.clone());
                        }
                    }
                }
            }
        }
        // Recomputed files always run under the exact current union (injected
        // retained structural symbols plus their own batch prefetch), so a
        // union change can only invalidate RETAINED contributions, and only
        // those whose descriptor demands touch a symbol whose suppression
        // status flipped.
        let suppression_delta: Option<HashSet<SymbolId>> = self
            .last_suppression
            .as_ref()
            .filter(|last| **last != union)
            .map(|last| union.symmetric_difference(last).cloned().collect());
        if let Some(delta) = suppression_delta {
            let mut refresh_set: BTreeSet<usize> = BTreeSet::new();
            for symbol in &delta {
                self.retained.range_descriptor_users(symbol, |path| {
                    let index = groups.partition_point(|group| group.path.as_str() < path);
                    if index == groups.len() || groups[index].path != path {
                        return;
                    }
                    let current = self.retained.get(path);
                    let same = match (&groups[index].contribution, &current) {
                        (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                        (None, None) => true,
                        _ => false,
                    };
                    if same {
                        refresh_set.insert(index);
                    }
                });
            }
            let refresh: Vec<usize> = refresh_set.into_iter().collect();
            for &index in &refresh {
                groups[index].ensure_expanded().map_err(BackendError)?;
            }
            if !refresh.is_empty() {
                retention.suppression_recompute = true;
                let demands: Vec<Arc<[EntityDemand]>> = refresh
                    .iter()
                    .map(|&index| {
                        groups[index]
                            .demands
                            .clone()
                            .unwrap_or_else(|| Arc::from(Vec::new()))
                    })
                    .collect();
                let runs: Vec<SemanticDemandRun<'_>> = refresh
                    .iter()
                    .zip(&demands)
                    .map(|(&index, demands)| SemanticDemandRun {
                        path: &groups[index].path,
                        demands,
                    })
                    .collect();
                let no_seed = HashMap::new();
                let results = self.backend.semantic_demand_runs(
                    ctx,
                    &runs,
                    &SemanticScope {
                        suppression: &union,
                        descriptor_seed: &no_seed,
                    },
                )?;
                drop(runs);
                rebuild_contributions(groups, results, &refresh)?;
                changed.extend(refresh);
            }
        }
        // Every recomputed group's rows may differ from the preceding
        // generation, and the transport manifest is built from changed paths
        // alone, so all of them must be named here or the delta silently omits
        // their rows. There are more reasons to recompute than an edit: a
        // changed demand run, a descriptor refresh under a shifted suppression
        // union, or a file holding non-durable identities, which are re-minted
        // every generation and so can never be retained. Naming them is what
        // lets such a file take the delta path instead of forcing a
        // whole-table pack.
        let manifest_changed_paths: Option<HashSet<String>> = if changed.is_empty() {
            self.transport_changed_paths.clone()
        } else {
            let mut paths = self.transport_changed_paths.clone().unwrap_or_default();
            paths.extend(changed.iter().map(|&index| groups[index].path.clone()));
            Some(paths)
        };

        // The source-fact memo's rule (ADR 0001): a contribution is stored only
        // when every identity it carries is durable. Files holding
        // generation-scoped counter ids recompute every generation, all of them
        // together, in canonical order, so their minted counters match a fresh
        // whole-batch run.
        for group in groups.iter() {
            if group
                .contribution
                .as_ref()
                .is_some_and(|contribution| !contribution.borrow().durable)
            {
                retention.non_durable_files += 1;
            }
        }
        retention.retained_files = groups.len().saturating_sub(changed.len());
        retention.recomputed_files = changed.len();
        stages.demand = started.elapsed();

        let started = Instant::now();
        // Entity rows are handed to the packed transition as per-path windows
        // onto the contributions' own allocations. The client owns the
        // expanded retained table and performs symbol closure; the producer
        // retains only compact demand contributions.
        let mut runs = Vec::with_capacity(groups.len());
        for group in groups.iter() {
            if let Some(contribution) = &group.contribution
                && let Some(entities) = &contribution.borrow().entities
                && !entities.is_empty()
            {
                runs.push(Arc::clone(entities));
            }
        }
        table.entities = EntityRows::Runs(runs);
        stages.assembly = started.elapsed();
        self.next_table_state_id += 1;
        if self.next_table_state_id == 0 {
            self.next_table_state_id += 1;
        }
        table.state_id = self.next_table_state_id;
        table.transport = rust_owned_transport_manifest(
            self.previous_table.as_ref(),
            manifest_changed_paths.as_ref(),
        );
        self.retained.commit(groups);
        self.last_suppression = Some(union);
        self.previous_table = None;
        self.transport_changed_paths = None;
        Ok((table, stages))
    }
}
