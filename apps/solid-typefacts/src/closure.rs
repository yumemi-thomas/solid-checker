//! The demand closure (Go: `closure.go` and `semantic_demand.go`): one
//! generation's fact table from a demand set. Every table is computed afresh
//! from the program and its demands (ADR 0364, 0367); only the backend's
//! binder-level answers outlive it (ADR 0369).
//!
//! The table it returns is transport-only: it exists to be converted to the
//! wire shape or diffed against its predecessor, and answers no per-location
//! queries. Symbol closure belongs to the client in the active protocol, so
//! the table carries entity and file rows and no symbol rows.
//!
//! Go guards the closure with a mutex because a protocol adapter may call it
//! from several goroutines; every Rust call arrives on the one worker thread
//! through `&mut self`, so there is no lock.

use crate::backend::{Backend, BackendError, ModuleInventory, UpdateOutcome};
use crate::context::Context;
use crate::contributions::RetainedContributionStore;
use crate::demands::{DemandGroup, canonical_demand_run};
use crate::gocompat::clean;
use crate::model::SymbolId;
use crate::trace::SharedTrace;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use typefacts::SymbolFact;
use typefacts::producer::ProducerTable;
use typefacts::v3::{EntityDemand, FileChange, ModuleGraphRequest, SymbolQuery};

pub const ERR_GENERATION_MISMATCH: &str = "type facts generation mismatch";
const ERR_CLOSED: &str = "closure project is closed";

/// How much of a generation's table was carried over (Go: `ClosureRetention`).
/// No demand contribution is (ADR 0364): `retained_files` stays on the wire at
/// zero. The cached counts are the backend's binder-level reuse (ADR 0369).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClosureRetention {
    pub retained_files: usize,
    pub recomputed_files: usize,
    pub non_durable_files: usize,
    pub cached_symbol_facts: usize,
    pub recomputed_symbol_facts: usize,
    pub cached_reference_facts: usize,
    pub recomputed_references: usize,
}

/// The cost of one generation's closed fact table (Go: `ClosureStats`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClosureStats {
    pub build_sequence: u64,
    pub generation: u64,
    pub files: usize,
    pub entities: usize,
    pub symbols: usize,
    pub full_tier_symbols: usize,
    pub build_duration: Duration,
    pub async_duration: Duration,
    pub demand_duration: Duration,
    pub symbol_duration: Duration,
    pub assembly_duration: Duration,
    pub sort_duration: Duration,
    pub close_duration: Duration,
    pub retention: ClosureRetention,
}

/// The per-stage durations of one materialization (Go: `semanticDemandStages`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SemanticDemandStages {
    pub async_stage: Duration,
    pub demand: Duration,
    pub assembly: Duration,
    pub sort: Duration,
    pub close: Duration,
    pub symbol: Duration,
}

/// One canonical per-file demand run as an external caller supplies it (Go:
/// `DemandGroup`). Stateful protocol adapters retain canonical runs of their
/// own instead.
#[derive(Clone, Debug, Default)]
pub struct DemandInputGroup {
    pub path: String,
    pub demands: Vec<EntityDemand>,
}

pub struct DemandClosure {
    pub(crate) backend: Box<dyn Backend>,
    pub(crate) trace: SharedTrace,
    pub(crate) table: Option<ProducerTable>,
    /// The immediate predecessor of the next table, and the paths that may
    /// differ from it. The private transport manifest is valid only for that
    /// base generation; protocol callers with an older acknowledged snapshot
    /// automatically take the full diff path.
    pub(crate) previous_table: Option<ProducerTable>,
    pub(crate) transport_changed_paths: Option<HashSet<String>>,
    pub(crate) stats: ClosureStats,
    pub(crate) generation: u64,
    pub(crate) next_table_state_id: u64,
    pub(crate) closed: bool,
    /// The published contributions, until the packed transition owns their
    /// rows.
    pub(crate) retained: RetainedContributionStore,
}

