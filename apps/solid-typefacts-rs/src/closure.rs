//! The demand closure (Go: `closure.go` and `semantic_demand.go`): one
//! generation's fact table from a demand set, reusing every per-file
//! contribution whose file stayed outside the affected set and whose demand
//! run is unchanged (ADR 0001).
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
use crate::symbols::intern::{INTERNER_RESET_SLACK, SharedInterner, SymbolInterner};
use crate::trace::SharedTrace;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use typefacts::producer::ProducerTable;
use typefacts::v3::{EntityDemand, FileChange, ModuleGraphRequest, SymbolQuery};
use typefacts::{AsyncFunctionFact, Location, SymbolFact};

pub const ERR_GENERATION_MISMATCH: &str = "type facts generation mismatch";
const ERR_CLOSED: &str = "closure project is closed";

/// How much of a generation's demand closure was carried over from retained
/// per-file contributions (Go: `ClosureRetention`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClosureRetention {
    pub retained_files: usize,
    pub recomputed_files: usize,
    pub retained_async_files: usize,
    pub recomputed_async_files: usize,
    pub non_durable_files: usize,
    pub suppression_recompute: bool,
    pub cached_symbol_facts: usize,
    pub recomputed_symbol_facts: usize,
    pub cached_reference_facts: usize,
    pub recomputed_references: usize,
    pub patched_symbol_rows: usize,
    pub shared_symbol_chunks: usize,
    pub stable_symbol_closure: bool,
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
    /// Assigns every symbol this session reaches a dense stable handle, and
    /// the seed snapshots indexed by those handles.
    pub(crate) interner: Option<SharedInterner>,
    pub(crate) last_roots: Option<Vec<bool>>,
    pub(crate) last_full_roots: Option<Vec<bool>>,
    pub(crate) last_full_tier: Option<Vec<bool>>,
    pub(crate) stats: ClosureStats,
    pub(crate) generation: u64,
    pub(crate) next_table_state_id: u64,
    pub(crate) closed: bool,
    /// Immutable per-file contributions and the reverse indexes used to
    /// invalidate exact dependents and structural-descriptor users.
    pub(crate) retained: RetainedContributionStore,
    /// The preceding generation's structural-accessor union. A changed union
    /// refreshes only paths named by the retained descriptor users.
    pub(crate) last_suppression: Option<HashSet<SymbolId>>,
    /// Reference lists whose presence is known (including known-empty lists).
    pub(crate) symbol_references: Option<HashMap<SymbolId, Vec<Location>>>,
    /// Durable canonical facts by clean declaring path, so `update` evicts an
    /// affected set by looking up its paths instead of scanning every retained
    /// fact. Every canonical-store change goes through `index_symbol_fact` /
    /// `unindex_symbol_fact`.
    pub(crate) symbols_by_path: HashMap<String, Vec<SymbolId>>,
    /// Every previously reached durable fact evicted by accepted source
    /// updates, and whether that evidence covered the entire preceding
    /// canonical store.
    pub(crate) invalidated_symbols: HashSet<SymbolId>,
    pub(crate) symbol_memo_complete: bool,
    /// Complete durable async-function contributions by demand path. Exact
    /// changed-path manifests let ordinary edits query only changed files;
    /// cross-file selections fall back to a full async batch.
    pub(crate) async_files: Option<HashMap<String, Vec<AsyncFunctionFact>>>,
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
            interner: None,
            last_roots: None,
            last_full_roots: None,
            last_full_tier: None,
            stats: ClosureStats::default(),
            generation: 1,
            next_table_state_id: 0,
            closed: false,
            retained: RetainedContributionStore::default(),
            last_suppression: None,
            symbol_references: None,
            symbols_by_path: HashMap::new(),
            invalidated_symbols: HashSet::new(),
            symbol_memo_complete: false,
            async_files: None,
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

    /// Records a retained fact under each clean declaring path. Callers pair
    /// it with every durable insert into the canonical symbol store.
    pub fn index_symbol_fact(&mut self, fact: &SymbolFact) {
        for declaration in fact.declarations.iter() {
            let ids = self
                .symbols_by_path
                .entry(declaration.location.path.to_string())
                .or_default();
            if !ids.contains(&fact.id) {
                ids.push(fact.id.clone());
            }
        }
    }

    /// Removes a retained fact from the path index. Callers must pair it with
    /// every canonical deletion, passing the fact being removed.
    pub fn unindex_symbol_fact(&mut self, fact: &SymbolFact) {
        for declaration in fact.declarations.iter() {
            let path = declaration.location.path.as_ref();
            let Some(ids) = self.symbols_by_path.get_mut(path) else {
                continue;
            };
            if let Some(index) = ids.iter().position(|existing| *existing == fact.id) {
                ids.remove(index);
            }
            if ids.is_empty() {
                self.symbols_by_path.remove(path);
            }
        }
    }

    /// Applies editor overlays and advances exactly one generation, even when
    /// the backend reports no affected source files. Retained contributions
    /// survive except for the affected set and the changed files themselves,
    /// which are evicted here rather than when next demanded.
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
        for path in &invalid_paths {
            while let Some(id) = self
                .symbols_by_path
                .get(path)
                .and_then(|ids| ids.last())
                .cloned()
            {
                self.invalidated_symbols.insert(id.clone());
                let retained = self
                    .table
                    .as_ref()
                    .and_then(|table| table.symbol(&id))
                    .cloned();
                if let Some(fact) = retained {
                    self.unindex_symbol_fact(&fact);
                } else if let Some(ids) = self.symbols_by_path.get_mut(path) {
                    ids.pop();
                }
                if let Some(references) = &mut self.symbol_references {
                    references.remove(&id);
                }
            }
        }
        self.previous_table = self.table.take();
        // Accepted source state invalidates the exact retained contributions
        // and their direct dependency users immediately. Analyze-time
        // preparation is transactional, but an accepted update itself cannot
        // be rolled back. Invalidation extends the set with those dependents,
        // and the manifest must name them all.
        self.retained.invalidate(&mut invalid_paths);
        self.transport_changed_paths = Some(invalid_paths);
        // Every accepted protocol update advances exactly one generation, even
        // when the backend reports no affected source files.
        self.generation += 1;
        Ok(affected)
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
        self.symbol_references = None;
        self.symbols_by_path = HashMap::new();
        self.retained.reset();
        self.last_suppression = None;
        self.async_files = None;
        self.interner = None;
        self.last_roots = None;
        self.last_full_roots = None;
        self.last_full_tier = None;
        self.invalidated_symbols = HashSet::new();
        self.symbol_memo_complete = false;
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
        self.backend.symbol_evidence(ctx, queries)
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

    /// Replaces the interner once dead identities outnumber the live symbol
    /// universe, using the preceding table's canonical symbol count as the
    /// liveness estimate. A session that keeps minting identities the closure
    /// never sees again would otherwise grow it without bound; handles carry
    /// no meaning across generations. Called between generations only, never
    /// while a build holds handles (Go: `maybeResetInterner`).
    pub fn maybe_reset_interner(&mut self, live_symbols: usize) {
        let stale = self.interner.as_ref().is_none_or(|interner| {
            interner.borrow().size() > 2 * live_symbols + INTERNER_RESET_SLACK
        });
        if stale {
            self.interner = Some(SymbolInterner::new_shared());
            // Seed snapshots are indexed by this interner's handles and cannot
            // cross an interner replacement.
            self.last_roots = None;
            self.last_full_roots = None;
            self.last_full_tier = None;
        }
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
            self.retained.discard(&invalidated);
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
                    ("patchedSymbolRows", count(retention.patched_symbol_rows)),
                    ("sharedSymbolChunks", count(retention.shared_symbol_chunks)),
                ],
            );
            trace.metrics(
                "retention",
                &[
                    ("retained", count(retention.retained_files)),
                    ("recomputed", count(retention.recomputed_files)),
                    (
                        "suppressionRecompute",
                        i64::from(retention.suppression_recompute),
                    ),
                ],
            );
        }
        self.table = Some(table);
        Ok(())
    }
}
