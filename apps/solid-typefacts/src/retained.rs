//! The materializer of the demand closure (Go: `semantic_retained.go`).
//!
//! The closure reuses no demand contribution across an update or across a
//! change of the demand set (ADR 0364): every table begins by discarding what
//! earlier analysis left (`DemandClosure::discard_derived_state`) and asks the
//! backend for every run. The backend answers each file in a pass of a
//! checker of its own and reuses a pass while everything it read is
//! unchanged (`per_file`, `passes`; ADR 03XX-p6). The contributions are
//! published in the store only so the packed transition can release their
//! entity rows.

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

/// The clean locations of the demands that asked for async facts.
fn async_locations(demands: &[EntityDemand]) -> Vec<typefacts::Location> {
    demands
        .iter()
        .filter(|demand| demand.r#async)
        .map(|demand| {
            let mut location = demand.location.clone();
            location.path = clean(&location.path).into();
            location
        })
        .collect()
}

/// Async facts keyed by clean path, their expression paths cleaned.
fn async_facts_by_path(facts: Vec<AsyncFunctionFact>) -> HashMap<String, Vec<AsyncFunctionFact>> {
    let mut by_path: HashMap<String, Vec<AsyncFunctionFact>> = HashMap::new();
    for mut fact in facts {
        let path = clean(&fact.expression.path);
        fact.expression.path = path.as_str().into();
        by_path.entry(path).or_default().push(fact);
    }
    by_path
}

/// One file row per source digest, holding its async facts.
fn push_file_facts(
    table: &mut ProducerTable,
    source_digests: &[SourceDigest],
    async_by_path: &mut HashMap<String, Vec<AsyncFunctionFact>>,
    ctx: &Context,
) -> Result<(), BackendError> {
    for source in source_digests {
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
    Ok(())
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

        // Reuse counts are this table's own.
        let _ = self.backend.binder_retention();
        // Each file is checked first in a pass of its own, with its async
        // facts and its run (`ClosureBackend::per_file_demand_runs`); a
        // group's run holds only the rows that asked for async.
        let mut stages = SemanticDemandStages::default();
        let started = Instant::now();
        let mut async_demands: Vec<EntityDemand> = Vec::new();
        for group in groups.iter() {
            group
                .append_async_demands(&mut async_demands)
                .map_err(BackendError)?;
        }
        let mut async_by_path = HashMap::new();
        stages.async_stage = started.elapsed();

        // Every file is resolved: the store is empty at the start of a table.
        let started = Instant::now();
        for group in groups.iter_mut() {
            group.ensure_expanded().map_err(BackendError)?;
        }
        if !groups.is_empty() {
            let locations = async_locations(&async_demands);
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
            let (results, facts) = self.backend.per_file_demand_runs(ctx, &runs, &locations)?;
            async_by_path = async_facts_by_path(facts);
            drop(runs);
            rebuild_contributions(groups, results)?;
        }
        push_file_facts(&mut table, &source_digests, &mut async_by_path, ctx)?;
        // Files holding an id that is not a binder-level hash are counted;
        // the timings report them.
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
        // Per-file passes with reuse (instant-edits P3) know, per path,
        // whether its rows changed: the manifest names exactly the paths whose
        // rows or source digest differ from the previous table's, or that
        // entered or left it.
        let versions: Option<crate::closure::PathVersions> =
            self.backend.tracks_content_versions().then(|| {
                let mut now: crate::closure::PathVersions = HashMap::new();
                for source in &source_digests {
                    now.entry(source.path.to_string()).or_default().1 = Some(source.sha256.clone());
                }
                for group in groups.iter() {
                    now.entry(group.path.clone()).or_default();
                }
                for (path, state) in &mut now {
                    state.0 = self.backend.content_version(path);
                }
                now
            });
        let exact: Option<HashSet<String>> = match (&versions, &self.path_versions) {
            (Some(now), Some((state_id, before)))
                if self
                    .previous_table
                    .as_ref()
                    .is_some_and(|previous| previous.state_id == *state_id) =>
            {
                let mut changed: HashSet<String> = now
                    .iter()
                    .filter(|(path, state)| before.get(*path) != Some(*state))
                    .map(|(path, _)| path.clone())
                    .collect();
                changed.extend(
                    before
                        .keys()
                        .filter(|path| !now.contains_key(*path))
                        .cloned(),
                );
                Some(changed)
            }
            _ => None,
        };
        self.path_versions = versions.map(|now| (table.state_id, now));
        // Otherwise every demanded file was recomputed and generation-scoped
        // ids are minted afresh, so any path may differ from the previous
        // table (a different counter, a shifted reference space) and the
        // manifest names all of them, along with the paths the update named.
        // The previous table keeps no rows (they are released once
        // published), so there is nothing to compare values against; naming
        // a path that did not change only costs bytes.
        let manifest_changed_paths: Option<HashSet<String>> = exact.or_else(|| {
            let mut paths = self.transport_changed_paths.clone().unwrap_or_default();
            paths.extend(groups.iter().map(|group| group.path.clone()));
            Some(paths)
        });
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