impl DemandClosure {
    /// Wraps a live backend. `trace` may be `None`, which disables tracing
    /// entirely (Go: `NewDemandClosure`).
    #[must_use]
    pub fn new(backend: Box<dyn Backend>, trace: SharedTrace) -> Self {
        Self {
            backend,
            trace,
            table: None,
            previous_table: None,
            transport_changed_paths: None,
            stats: ClosureStats::default(),
            generation: 1,
            next_table_state_id: 0,
            closed: false,
            retained: RetainedContributionStore::default(),
        }
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The most recent generation's materialization cost.
    #[must_use]
    pub fn stats(&self) -> ClosureStats {
        self.stats
    }

    /// The published table, for assertions on what the closure keeps.
    #[must_use]
    pub fn table(&self) -> Option<&ProducerTable> {
        self.table.as_ref()
    }

    /// The retained contributions, for assertions on what the closure keeps.
    #[must_use]
    pub fn retained(&self) -> &RetainedContributionStore {
        &self.retained
    }

    /// The backend, for tests that inspect their double.
    pub fn backend_mut(&mut self) -> &mut dyn Backend {
        self.backend.as_mut()
    }

    /// Called only after the packed transition owns the expanded entity and
    /// file rows. The closure keeps only the compact retained contributions
    /// needed to produce the next transition (Go: `releaseTransportRows`).
    pub fn release_transport_rows(&mut self) {
        let Some(table) = &mut self.table else {
            return;
        };
        for contribution in self.retained.by_path.values() {
            contribution.borrow_mut().release_transport_rows();
        }
        table.entities = typefacts::producer::EntityRows::Runs(Vec::new());
        table.files = Vec::new();
        table.sources = Vec::new();
    }

    pub fn force_full_materialization(&mut self) {
        self.reset_analysis_state();
    }

    /// Applies editor overlays and advances exactly one generation, even when
    /// the backend reports no affected source files.
    ///
    /// Every type-derived answer is recomputed: a retained answer is reusable
    /// only if everything it read is unchanged, and an answer computed
    /// through the checker's types can depend on what the checker computed
    /// before (ADR 0364). So the demand contributions and the backend's
    /// source-fact memo and identity maps are discarded, and the next table is
    /// computed as a fresh program would compute it. What stays is the program itself (incremental reuse inside
    /// the backend), the previous table, which only the transport delta
    /// compares against, and the backend's binder-level answers, each with the
    /// digests it read (ADR 0369). Every changed path plus the affected set is
    /// named as changed, and the recompute names every demanded path.
    pub fn update(
        &mut self,
        ctx: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        let affected = self.backend.update(ctx, changes)?;
        let mut invalid_paths: HashSet<String> = affected.affected.iter().cloned().collect();
        for change in changes {
            invalid_paths.insert(clean(&change.path));
        }
        self.discard_derived_state();
        self.previous_table = self.table.take();
        self.transport_changed_paths = Some(invalid_paths);
        // Every accepted protocol update advances exactly one generation, even
        // when the backend reports no affected source files.
        self.generation += 1;
        Ok(affected)
    }

    /// Drops what earlier analysis left behind and a table could otherwise
    /// reuse: demand contributions, and the backend's memo, reference index and
    /// id maps. The previous table stays (it only feeds the transport delta),
    /// and so do the backend's binder-level answers, which check their own
    /// dependencies (ADR 0369).
    fn discard_derived_state(&mut self) {
        self.retained.reset();
        self.backend.discard_derived_state();
    }

    pub fn close(&mut self) -> Result<(), BackendError> {
        self.closed = true;
        self.reset_analysis_state();
        self.backend.close()
    }

    fn reset_analysis_state(&mut self) {
        self.table = None;
        self.previous_table = None;
        self.transport_changed_paths = None;
        self.retained.reset();
    }

    /// Drops every derived analysis cache after a materialized table failed to
    /// publish through its owning session. Reconstructing on the next analyze
    /// is intentionally conservative and rare: retaining any part of the
    /// rejected demand snapshot could make it observable through a later,
    /// otherwise unrelated demand delta.
    pub fn abandon_analysis(&mut self) {
        self.reset_analysis_state();
    }

    /// Answers from the retained backend without materializing the demand
    /// closure: a table forced into existence here would be discarded as soon
    /// as the first analyze request arrives with real compiler seeds.
    pub fn source_files(
        &mut self,
        ctx: &Context,
    ) -> Result<Vec<typefacts::v3::SourceFile>, BackendError> {
        if self.closed {
            return Err(BackendError(ERR_CLOSED.to_owned()));
        }
        self.backend.source_files(ctx)
    }

    /// Answers from the retained backend without materializing the demand
    /// closure, exactly as [`Self::source_files`] does. The module graph is a
    /// property of the accepted program, not of any demand set, so asking for
    /// it neither builds a fact table nor disturbs one that exists.
    pub fn module_graph(
        &mut self,
        ctx: &Context,
        demand: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        if self.closed {
            return Err(BackendError(ERR_CLOSED.to_owned()));
        }
        self.backend.module_graph(ctx, demand)
    }

    /// Answers one client-owned closure worklist batch. It does not retain the
    /// returned rows: ownership crosses the process seam immediately (Go:
    /// `resolveSymbolEvidence`).
    pub fn resolve_symbol_evidence(
        &mut self,
        ctx: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        if self.closed {
            return Err(BackendError(ERR_CLOSED.to_owned()));
        }
        // One row per id: queries naming the same id are merged before the
        // backend answers, so no requested evidence is dropped. References
        // are wanted if any query wants them; the declaration and alias half
        // is skipped only if every query skips it.
        let mut merged: Vec<SymbolQuery> = Vec::with_capacity(queries.len());
        let mut position: HashMap<std::sync::Arc<str>, usize> = HashMap::new();
        for query in queries {
            match position.get(&query.id) {
                Some(&at) => {
                    merged[at].references |= query.references;
                    merged[at].references_only &= query.references_only;
                }
                None => {
                    position.insert(query.id.clone(), merged.len());
                    merged.push(query.clone());
                }
            }
        }
        self.backend.symbol_evidence(ctx, &merged)
    }

    pub fn release_backend_analysis_state(&mut self) {
        self.backend.release_analysis_state();
    }

    pub fn changed_references(
        &mut self,
        ctx: &Context,
    ) -> Result<(Vec<SymbolId>, bool), BackendError> {
        self.backend.changed_references(ctx)
    }

    /// The retained-v3 interface. `changed_paths` must name every run replaced
    /// or removed since the preceding successful call. `update` contributes
    /// source-affected paths independently, so callers only add demand-set
    /// changes here (Go: `DemandTableForGroups`).
    pub fn demand_table_for_groups(
        &mut self,
        ctx: &Context,
        generation: u64,
        groups: &[DemandInputGroup],
        changed_paths: &[String],
    ) -> Result<ProducerTable, BackendError> {
        let mut retained_groups: Vec<DemandGroup> = groups
            .iter()
            .map(|group| {
                let path = clean(&group.path);
                DemandGroup {
                    demands: Some(canonical_demand_run(group.demands.clone())),
                    path,
                    ..DemandGroup::default()
                }
            })
            .collect();
        self.demand_table_for_prepared_groups(ctx, generation, &mut retained_groups, changed_paths)
    }

    /// The session-owned path. The retained demand transaction has already
    /// canonicalized every changed run, so rechecking every retained run here
    /// would move external-input work back into the edit hot path (Go:
    /// `demandTableForCanonicalGroups`).
    pub fn demand_table_for_canonical_groups(
        &mut self,
        ctx: &Context,
        generation: u64,
        groups: &mut [DemandGroup],
        changed_paths: &[String],
    ) -> Result<ProducerTable, BackendError> {
        self.demand_table_for_prepared_groups(ctx, generation, groups, changed_paths)
    }

    fn demand_table_for_prepared_groups(
        &mut self,
        ctx: &Context,
        generation: u64,
        groups: &mut [DemandGroup],
        changed_paths: &[String],
    ) -> Result<ProducerTable, BackendError> {
        if generation != self.generation {
            return Err(BackendError(ERR_GENERATION_MISMATCH.to_owned()));
        }
        if !changed_paths.is_empty() {
            if self.table.is_some() {
                self.previous_table = self.table.take();
            }
            let invalidated: HashSet<String> =
                changed_paths.iter().map(|path| clean(path)).collect();
            self.transport_changed_paths
                .get_or_insert_with(|| HashSet::with_capacity(invalidated.len()))
                .extend(invalidated);
        }
        if self.table.is_none() {
            groups.sort_by(|left, right| left.path.cmp(&right.path));
            for index in 1..groups.len() {
                if groups[index - 1].path == groups[index].path {
                    return Err(BackendError(format!(
                        "demand groups name {} twice; each path may appear once",
                        crate::gocompat::quote(&groups[index].path)
                    )));
                }
            }
            self.materialize_demand_table(ctx, generation, groups)?;
        }
        self.table
            .clone()
            .ok_or_else(|| BackendError("demand closure produced no table".to_owned()))
    }

    fn materialize_demand_table(
        &mut self,
        ctx: &Context,
        generation: u64,
        groups: &mut [DemandGroup],
    ) -> Result<(), BackendError> {
        let started = std::time::Instant::now();
        // A table is a function of the program and its demands, not of the
        // order earlier queries or earlier tables claimed ids in.
        self.discard_derived_state();
        let (table, full_tier_symbols, stages, retention) =
            self.materialize_semantic_demand_retained(ctx, groups, generation)?;
        let entity_count = table.entities.len();
        let symbol_count = table.symbols.len();
        let file_count = table.files.len();
        self.stats = ClosureStats {
            build_sequence: self.stats.build_sequence + 1,
            generation,
            files: file_count,
            entities: entity_count,
            symbols: symbol_count,
            full_tier_symbols,
            build_duration: started.elapsed(),
            async_duration: stages.async_stage,
            demand_duration: stages.demand,
            symbol_duration: stages.symbol,
            assembly_duration: stages.assembly,
            sort_duration: stages.sort,
            close_duration: stages.close,
            retention,
        };
        if let Some(trace) = &self.trace {
            // These walks are why the trace is optional: counting descriptors,
            // calls and references is a full pass over the table, and it must
            // not run when nobody is listening.
            let (mut descriptors, mut calls, mut references) = (0_i64, 0_i64, 0_i64);
            table.entities.for_each(|entity| {
                descriptors += i64::from(entity.type_descriptor.is_some());
                calls += i64::from(entity.resolved_call.is_some());
            });
            for symbol in &table.symbols {
                references += i64::try_from(symbol.references.len()).unwrap_or(i64::MAX);
            }
            let count = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
            trace.stage("analyze-materialize", self.stats.build_duration);
            trace.stage("analyze-async", stages.async_stage);
            trace.stage("analyze-demand", stages.demand);
            trace.stage("analyze-symbols", stages.symbol);
            trace.metrics(
                "counts",
                &[
                    ("entities", count(entity_count)),
                    ("symbols", count(symbol_count)),
                    ("descriptors", descriptors),
                    ("calls", calls),
                    ("references", references),
                    ("cachedSymbolFacts", count(retention.cached_symbol_facts)),
                    (
                        "recomputedSymbolFacts",
                        count(retention.recomputed_symbol_facts),
                    ),
                    (
                        "cachedReferenceFacts",
                        count(retention.cached_reference_facts),
                    ),
                    (
                        "recomputedReferences",
                        count(retention.recomputed_references),
                    ),
                ],
            );
            trace.metrics(
                "retention",
                &[
                    ("retained", count(retention.retained_files)),
                    ("recomputed", count(retention.recomputed_files)),
                ],
            );
        }
        self.table = Some(table);
        Ok(())
    }
}
