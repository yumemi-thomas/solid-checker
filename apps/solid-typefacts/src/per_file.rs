//! Per-file checkers (phase P1 of the incremental-edits plan), behind
//! `SOLID_TYPEFACTS_PER_FILE_CHECKERS=1`; off by default, and then nothing
//! here runs.
//!
//! Every demanded file's facts are computed by a fresh checker of its own
//! (`unsafe_boundary` invariant 8), in one pass that is freed when the file is
//! done: the file's check (its check-first), its async facts, its semantic
//! demand run, its reference-index contribution, and the symbol evidence rows
//! of every id its facts reach (with their alias targets). Every other
//! non-declaration file of the program gets a pass of its own for its
//! reference contribution. A file's facts are then a function of the file and
//! what its checker read, not of the order the program's files were checked
//! in.
//!
//! # What crosses passes
//!
//! Only pointer-free state: ids (strings), locations, the reference
//! contributions, the rows, and the identity state's pointer-free half
//! (`keys`, the exported identities, runtime paths). After every pass
//! `Identities::purge_scope_pointers` drops every map that may hold a handle
//! into the freed region; `ReferenceIndex` holds program-arena addresses
//! only (`scopes`, `current`), and the per-run memos (`DescriptorMemo`,
//! `CallCaches`) are dropped with the run, inside the pass.
//!
//! Three answers of a run read every file and are completed after every pass
//! (`semantic_runs::Deferrals`): reference spaces, the write half of the
//! default-library census, and the batch-wide structural-accessor suppression
//! of type descriptors.
//!
//! # Ids (P1)
//!
//! Derived symbols still take counter ids, now handed out across passes (the
//! counter is never reset inside a generation, so no id is issued twice). A
//! derived symbol two files share gets a different id in each: that is known
//! and is what P2's exact keys fix. A binder id hashes its key, so it is the
//! same in every pass.
//!
//! # `symbols` requests
//!
//! Served from the rows. An id without a row is resolved in a scratch pass:
//! a binder id re-resolves through its key; a counter id of a freed pass no
//! longer resolves and answers an id-only row (fails closed), counted in
//! [`Stats::unresolved_queries`]. Reference lists come from the merged index;
//! after `release_analysis_state` dropped it, it is rebuilt per file.

use crate::backend::BackendError;
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::identity::Env;
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SymbolId, blank_symbol};
use crate::project::{Project, id_declarations};
use crate::resolved_calls::reference_is_write;
use crate::semantic_runs::Deferrals;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::SymbolFlags;
use typefacts::v3::SymbolQuery;
use typefacts::{AsyncFunctionFact, Declaration, Location, SymbolFact};

/// The environment flag that selects per-file checkers.
pub const ENV_FLAG: &str = "SOLID_TYPEFACTS_PER_FILE_CHECKERS";

/// Opt-in self-check: a row already computed by an earlier pass is computed
/// again by every later pass that reaches its binder id, and differences are
/// counted ([`Stats::row_mismatches`]).
pub const ENV_VERIFY_ROWS: &str = "SOLID_TYPEFACTS_PER_FILE_VERIFY_ROWS";

/// Opt-in: the counters on stderr after every table.
pub const ENV_STATS: &str = "SOLID_TYPEFACTS_PER_FILE_STATS";

fn env_is_one(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

/// Whether the environment selects per-file checkers.
#[must_use]
pub fn enabled_by_env() -> bool {
    env_is_one(ENV_FLAG)
}

/// One symbol's evidence, computed inside the pass that reached its id.
/// Pointer-free.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    /// Whether the id resolved in that pass. An unresolved id answers an
    /// id-only row, as the canonical producer answers a stale id.
    found: bool,
    alias_target: SymbolId,
    declarations: Arc<[Declaration]>,
    /// The id of the alias-canonical symbol: its bucket is the reference list.
    canonical: SymbolId,
}

impl Row {
    fn missing() -> Self {
        Self {
            found: false,
            alias_target: "".into(),
            declarations: Arc::from(Vec::new()),
            canonical: "".into(),
        }
    }
}

