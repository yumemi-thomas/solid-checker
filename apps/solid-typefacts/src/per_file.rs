//! Per-file checkers, the producer's only mode (ADR 03XX-p6).
//!
//! Every demanded file's facts are computed by a fresh checker of its own
//! (`unsafe_boundary` invariant 8), in one pass that is freed when the file is
//! done: the file's check (first), its async facts, its semantic
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
//! # Ids
//!
//! A binder id hashes its key and a derived symbol's id its exact key
//! (`derived_key.rs`), so both are the same in every pass; a symbol with no
//! exact key carries no id at all.
//!
//! # `symbols` requests
//!
//! Served from the rows. An id without a row is resolved in a scratch pass:
//! a binder id re-resolves through its key; a derived id no position names
//! does not resolve there and answers an id-only row (fails closed), counted
//! in [`Stats::unresolved_queries`]. Reference lists come from the merged index;
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

/// Opt-in self-check: a row already computed by an earlier pass is computed
/// again by every later pass that reaches its binder id, and differences are
/// counted ([`Stats::row_mismatches`]).
pub const ENV_VERIFY_ROWS: &str = "SOLID_TYPEFACTS_PER_FILE_VERIFY_ROWS";

/// Opt-in: the counters on stderr after every table.
pub const ENV_STATS: &str = "SOLID_TYPEFACTS_PER_FILE_STATS";

fn env_is_one(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

/// One symbol's evidence, computed inside the pass that reached its id.
/// Pointer-free.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    /// Whether the id resolved in that pass. An unresolved id answers an
    /// id-only row, as the canonical producer answers a stale id.
    pub(crate) found: bool,
    pub(crate) alias_target: SymbolId,
    pub(crate) declarations: Arc<[Declaration]>,
    /// The id of the alias-canonical symbol: its bucket is the reference list.
    pub(crate) canonical: SymbolId,
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

/// A reused pass differed from its recomputation
/// (`passes::ENV_VERIFY_REUSE`): reported, and the fresh output is served; a
/// test build panics.
fn reuse_mismatch(path: &str) {
    eprintln!("per-file reuse mismatch: {path} (served fresh)");
    if cfg!(test) {
        panic!("per-file reuse mismatch: {path}");
    }
}

/// What the runs and async locations of one table answer.
type RunsAnswer = (Vec<SemanticDemandRunResult>, Vec<AsyncFunctionFact>);

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
    /// Pass reuse and the pool (`passes`), for the last table.
    pub reuse: crate::passes::ReuseStats,
    /// Tables whose passes minted one id for two keys (the sequential
    /// shared-keys run answered them instead).
    pub key_collisions: usize,
}

/// The per-file state a project keeps beside its identity state.
#[derive(Default)]
pub(crate) struct PerFile {
    rows: HashMap<SymbolId, Row>,
    pub(crate) stats: Stats,
    verify_rows: bool,
    /// Kept pass outputs (instant-edits P3).
    pub(crate) reuse: crate::passes::Reuse,
    /// Worker threads for the passes.
    workers: usize,
    /// What the last table served per path, and a version per path that
    /// changes exactly when what the path's rows hold changed
    /// ([`Project::per_file_content_version`]).
    served_runs: HashMap<String, (Arc<crate::passes::PassOutput>, Vec<Completion>)>,
    served_async: HashMap<String, Vec<AsyncFunctionFact>>,
    versions: HashMap<String, u64>,
    next_version: u64,
    /// Whether `versions` describes the last table.
    versions_valid: bool,
    /// The last table's pass outputs merged (keys, rows), patched by what
    /// changed.
    pub(crate) merged: crate::passes::Merged,
    /// The merged reference index while the project's is dropped (a
    /// release, or a program change until the next table patches it).
    pub(crate) stashed_index: Option<crate::references::PassIndex>,
    /// Whether `stashed_index` describes the current program (a release
    /// stashed it), so reference questions may use it.
    pub(crate) stashed_current: bool,
    /// Which `symbols` answers the last table changed against the table
    /// before it ([`Project::per_file_symbol_changes`]); `None` when that is
    /// not known exactly.
    symbol_changes: Option<SymbolChanges>,
    /// Whether the last table's rows were the merged passes' (so the next
    /// merge's changes are relative to what that table answered).
    merged_base: bool,
    /// Every id a scratch pass ever answered: its answer is not a row the
    /// merge tracks, so every table names it as changed.
    scratch_ids: HashSet<SymbolId>,
    /// Tests: every table is the sequential run's (as after a key collision).
    #[cfg(test)]
    sequential_for_test: bool,
}

