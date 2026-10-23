//! The materializer of the demand closure (Go: `semantic_retained.go`).
//!
//! This producer reuses no demand contribution across an update or across a
//! change of the demand set (ADR 0364): every table begins by discarding what
//! earlier analysis left (`DemandClosure::discard_derived_state`), so each file
//! is resolved against the compiler, in canonical order after the check-first
//! pass (ADR 0367), and ids are minted as a fresh program mints them. The
//! contributions are published in the store only so the packed transition can
//! release their entity rows. Only binder-level answers (reference spans,
//! entity symbols and symbol declarations resolved without types) are reused,
//! by the backend, under recorded dependencies (ADR 0369).

use crate::backend::BackendError;
use crate::closure::{ClosureRetention, DemandClosure, SemanticDemandStages};
use crate::context::Context;
use crate::contributions::prepare_retained_contribution;
use crate::demands::DemandGroup;
use crate::gocompat::{clean, quote};
use crate::model::{SemanticDemandRun, SemanticDemandRunResult};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use typefacts::producer::{EntityRows, ProducerTable, rust_owned_transport_manifest};
use typefacts::v3::EntityDemand;
use typefacts::{AsyncFunctionFact, FileFact, SourceDigest};

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
) -> Result<(), BackendError> {
    if results.len() != groups.len() {
        return Err(BackendError(format!(
            "semantic demand run results = {}, want {}",
            results.len(),
            groups.len()
        )));
    }
    for (result, group) in results.into_iter().zip(groups.iter_mut()) {
        let demands = group
            .demands
            .clone()
            .unwrap_or_else(|| Arc::from(Vec::new()));
        group.contribution = Some(
            prepare_retained_contribution(&group.path, &demands, result).map_err(BackendError)?,
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
    /// contributions) and the closure's retained state only past the point
    /// where nothing can fail.
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

        // Check-first (ADR 0367): every demanded file is fully checked, in
        // path order (the groups are path-ordered), before any demand of the
        // table is answered, so no answer depends on demand, batch or request
        // order.
        // Reuse counts are this table's own (ADR 0369).
        let _ = self.backend.binder_retention();
        let demanded: Vec<&str> = groups.iter().map(|group| group.path.as_str()).collect();
        let started = Instant::now();
        self.backend.check_demanded_files(ctx, &demanded)?;
        if let Some(trace) = &self.trace {
            trace.stage("analyze-check-first", started.elapsed());
        }

        // The async facts are rebuilt every table; a group's run holds only
        // the rows that asked for async.
        let mut stages = SemanticDemandStages::default();
        let started = Instant::now();
        let mut async_demands: Vec<EntityDemand> = Vec::new();
        for group in groups.iter() {
            group
                .append_async_demands(&mut async_demands)
                .map_err(BackendError)?;
        }
        let mut async_by_path = async_functions_for_demands(self, ctx, &async_demands)?;
        for source in &source_digests {
            cancelled(ctx)?;
            let facts = async_by_path
                .remove(source.path.as_ref())
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

        // Every file is resolved: the store is empty at the start of a table.
        let started = Instant::now();
        for group in groups.iter_mut() {
            group.ensure_expanded().map_err(BackendError)?;
        }
        if !groups.is_empty() {
            let demands: Vec<Arc<[EntityDemand]>> = groups
                .iter()
                .map(|group| {
                    group
                        .demands
                        .clone()
                        .unwrap_or_else(|| Arc::from(Vec::new()))
                })
                .collect();
            let runs: Vec<SemanticDemandRun<'_>> = groups
                .iter()
                .zip(&demands)
                .map(|(group, demands)| SemanticDemandRun {
                    path: &group.path,
                    demands,
                })
                .collect();
            let results = self.backend.semantic_demand_runs(ctx, &runs)?;
            drop(runs);
            rebuild_contributions(groups, results)?;
        }
        // Files holding generation-scoped counter ids are counted; the
        // timings report them.
        for group in groups.iter() {
            if group
                .contribution
                .as_ref()
                .is_some_and(|contribution| !contribution.borrow().durable)
            {
                retention.non_durable_files += 1;
            }
        }
        retention.recomputed_files = groups.len();
        let binder = self.backend.binder_retention();
        if let Some(trace) = &self.trace {
            trace.stage("analyze-references", binder.build);
        }
        retention.cached_reference_facts = binder.reused_reference_files;
        retention.recomputed_references = binder.scanned_reference_files;
        retention.cached_symbol_facts = binder.reused_symbols;
        retention.recomputed_symbol_facts = binder.resolved_symbols;
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
        // Every demanded file was recomputed and generation-scoped ids are
        // minted afresh, so any path may differ from the previous table (a
        // different counter, a shifted reference space) and the manifest names
        // all of them, along
        // with the paths the update named. The previous table keeps no rows
        // (they are released once published), so there is nothing to compare
        // values against; naming a path that did not change only costs bytes.
        let manifest_changed_paths: Option<HashSet<String>> = {
            let mut paths = self.transport_changed_paths.clone().unwrap_or_default();
            paths.extend(groups.iter().map(|group| group.path.clone()));
            Some(paths)
        };
        table.transport = rust_owned_transport_manifest(
            self.previous_table.as_ref(),
            manifest_changed_paths.as_ref(),
        );
        // Published for this generation's readers; the next table or update
        // discards them (`discard_derived_state`), never reuses them.
        self.retained.commit(groups);
        self.previous_table = None;
        self.transport_changed_paths = None;
        Ok((table, stages))
    }
}