/// Counters of the per-file mode, for measurement.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Passes run (a fresh checker each).
    pub passes: usize,
    /// Passes for a demanded file.
    pub demanded_passes: usize,
    /// Passes for a reference contribution only.
    pub reference_passes: usize,
    /// Passes for a question asked outside any pass.
    pub scratch_passes: usize,
    /// Rows computed in the demanded passes.
    pub rows: usize,
    /// Rows recomputed by a later pass that differed ([`ENV_VERIFY_ROWS`]).
    pub row_mismatches: usize,
    /// Rows compared ([`ENV_VERIFY_ROWS`]).
    pub rows_verified: usize,
    /// `symbols` queries answered from a precomputed row.
    pub row_queries: usize,
    /// `symbols` queries resolved in a scratch pass.
    pub scratch_queries: usize,
    /// Queries whose id resolved in no pass (an id-only row).
    pub unresolved_queries: usize,
    /// Deferred answers completed after every pass.
    pub deferred_spaces: usize,
    pub deferred_library: usize,
    pub withdrawn_library: usize,
    pub suppressed_descriptors: usize,
}

/// The per-file state a project keeps beside its identity state.
#[derive(Default)]
pub(crate) struct PerFile {
    rows: HashMap<SymbolId, Row>,
    pub(crate) stats: Stats,
    verify_rows: bool,
}

impl PerFile {
    pub(crate) fn new() -> Self {
        Self {
            verify_rows: env_is_one(ENV_VERIFY_ROWS),
            ..Self::default()
        }
    }

    /// Drops every row (a new table, a release, an update).
    pub(crate) fn clear_rows(&mut self) {
        self.rows = HashMap::new();
    }
}

/// The ids a table row of this file names, which the client's symbol closure
/// starts from: entity symbols, resolved-call targets, async-function symbols
/// and targets.
fn seeds(result: &SemanticDemandRunResult, facts: &[AsyncFunctionFact]) -> Vec<SymbolId> {
    let mut ids = Vec::new();
    for entity in &result.entities {
        ids.push(entity.symbol.clone());
        if let Some(call) = &entity.resolved_call {
            ids.push(call.target.clone());
        }
    }
    for fact in facts {
        ids.push(fact.symbol.clone());
        ids.push(fact.target.clone());
    }
    ids.retain(|id| !id.is_empty());
    ids
}

/// `id`'s row in the current pass: the canonical `symbol_evidence` answer
/// without references (alias target, declarations), and the canonical id the
/// references are listed under.
fn row_for(env: &mut Env<'_>, id: &SymbolId) -> Row {
    let Some(symbol) = env.symbol_for(id) else {
        return Row::missing();
    };
    let alias_target = if symbol.flags().intersects(SymbolFlags::Alias) {
        let original = env.checker.get_aliased_symbol(symbol);
        env.id_for(original)
    } else {
        "".into()
    };
    let declarations = id_declarations(env, id, symbol).into();
    let canonical = env.canonical_symbol(symbol);
    let canonical = env.id_for(canonical);
    Row {
        found: true,
        alias_target,
        declarations,
        canonical,
    }
}

/// Two rows of one binder id, computed by different passes, agree on what
/// does not depend on which pass minted a counter.
fn rows_agree(left: &Row, right: &Row) -> bool {
    let id_agrees = |a: &SymbolId, b: &SymbolId| {
        a == b || (!crate::model::durable_symbol_id(a) && !crate::model::durable_symbol_id(b))
    };
    left.found == right.found
        && left.declarations == right.declarations
        && id_agrees(&left.alias_target, &right.alias_target)
        && id_agrees(&left.canonical, &right.canonical)
}

/// The rows of `seeds` and of every alias target they reach, in the current
/// pass. A row an earlier pass computed stays (first pass wins).
fn rows_for(env: &mut Env<'_>, seeds: Vec<SymbolId>, state: &mut PerFile) {
    let mut pending = seeds;
    let mut visited: HashSet<SymbolId> = HashSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(existing) = state.rows.get(&id) {
            if state.verify_rows && crate::model::durable_symbol_id(&id) {
                let again = row_for(env, &id);
                state.stats.rows_verified += 1;
                if !rows_agree(existing, &again) {
                    state.stats.row_mismatches += 1;
                    if env_is_one(ENV_STATS) {
                        eprintln!("per-file row mismatch {id}: {existing:?} vs {again:?}");
                    }
                }
            }
            continue;
        }
        let row = row_for(env, &id);
        if !row.alias_target.is_empty() && !state.rows.contains_key(&row.alias_target) {
            pending.push(row.alias_target.clone());
        }
        state.rows.insert(id, row);
        state.stats.rows += 1;
    }
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