/// What one table changed in the `symbols` answers of the table before it
/// (instant-edits P4): the ids whose served row differs, and the reference
/// buckets (alias-canonical ids) whose locations differ.
#[derive(Clone, Debug, Default)]
struct SymbolChanges {
    rows: Vec<SymbolId>,
    buckets: HashSet<SymbolId>,
    /// `rows`, the scratch ids and every id whose row's bucket is in
    /// `buckets`, sorted: computed on the first question.
    answered: Option<Vec<SymbolId>>,
}

/// The ids whose reference spans differ between two contributions of one
/// path.
fn changed_buckets(
    old: Option<&crate::references::FileReferences>,
    new: Option<&crate::references::FileReferences>,
    buckets: &mut HashSet<SymbolId>,
) {
    fn groups(
        references: Option<&crate::references::FileReferences>,
    ) -> HashMap<&SymbolId, &Vec<crate::references::Span>> {
        references
            .map(|references| {
                references
                    .refs
                    .iter()
                    .map(|group| (&group.id, &group.spans))
                    .collect()
            })
            .unwrap_or_default()
    }
    let (old, new) = (groups(old), groups(new));
    for (id, spans) in &old {
        if new.get(id) != Some(spans) {
            buckets.insert((*id).clone());
        }
    }
    for id in new.keys() {
        if !old.contains_key(id) {
            buckets.insert((*id).clone());
        }
    }
}

/// How the cross-file answers of one entity were completed after the passes
/// (`Deferrals`): what the entity row holds beyond its pass's own output.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Completion {
    Space(typefacts::ReferenceSpace),
    Withdrawn(bool),
    Suppressed(usize),
}

impl PerFile {
    pub(crate) fn new() -> Self {
        Self {
            verify_rows: env_is_one(ENV_VERIFY_ROWS),
            workers: crate::passes::workers_from_env(),
            ..Self::default()
        }
    }

    /// Drops every row (a new table, a release, an update). The merged pass
    /// rows stop answering until the next table.
    pub(crate) fn clear_rows(&mut self) {
        self.rows = HashMap::new();
        self.merged.serving = false;
    }

    /// The row of `id` the last table computed.
    fn row(&self, id: &SymbolId) -> Option<&Row> {
        if self.merged.serving {
            self.merged.row(id)
        } else {
            self.rows.get(id)
        }
    }
}