impl Project {
    /// Whether this project computes facts with per-file checkers.
    #[must_use]
    pub fn per_file_checkers(&self) -> bool {
        self.per_file.is_some()
    }

    /// Selects per-file checkers on an opened project (tests; the binary reads
    /// the environment at open).
    #[cfg(test)]
    pub(crate) fn select_per_file_checkers_for_test(&mut self) {
        self.per_file = Some(PerFile::new());
        self.references.set_per_file(true);
    }

    /// The per-file counters (tests, measurement).
    #[must_use]
    pub fn per_file_stats(&self) -> Option<Stats> {
        self.per_file.as_ref().map(|state| state.stats)
    }

    fn per_file_state(&mut self) -> &mut PerFile {
        self.per_file
            .as_mut()
            .expect("per-file checkers are selected")
    }

    /// Runs `f` inside a pass: a fresh checker of its own is this thread's
    /// checker for the program until `f` returns; then every identity map
    /// that may point into its region is purged and the checker and its
    /// region are freed (`unsafe_boundary` invariant 8). Nothing `f` returns
    /// may hold a tsrs handle.
    pub(crate) fn in_file_pass<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R, BackendError> {
        let scope = self.live()?.file_checker();
        let result = f(self);
        // Invariant 1 at the drop: nothing may point into the region once it
        // is freed, so the maps go first.
        self.ids.purge_scope_pointers();
        drop(scope);
        if let Some(state) = &mut self.per_file {
            state.stats.passes += 1;
        }
        Ok(result)
    }

    /// Per-file checkers: resolves the runs (path-ordered, as the closure
    /// orders them) and the async locations, one pass per demanded file, then
    /// the reference contributions of the program's other files, then the
    /// answers that read every file. Results are index-aligned with `runs`;
    /// the async facts are what one `async_functions_at_locations` call over
    /// `async_locations` returns.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn per_file_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        async_locations: &[Location],
    ) -> Result<(Vec<SemanticDemandRunResult>, Vec<AsyncFunctionFact>), BackendError> {
        check(ctx)?;
        let program = self.live()?.program();
        // Held outside the project for the passes (they borrow it whole);
        // the placeholder left behind keeps the mode selected.
        let mut state = std::mem::take(self.per_file_state());
        state.clear_rows();
        let mut async_by_path: BTreeMap<Arc<str>, Vec<Location>> = BTreeMap::new();
        for location in async_locations {
            async_by_path
                .entry(location.path.clone())
                .or_default()
                .push(location.clone());
        }
        let mut unavailable = std::mem::take(&mut self.unavailable);
        let mut library_writes: Option<Arc<HashSet<String>>> = None;
        let mut results: Vec<SemanticDemandRunResult> = Vec::with_capacity(runs.len());
        let mut deferred: Vec<Deferrals> = Vec::with_capacity(runs.len());
        let mut async_batches: Vec<Vec<AsyncFunctionFact>> = Vec::new();
        let mut scanned: HashSet<String> = HashSet::new();

        let outcome = (|| -> Result<(), BackendError> {
            for run in runs {
                check(ctx)?;
                let file = absolute(run.path)
                    .ok()
                    .and_then(|path| program.get_source_file(&path));
                let locations = async_by_path.remove(run.path).unwrap_or_default();
                let mut deferrals = Deferrals {
                    library_writes: library_writes.take(),
                    ..Deferrals::default()
                };
                let (result, facts) = self.in_file_pass(|this| {
                    // The file's own check-first (ADR 0367), under its checker.
                    if let Some(file) = file {
                        let _ = this.live()?.semantic_diagnostics(file);
                        check(ctx)?;
                    }
                    let facts = if locations.is_empty() {
                        Vec::new()
                    } else {
                        this.async_functions_at_locations(ctx, &locations)?
                    };
                    let mut results = this.with_env(|env, references| {
                        crate::semantic_runs::run(
                            ctx,
                            env,
                            references,
                            std::slice::from_ref(run),
                            &mut unavailable,
                            Some(&mut deferrals),
                        )
                    })??;
                    let result = results.pop().ok_or_else(|| {
                        BackendError("semantic demand run returned no result".to_owned())
                    })?;
                    if let Some(file) = file
                        && !file.is_declaration_file()
                    {
                        this.with_env(|env, references| references.scan_file(env, file))?;
                        scanned.insert(clean(file.file_name()));
                    }
                    let seeds = seeds(&result, &facts);
                    this.with_env(|env, _| rows_for(env, seeds, &mut state))?;
                    Ok::<_, BackendError>((result, facts))
                })??;
                state.stats.demanded_passes += 1;
                library_writes = deferrals.library_writes.take();
                results.push(result);
                deferred.push(deferrals);
                async_batches.push(facts);
            }
            // Async locations outside every run (none in practice: the
            // closure derives them from the runs' own demands).
            let rest: Vec<(Arc<str>, Vec<Location>)> =
                std::mem::take(&mut async_by_path).into_iter().collect();
            for (_, locations) in rest {
                check(ctx)?;
                let facts =
                    self.in_file_pass(|this| this.async_functions_at_locations(ctx, &locations))??;
                async_batches.push(facts);
            }
            // Every other non-declaration file's reference contribution, each
            // under a checker of its own.
            let mut paths: Vec<String> = Vec::new();
            for file in program.source_files() {
                if file.is_declaration_file() {
                    continue;
                }
                let path = clean(file.file_name());
                if !scanned.contains(&path) {
                    check(ctx)?;
                    let file = *file;
                    self.in_file_pass(|this| {
                        this.with_env(|env, references| references.scan_file(env, file))
                    })??;
                    state.stats.reference_passes += 1;
                }
                paths.push(path);
            }
            self.references.finish_per_file(paths);
            Ok(())
        })();
        for (family, count) in unavailable.0 {
            *self.unavailable.0.entry(family).or_default() += count;
        }
        // The passes counted themselves on the placeholder state.
        let placeholder = std::mem::take(self.per_file_state());
        state.stats.passes += placeholder.stats.passes;
        state.stats.scratch_passes += placeholder.stats.scratch_passes;
        if let Err(error) = outcome {
            *self.per_file_state() = state;
            return Err(error);
        }

        // The answers that read every file, from pointer-free state only.
        let structural: HashSet<SymbolId> = deferred
            .iter()
            .flat_map(|deferrals| deferrals.structural.iter().cloned())
            .collect();
        for (result, deferrals) in results.iter_mut().zip(&deferred) {
            for (index, id) in &deferrals.spaces {
                result.entities[*index].reference_space = Some(self.references.merged_space(id));
                state.stats.deferred_spaces += 1;
            }
            for (index, id) in &deferrals.library {
                state.stats.deferred_library += 1;
                let written = self.references.query_merged(id).iter().any(|reference| {
                    let Some(source) = program.get_source_file(&reference.path) else {
                        return true;
                    };
                    u32::try_from(reference.start_byte)
                        .map_or(true, |start| reference_is_write(source, start))
                });
                if written && let Some(call) = &mut result.entities[*index].resolved_call {
                    Arc::make_mut(call).default_library_binding = None;
                    state.stats.withdrawn_library += 1;
                }
            }
            // The canonical run suppresses the descriptor of an entity whose
            // symbol is a structural accessor anywhere in the batch.
            for entity in &mut result.entities {
                if entity.type_descriptor.is_some()
                    && !entity.symbol.is_empty()
                    && structural.contains(&entity.symbol)
                    && !deferrals.structural.contains(&entity.symbol)
                {
                    entity.type_descriptor = None;
                    state.stats.suppressed_descriptors += 1;
                }
            }
        }
        let facts = crate::source_facts::merge_async_facts(async_batches);
        if env_is_one(ENV_STATS) {
            eprintln!("per-file stats (table): {:?}", state.stats);
        }
        if let Some(trace) = &self.trace {
            let count = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
            let stats = state.stats;
            trace.metrics(
                "per-file",
                &[
                    ("passes", count(stats.passes)),
                    ("demandedPasses", count(stats.demanded_passes)),
                    ("referencePasses", count(stats.reference_passes)),
                    ("rows", count(stats.rows)),
                    ("rowMismatches", count(stats.row_mismatches)),
                ],
            );
        }
        *self.per_file_state() = state;
        Ok((results, facts))
    }

    /// The rows of `ids`: the table passes' rows, and for an id without one,
    /// a row computed in one scratch pass for this call only. A scratch row is
    /// not kept: identity state moves between calls (an update, an id minted
    /// again), and a kept "not found" would outlive it.
    fn rows_of(&mut self, ids: &[SymbolId]) -> Result<HashMap<SymbolId, Row>, BackendError> {
        let state = self.per_file_state();
        let mut rows: HashMap<SymbolId, Row> = HashMap::with_capacity(ids.len());
        let mut missing: Vec<SymbolId> = Vec::new();
        for id in ids {
            if rows.contains_key(id) {
                continue;
            }
            match state.rows.get(id) {
                Some(row) => {
                    state.stats.row_queries += 1;
                    rows.insert(id.clone(), row.clone());
                }
                None => {
                    if !missing.contains(id) {
                        missing.push(id.clone());
                    }
                }
            }
        }
        if missing.is_empty() {
            return Ok(rows);
        }
        let mut computed: Vec<(SymbolId, Row)> = Vec::with_capacity(missing.len());
        self.in_file_pass(|this| {
            this.with_env(|env, _| {
                for id in missing {
                    let row = row_for(env, &id);
                    computed.push((id, row));
                }
            })
        })??;
        let state = self.per_file_state();
        state.stats.scratch_passes += 1;
        state.stats.scratch_queries += computed.len();
        for (id, row) in computed {
            if !row.found {
                state.stats.unresolved_queries += 1;
            }
            rows.insert(id, row);
        }
        Ok(rows)
    }

    /// The merged reference index, rebuilt per file if a release dropped it.
    pub(crate) fn ensure_references_per_file(&mut self, ctx: &Context) -> Result<(), BackendError> {
        if self.references.has_merged() {
            return Ok(());
        }
        let program = self.live()?.program();
        let mut paths: Vec<String> = Vec::new();
        for file in program.source_files() {
            if file.is_declaration_file() {
                continue;
            }
            check(ctx)?;
            let file = *file;
            self.in_file_pass(|this| {
                this.with_env(|env, references| references.scan_file(env, file))
            })??;
            self.per_file_state().stats.reference_passes += 1;
            paths.push(clean(file.file_name()));
        }
        self.references.finish_per_file(paths);
        Ok(())
    }

    /// Per-file `symbol_evidence`: the canonical answer, from the rows.
    pub(crate) fn per_file_symbol_evidence(
        &mut self,
        ctx: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        check(ctx)?;
        let ids: Vec<SymbolId> = queries.iter().map(|query| query.id.clone()).collect();
        let rows = self.rows_of(&ids)?;
        if queries.iter().any(|query| query.references) {
            self.ensure_references_per_file(ctx)?;
        }
        let mut facts = Vec::with_capacity(queries.len());
        for query in queries {
            check(ctx)?;
            let mut fact = blank_symbol(query.id.clone());
            let row = &rows[&query.id];
            if !row.found {
                facts.push(fact);
                continue;
            }
            if !query.references_only {
                fact.alias_target = row.alias_target.clone();
                fact.declarations = Arc::clone(&row.declarations);
            }
            if query.references {
                fact.references = self.references.query_merged(&row.canonical).into();
            }
            facts.push(fact);
        }
        Ok(facts)
    }

    /// Per-file `references_batch`.
    pub(crate) fn per_file_references_batch(
        &mut self,
        ctx: &Context,
        ids: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
        check(ctx)?;
        let rows = self.rows_of(ids)?;
        self.ensure_references_per_file(ctx)?;
        let mut result = HashMap::with_capacity(ids.len());
        for id in ids {
            let row = &rows[id];
            if !row.found {
                continue;
            }
            result.insert(id.clone(), self.references.query_merged(&row.canonical));
        }
        Ok(result)
    }

    /// Per-file `references`: `None` for an id that resolves in no pass.
    pub(crate) fn per_file_references(
        &mut self,
        ctx: &Context,
        id: &SymbolId,
    ) -> Result<Option<Vec<Location>>, BackendError> {
        Ok(self
            .per_file_references_batch(ctx, std::slice::from_ref(id))?
            .remove(id))
    }

    /// Per-file `resolve_alias` and `declarations`: the id's row.
    pub(crate) fn per_file_row_answer(
        &mut self,
        id: &SymbolId,
    ) -> Result<(Option<SymbolId>, Option<Vec<Declaration>>), BackendError> {
        let mut rows = self.rows_of(std::slice::from_ref(id))?;
        let row = rows.remove(id).unwrap_or_else(Row::missing);
        if !row.found {
            return Ok((None, None));
        }
        let alias = (!row.alias_target.is_empty()).then(|| row.alias_target.clone());
        let declarations = (!row.declarations.is_empty()).then(|| row.declarations.to_vec());
        Ok((alias, declarations))
    }
}