/// Every id a table row of this file names: the client's symbol closure
/// starts from entity symbols, resolved-call targets, async-function symbols
/// and targets; a call's declarations (and their owners) and its parameters
/// name ids too, and a later `symbols` query for any of them is answered from
/// this pass's row, as the canonical producer answers it from the symbol it
/// holds for the generation. (A derived id cannot be re-resolved in a later
/// pass: no position names it.)
pub(crate) fn seeds(
    result: &SemanticDemandRunResult,
    facts: &[AsyncFunctionFact],
) -> Vec<SymbolId> {
    fn declaration(ids: &mut Vec<SymbolId>, declaration: &typefacts::ResolvedDeclaration) {
        ids.push(declaration.symbol.clone());
        ids.extend(declaration.owners.iter().map(|owner| owner.symbol.clone()));
    }
    let mut ids = Vec::new();
    for entity in &result.entities {
        ids.push(entity.symbol.clone());
        if let Some(call) = &entity.resolved_call {
            ids.push(call.target.clone());
            if let Some(found) = &call.declaration {
                declaration(&mut ids, found);
            }
            if let Some(found) = &call.default_library_binding {
                declaration(&mut ids, found);
            }
            if let Some(targets) = &call.targets {
                for found in targets.candidates.iter() {
                    declaration(&mut ids, found);
                }
            }
            for argument in call.arguments.iter() {
                if let Some(parameter) = &argument.parameter {
                    ids.push(parameter.symbol.clone());
                }
            }
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
pub(crate) fn row_for(env: &mut Env<'_>, id: &SymbolId) -> Row {
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

/// Two rows of one id, computed by different passes, agree: an id names
/// one symbol in every pass.
pub(crate) fn rows_agree(left: &Row, right: &Row) -> bool {
    left == right
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
            if state.verify_rows {
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
    /// Answers every table with the sequential shared-identity run, as a key
    /// collision does (tests).
    #[cfg(test)]
    pub(crate) fn select_sequential_for_test(&mut self) {
        self.per_file.sequential_for_test = true;
    }

    /// Sets the pass pool's workers and pass reuse (tests; the binary reads
    /// the environment at open).
    #[cfg(test)]
    pub(crate) fn select_passes_for_test(&mut self, workers: usize, reuse: bool, verify: bool) {
        let state = self.per_file_state();
        state.workers = workers;
        state.reuse.set_enabled_for_test(reuse, verify);
    }

    /// The read set of `path` at the last table (tests).
    #[cfg(test)]
    pub(crate) fn per_file_read_set_for_test(&self, path: &str) -> Vec<String> {
        self.per_file.reuse.read_set_for_test(path)
    }

    /// The per-file counters (tests, measurement).
    #[must_use]
    pub fn per_file_stats(&self) -> Stats {
        self.per_file.stats
    }

    fn per_file_state(&mut self) -> &mut PerFile {
        &mut self.per_file
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
        self.per_file.stats.passes += 1;
        Ok(result)
    }

    /// Per-file checkers: resolves the runs (path-ordered, as the closure
    /// orders them) and the async locations, one pass per demanded file, then
    /// the reference contributions of the program's other files, then the
    /// answers that read every file. Results are index-aligned with `runs`;
    /// the async facts are what one `async_functions_at_locations` call over
    /// `async_locations` returns.
    ///
    /// Each pass is a function of what its checker read: passes are reused
    /// across tables and run on a pool ([`Project::pooled_demand_runs`]).
    /// When two passes minted one id for two keys, the passes run in path
    /// order over one identity state instead
    /// ([`Project::sequential_demand_runs`]): the first pass to mint an id
    /// decides its key.
    pub(crate) fn per_file_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        async_locations: &[Location],
    ) -> Result<(Vec<SemanticDemandRunResult>, Vec<AsyncFunctionFact>), BackendError> {
        #[cfg(test)]
        let pooled = !self.per_file.sequential_for_test;
        #[cfg(not(test))]
        let pooled = true;
        if pooled && let Some(answer) = self.pooled_demand_runs(ctx, runs, async_locations)? {
            return Ok(answer);
        }
        let state = self.per_file_state();
        state.versions_valid = false;
        state.served_runs.clear();
        state.served_async.clear();
        state.symbol_changes = None;
        state.merged_base = false;
        state.merged = crate::passes::Merged::default();
        state.stashed_index = None;
        self.ids.set_table_keys(None);
        self.sequential_demand_runs(ctx, runs, async_locations)
    }

    /// The sequential per-file run over the project's one identity state
    /// (a key collision): the first pass in path order to mint an id decides
    /// its key.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn sequential_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        async_locations: &[Location],
    ) -> Result<(Vec<SemanticDemandRunResult>, Vec<AsyncFunctionFact>), BackendError> {
        check(ctx)?;
        let program = self.live()?.program();
        // Held outside the project for the passes (they borrow it whole);
        // a placeholder is left behind.
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
                    // The file's own check first, under its checker.
                    if let Some(file) = file {
                        let _ = this.live()?.semantic_diagnostics(file);
                        check(ctx)?;
                    }
                    let facts = if locations.is_empty() {
                        Vec::new()
                    } else {
                        this.async_functions_at_locations(ctx, &locations)?
                    };
                    let result = this.with_env(|env, _| {
                        crate::semantic_runs::run(ctx, env, run, &mut unavailable, &mut deferrals)
                    })??;
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
        let deferred: Vec<&Deferrals> = deferred.iter().collect();
        self.complete_deferred(program, &mut results, &deferred, &mut state.stats);
        let facts = crate::source_facts::merge_async_facts(async_batches);
        if env_is_one(ENV_STATS) {
            eprintln!(
                "per-file stats (table): {:?}; leaked file checkers {}",
                state.stats,
                crate::unsafe_boundary::leaked_file_checkers()
            );
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

    /// Per-file passes with reuse and a pool (instant-edits P3, `passes`).
    /// `None` when two passes minted one id for two keys: the caller answers
    /// with the sequential run, whose first pass in path order decides.
    #[allow(clippy::too_many_lines)]
    fn pooled_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        async_locations: &[Location],
    ) -> Result<Option<RunsAnswer>, BackendError> {
        use crate::passes::{PassOutput, PassWork, run_works};
        check(ctx)?;
        let program = self.live()?.program();
        let mut state = std::mem::take(self.per_file_state());
        state.clear_rows();
        let mut async_by_path: BTreeMap<Arc<str>, Vec<Location>> = BTreeMap::new();
        for location in async_locations {
            async_by_path
                .entry(location.path.clone())
                .or_default()
                .push(location.clone());
        }
        // The works, in the order their outputs merge: the runs (path
        // order), async locations outside every run, then every other
        // non-declaration file's reference contribution.
        let mut works: Vec<PassWork<'_>> = Vec::with_capacity(runs.len());
        let mut scanned: HashSet<String> = HashSet::new();
        for run in runs {
            if let Some(file) = absolute(run.path)
                .ok()
                .and_then(|path| program.get_source_file(&path))
                && !file.is_declaration_file()
            {
                scanned.insert(clean(file.file_name()));
            }
            works.push(PassWork {
                path: run.path.to_owned(),
                run: Some(run),
                locations: async_by_path.remove(run.path).unwrap_or_default(),
                scan: true,
                reusable: true,
            });
        }
        let async_count = works.len() + async_by_path.len();
        for (path, locations) in std::mem::take(&mut async_by_path) {
            works.push(PassWork {
                path: path.to_string(),
                run: None,
                locations,
                scan: false,
                reusable: false,
            });
        }
        let mut paths: Vec<String> = Vec::new();
        for file in program.source_files() {
            if file.is_declaration_file() {
                continue;
            }
            let path = clean(file.file_name());
            if !scanned.contains(&path) {
                works.push(PassWork {
                    path: path.clone(),
                    run: None,
                    locations: Vec::new(),
                    scan: true,
                    reusable: true,
                });
            }
            paths.push(path);
        }

        // Kept outputs whose read set is unchanged.
        let started = std::time::Instant::now();
        state
            .reuse
            .refresh(program, &mut self.inputs, self.ids.exported_by_ref());
        let mut outputs: Vec<Option<Arc<PassOutput>>> = works
            .iter()
            .map(|work| state.reuse.lookup(work, program, &mut self.inputs))
            .collect();
        let verify = state.reuse.verify;
        let to_run: Vec<usize> = (0..works.len())
            .filter(|index| verify || outputs[*index].is_none())
            .collect();
        let census = (!to_run.is_empty()).then(|| state.reuse.census(program, &mut self.inputs));
        let refresh = started.elapsed();

        let started = std::time::Instant::now();
        let seed = self.ids.pass_seed();
        let selected: Vec<&PassWork<'_>> = to_run.iter().map(|index| &works[*index]).collect();
        let workers = state.workers;
        let outcome = run_works(
            ctx,
            self.live()?,
            &seed,
            census.as_ref(),
            &selected,
            workers,
        );
        let pool = started.elapsed();
        for paths in outcome.runtime_paths {
            self.ids.absorb_runtime_paths(paths);
        }
        let mut failure: Option<BackendError> = None;
        let (mut ran, mut verified, mut mismatches) = (0, 0, 0);
        let mut recomputed = vec![false; works.len()];
        for (index, fresh) in to_run.iter().zip(outcome.outputs) {
            let fresh = match fresh {
                Ok(fresh) => fresh,
                Err(error) => {
                    failure.get_or_insert(error);
                    continue;
                }
            };
            if let Some(reused) = &outputs[*index] {
                verified += 1;
                if **reused == fresh {
                    continue;
                }
                mismatches += 1;
                reuse_mismatch(&works[*index].path);
            } else {
                ran += 1;
            }
            let fresh = Arc::new(fresh);
            state.reuse.keep(&works[*index], &fresh);
            outputs[*index] = Some(fresh);
            recomputed[*index] = true;
        }
        state.stats.reuse = crate::passes::ReuseStats {
            reused: works.len() - ran,
            ran,
            verified,
            mismatches,
            workers: workers.min(selected.len()).max(1),
            ..state.reuse.stats
        };
        if let Some(error) = failure {
            *self.per_file_state() = state;
            return Err(error);
        }
        let outputs: Vec<Arc<PassOutput>> = outputs
            .into_iter()
            .map(|output| output.expect("every work has an output"))
            .collect();
        state.stats.passes += ran + verified;
        state.stats.demanded_passes += runs.len();
        state.stats.reference_passes += works.len() - async_count;

        // Keys and rows, merged by the positions whose output changed: one id
        // names one key across every pass and what the project's own state
        // minted (scratch questions), or the table is the sequential run's.
        let started = std::time::Instant::now();
        self.ids.set_table_keys(None);
        let ids_of: Vec<crate::passes::WorkId> =
            works.iter().map(crate::passes::WorkId::of).collect();
        let update = match state
            .merged
            .update(ids_of, outputs.clone(), state.verify_rows)
        {
            Ok(update) => {
                let own = self
                    .ids
                    .own_keys()
                    .find(|(id, key)| state.merged.key(id).is_some_and(|held| held != *key))
                    .map(|(id, _)| id.clone());
                match own {
                    Some(id) => Err(id),
                    None => Ok(update),
                }
            }
            Err(id) => Err(id),
        };
        let update = match update {
            Ok(update) => update,
            Err(id) => {
                state.stats.key_collisions += 1;
                if env_is_one(ENV_STATS) {
                    eprintln!("per-file key collision {id}: the sequential run answers");
                }
                state.reuse.clear();
                state.merged = crate::passes::Merged::default();
                state.stashed_index = None;
                state.symbol_changes = None;
                state.merged_base = false;
                *self.per_file_state() = state;
                return Ok(None);
            }
        };
        let mut exact = state.merged_base && update.changed_rows.is_some();
        if state.reuse.verify && !update.full {
            let mut fresh = crate::passes::Merged::default();
            let rebuilt = fresh.update(
                works.iter().map(crate::passes::WorkId::of).collect(),
                outputs.clone(),
                false,
            );
            if rebuilt.is_err() || !fresh.same_as(&state.merged) {
                reuse_mismatch("the merged keys and rows");
                exact = false;
                state.merged = fresh;
                if rebuilt.is_err() {
                    state.reuse.clear();
                    state.merged = crate::passes::Merged::default();
                    state.symbol_changes = None;
                    state.merged_base = false;
                    *self.per_file_state() = state;
                    return Ok(None);
                }
            }
        }
        self.ids.set_table_keys(Some(state.merged.keys()));
        state.merged.serving = true;
        state.stats.rows = state.merged.row_count();
        state.stats.rows_verified += update.rows_verified;
        state.stats.row_mismatches += update.row_mismatches;
        let mut reuse = crate::references::BinderRetention::default();
        for (output, recomputed) in outputs.iter().zip(&recomputed) {
            let reused = !recomputed;
            let symbols = output.result.as_ref().map_or(0, |result| {
                result
                    .entities
                    .iter()
                    .filter(|entity| !entity.symbol.is_empty())
                    .count()
            });
            let references = usize::from(output.references.is_some());
            if reused {
                reuse.reused_reference_files += references;
                reuse.reused_symbols += symbols;
            } else {
                reuse.scanned_reference_files += references;
                reuse.resolved_symbols += symbols;
            }
        }
        self.references.note_pass_reuse(reuse);
        for output in &outputs {
            for (family, count) in &output.unavailable.0 {
                *self.unavailable.0.entry(family).or_default() += count;
            }
        }
        // The reference index: patched by the changed contributions while
        // the program's paths are the same, else built from every one.
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        let kept = self
            .references
            .take_pass_index()
            .or_else(|| state.stashed_index.take());
        state.stashed_current = false;
        let patched = kept.and_then(|mut index| {
            if update.full || index.paths() != sorted_paths.as_slice() {
                return None;
            }
            for (old, new) in &update.changed {
                if let Some((path, references)) =
                    old.as_ref().and_then(|old| old.references.as_ref())
                    && !index.patch(path, Some(references), None)
                {
                    return None;
                }
                if let Some((path, references)) = &new.references
                    && !index.patch(path, None, Some(references))
                {
                    return None;
                }
            }
            Some(index)
        });
        exact &= patched.is_some();
        let build = |paths: Vec<String>| {
            crate::references::PassIndex::build(
                paths,
                outputs.iter().filter_map(|output| {
                    output
                        .references
                        .as_ref()
                        .map(|(path, references)| (path.as_str(), references))
                }),
            )
        };
        let index = match patched {
            Some(index) if state.reuse.verify => {
                let rebuilt = build(paths);
                if index.same_answers(&rebuilt) {
                    index
                } else {
                    reuse_mismatch("the merged reference index");
                    exact = false;
                    rebuilt
                }
            }
            Some(index) => index,
            None => build(paths),
        };
        self.references.install_pass_index(index);
        state.symbol_changes = exact.then(|| {
            let mut buckets = HashSet::new();
            for (old, new) in &update.changed {
                let old = old.as_ref().and_then(|old| old.references.as_ref());
                if old.is_some() || new.references.is_some() {
                    changed_buckets(
                        old.map(|(_, references)| references),
                        new.references.as_ref().map(|(_, references)| references),
                        &mut buckets,
                    );
                }
            }
            SymbolChanges {
                rows: update.changed_rows.clone().unwrap_or_default(),
                buckets,
                answered: None,
            }
        });
        state.merged_base = true;
        let mut results: Vec<SemanticDemandRunResult> = outputs[..runs.len()]
            .iter()
            .map(|output| output.result.clone().unwrap_or_default())
            .collect();
        let deferred: Vec<&Deferrals> = outputs[..runs.len()]
            .iter()
            .map(|output| &output.deferrals)
            .collect();
        let completions =
            self.complete_deferred(program, &mut results, &deferred, &mut state.stats);
        let facts = crate::source_facts::merge_async_facts(
            outputs[..async_count]
                .iter()
                .map(|output| output.facts.clone())
                .collect(),
        );
        self.record_versions(
            &mut state,
            &works[..runs.len()],
            &outputs[..runs.len()],
            completions,
            &facts,
        );
        let merge = started.elapsed();
        if env_is_one(ENV_STATS) {
            eprintln!(
                "per-file stats (table): {:?}; refresh {refresh:?} pool {pool:?} merge {merge:?}; leaked file checkers {}",
                state.stats,
                crate::unsafe_boundary::leaked_file_checkers()
            );
        }
        if let Some(trace) = &self.trace {
            let count = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
            let nanos =
                |value: std::time::Duration| i64::try_from(value.as_nanos()).unwrap_or(i64::MAX);
            let stats = state.stats;
            trace.metrics(
                "per-file",
                &[
                    ("passes", count(stats.passes)),
                    ("demandedPasses", count(stats.demanded_passes)),
                    ("referencePasses", count(stats.reference_passes)),
                    ("rows", count(stats.rows)),
                    ("rowMismatches", count(stats.row_mismatches)),
                    ("reused", count(stats.reuse.reused)),
                    ("ran", count(stats.reuse.ran)),
                    ("invalidated", count(stats.reuse.invalidated)),
                    ("workers", count(stats.reuse.workers)),
                    ("reuseMismatches", count(stats.reuse.mismatches)),
                    ("refreshNs", nanos(refresh)),
                    ("poolNs", nanos(pool)),
                    ("mergeNs", nanos(merge)),
                ],
            );
        }
        *self.per_file_state() = state;
        Ok(Some((results, facts)))
    }

    /// Advances the version of every path whose rows (its run's entities,
    /// its async facts) differ from what the previous table served.
    fn record_versions(
        &mut self,
        state: &mut PerFile,
        works: &[crate::passes::PassWork<'_>],
        outputs: &[Arc<crate::passes::PassOutput>],
        completions: Vec<Vec<Completion>>,
        facts: &[AsyncFunctionFact],
    ) {
        let mut runs: HashMap<String, (Arc<crate::passes::PassOutput>, Vec<Completion>)> =
            HashMap::with_capacity(works.len());
        for ((work, output), completion) in works.iter().zip(outputs).zip(completions) {
            runs.insert(work.path.clone(), (Arc::clone(output), completion));
        }
        let mut by_path: HashMap<String, Vec<AsyncFunctionFact>> = HashMap::new();
        for fact in facts {
            by_path
                .entry(clean(&fact.expression.path))
                .or_default()
                .push(fact.clone());
        }
        let mut every: HashSet<String> = HashSet::new();
        every.extend(state.served_runs.keys().cloned());
        every.extend(state.served_async.keys().cloned());
        every.extend(runs.keys().cloned());
        every.extend(by_path.keys().cloned());
        if !state.versions_valid {
            state.versions.clear();
        }
        for path in every {
            let same_run = match (state.served_runs.get(&path), runs.get(&path)) {
                (None, None) => true,
                (Some(before), Some(now)) => {
                    (Arc::ptr_eq(&before.0, &now.0) || *before.0 == *now.0) && before.1 == now.1
                }
                _ => false,
            };
            let same_async = state.served_async.get(&path) == by_path.get(&path);
            let present = runs.contains_key(&path) || by_path.contains_key(&path);
            if !present {
                state.versions.remove(&path);
            } else if !same_run || !same_async || !state.versions.contains_key(&path) {
                state.next_version += 1;
                state.versions.insert(path, state.next_version);
            }
        }
        state.served_runs = runs;
        state.served_async = by_path;
        state.versions_valid = true;
    }

    /// Per-file passes with reuse: a number that changes exactly when the
    /// rows of `path` (its entities and async facts) differ from the
    /// previous table's, `None` for a path without rows. `None` for every
    /// path when the last table did not track them (the sequential run).
    #[must_use]
    pub fn per_file_content_version(&self, path: &str) -> Option<Option<u64>> {
        let state = &self.per_file;
        if !state.versions_valid {
            return None;
        }
        Some(state.versions.get(path).copied())
    }

    /// Completes the answers of `results` that read every file (`Deferrals`,
    /// index-aligned), from the merged reference index and the program's
    /// syntax only; returns, per result, how each was completed.
    fn complete_deferred(
        &mut self,
        program: &'static tsrs_compiler::Program,
        results: &mut [SemanticDemandRunResult],
        deferred: &[&Deferrals],
        stats: &mut Stats,
    ) -> Vec<Vec<Completion>> {
        let structural: HashSet<SymbolId> = deferred
            .iter()
            .flat_map(|deferrals| deferrals.structural.iter().cloned())
            .collect();
        let mut completions = Vec::with_capacity(results.len());
        for (result, deferrals) in results.iter_mut().zip(deferred) {
            let mut completion = Vec::new();
            for (index, id) in &deferrals.spaces {
                let space = self.references.merged_space(id);
                result.entities[*index].reference_space = Some(space);
                completion.push(Completion::Space(space));
                stats.deferred_spaces += 1;
            }
            for (index, id) in &deferrals.library {
                stats.deferred_library += 1;
                let written = self.references.query_merged(id).iter().any(|reference| {
                    let Some(source) = program.get_source_file(&reference.path) else {
                        return true;
                    };
                    u32::try_from(reference.start_byte)
                        .map_or(true, |start| reference_is_write(source, start))
                });
                completion.push(Completion::Withdrawn(written));
                if written && let Some(call) = &mut result.entities[*index].resolved_call {
                    Arc::make_mut(call).default_library_binding = None;
                    stats.withdrawn_library += 1;
                }
            }
            // The canonical run suppresses the descriptor of an entity whose
            // symbol is a structural accessor anywhere in the batch.
            for (index, entity) in result.entities.iter_mut().enumerate() {
                if entity.type_descriptor.is_some()
                    && !entity.symbol.is_empty()
                    && structural.contains(&entity.symbol)
                    && !deferrals.structural.contains(&entity.symbol)
                {
                    entity.type_descriptor = None;
                    completion.push(Completion::Suppressed(index));
                    stats.suppressed_descriptors += 1;
                }
            }
            completions.push(completion);
        }
        completions
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
            match state.row(id).cloned() {
                Some(row) => {
                    state.stats.row_queries += 1;
                    rows.insert(id.clone(), row);
                }
                None => {
                    if !missing.contains(id) {
                        missing.push(id.clone());
                    }
                }
            }
        }
        self.references.note_symbol_rows(rows.len(), missing.len());
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
            state.scratch_ids.insert(id.clone());
            if !row.found {
                state.stats.unresolved_queries += 1;
                if env_is_one(ENV_STATS) {
                    eprintln!("per-file unresolved query {id}");
                }
            }
            rows.insert(id, row);
        }
        Ok(rows)
    }

    /// Per-file passes with reuse (instant-edits P4): every id whose
    /// `symbols` answer (row or references) may differ from the one the
    /// previous table gave, or `None` when the last table cannot say
    /// exactly (its first table, a merge that started over, a rebuilt
    /// reference index, the sequential run, a reuse mismatch).
    pub(crate) fn per_file_symbol_changes(&mut self) -> Option<Vec<SymbolId>> {
        let state = &mut self.per_file;
        let changes = state.symbol_changes.as_mut()?;
        if changes.answered.is_none() {
            let mut answered: HashSet<SymbolId> = changes.rows.iter().cloned().collect();
            answered.extend(state.scratch_ids.iter().cloned());
            if !changes.buckets.is_empty() {
                answered.extend(state.merged.ids_with_canonical(&changes.buckets));
            }
            let mut answered: Vec<SymbolId> = answered.into_iter().collect();
            answered.sort_unstable();
            changes.answered = Some(answered);
        }
        changes.answered.clone()
    }

    /// The merged reference index, rebuilt per file if a release dropped it.
    pub(crate) fn ensure_references_per_file(&mut self, ctx: &Context) -> Result<(), BackendError> {
        if self.references.has_merged() {
            return Ok(());
        }
        let state = self.per_file_state();
        if state.stashed_current
            && let Some(index) = state.stashed_index.take()
        {
            state.stashed_current = false;
            self.references.install_pass_index(index);
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
