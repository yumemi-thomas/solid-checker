//! Per-file passes as functions of what their checker read (instant-edits
//! P3): a pass's pointer-free output is reused while everything it can have
//! read is unchanged, and the passes that must run do so on a pool of worker
//! threads (ADR 03XX-p6); `per_file` drives it.
//!
//! # A pass
//!
//! [`run_pass`] computes one file's facts under a fresh checker of its own
//! (`unsafe_boundary` invariant 8) and an identity state of its own: the
//! file's own check first, its async facts, its demand run (with the answers that
//! read every file deferred, `semantic_runs::Deferrals`), its reference-index
//! contribution and the rows of every id its facts name, plus the keys it
//! minted. Nothing in the output points into the checker. A pass reads no
//! state another pass wrote: its keys start empty, its reference index and
//! input digests are its own, and the program-wide syntactic census of
//! property writes is handed in and every question asked of it recorded.
//!
//! # The read set
//!
//! A checker reaches a declaration only through the file's scope chain:
//! its locals, its module imports, and the global scope. So what a pass can
//! read is contained in
//!
//! * the forward closure of the file over every module resolution the
//!   program recorded for it (`Program::get_resolved_modules`: static and
//!   dynamic imports, `import()` types, `require` in JS, module augmentation
//!   names, the JSX runtime and import-helper imports) and over its type
//!   reference directives;
//! * the forward closure, over the same edges, of every file that contributes
//!   to the global scope (`inputs::ProgramInputs::is_ambient`: scripts,
//!   default libraries, `declare global`, module augmentations, UMD
//!   exports), because the checker merges every global contributor and every
//!   augmentation at creation, whichever file it then checks.
//!
//! A `/// <reference path>` adds a file to the program, not to the
//! referencing file's scope: a referenced script is ambient, a referenced
//! module is reached only by an import. Per path in the read set, a pass also
//! depends on `Exports(path)` (which of the path's declarations a module
//! exports decides `Export` against `Decl` keys of the symbols declared
//! there). Beyond the read set it depends on the compiler options, the set of
//! global contributors, the census answers it recorded, and its own inputs
//! (demands, async locations).
//!
//! # Reuse
//!
//! [`Reuse`] keeps the last output of every path with its read set (a bit
//! set over the program's paths). At every table it compares each path's
//! `(Text, Exports)` with the previous table's, and drops every entry whose
//! read set holds a changed path. Options or `Ambient` changing drops every
//! entry; so does a program chain that was rebuilt (the project bumps
//! [`Reuse::program_rebuilt`]) and any change of membership or resolution
//! (the graph's digest). [`ENV_VERIFY_REUSE`] recomputes every reused pass and
//! compares it with what reuse would serve.
//!
//! # The pool
//!
//! [`run_works`] runs passes on `N` scoped worker threads (default
//! `min(cores - 2, 12)`, [`ENV_WORKERS`]). Each worker builds its own identity
//! state from a [`crate::identity::PassSeed`] and installs its own file
//! checker per pass. Outputs land by index, so the caller merges them in path
//! order whatever the schedule. The program is shared read-only: its lazily
//! filled caches are primed first (`Generation::prime_caches`); tsrs files,
//! binder symbols and nodes are frozen after binding and its source-file
//! lazies are `OnceLock`s or locked and allocated in the file's own region.

use crate::backend::BackendError;
use crate::binder_key::Key;
use crate::context::Context;
use crate::durable::DurableRef;
use crate::gocompat::{absolute, clean};
use crate::identity::{Env, Identities, PassSeed, Preserved, TableKeys};
use crate::inputs::{Digest, ProgramInputs};
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SymbolId};
use crate::per_file::{Row, row_for, seeds};
use crate::references::{FileReferences, ReferenceIndex};
use crate::resolved_calls::{PropertyWrites, file_property_writes};
use crate::semantic_runs::{Deferrals, Unavailable};
use crate::unsafe_boundary::Generation;
use sha2::{Digest as _, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tsrs_compiler::Program;
use typefacts::v3::EntityDemand;
use typefacts::{AsyncFunctionFact, Location};

/// Worker threads for the passes (`1`: on the calling thread).
pub const ENV_WORKERS: &str = "SOLID_TYPEFACTS_PER_FILE_WORKERS";

/// `1`: every reused pass is recomputed and compared with the reused output.
/// A difference is counted and reported on stderr, the fresh output is
/// served (fail closed), and a test build panics.
pub const ENV_VERIFY_REUSE: &str = "SOLID_TYPEFACTS_PER_FILE_VERIFY_REUSE";

/// `1`: no pass output is reused (every pass runs every table).
pub const ENV_NO_REUSE: &str = "SOLID_TYPEFACTS_PER_FILE_NO_REUSE";

fn env_is_one(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

/// How many worker threads run passes.
#[must_use]
pub fn workers_from_env() -> usize {
    if cfg!(target_arch = "wasm32") {
        return 1;
    }
    if let Some(count) = std::env::var(ENV_WORKERS)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        return count.max(1);
    }
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .saturating_sub(2)
        .clamp(1, 12)
}

/// One pass to run: a demanded file (`run`), the async facts of locations
/// outside every run, or a non-demanded file's reference contribution.
pub(crate) struct PassWork<'a> {
    /// The run's path, or the file's clean name.
    pub path: String,
    pub run: Option<&'a SemanticDemandRun<'a>>,
    pub locations: Vec<Location>,
    /// Whether the file's reference contribution is part of the pass.
    pub scan: bool,
    /// Whether the output may be kept for a later table.
    pub reusable: bool,
}

/// What one pass computed. Pointer-free.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PassOutput {
    /// The run's result, before the deferred answers are completed.
    pub result: Option<SemanticDemandRunResult>,
    pub facts: Vec<AsyncFunctionFact>,
    /// `library_writes` is always `None` here.
    pub deferrals: Deferrals,
    /// The file's reference contribution, by its clean name.
    pub references: Option<(String, FileReferences)>,
    /// The rows of every id the facts name (and their alias targets), by id.
    pub rows: Vec<(SymbolId, Row)>,
    /// The keys the pass minted, in id order.
    pub keys: Vec<(SymbolId, Arc<Key>)>,
    pub unavailable: Unavailable,
}

impl PassOutput {
    /// This pass's row of `id`.
    #[must_use]
    pub(crate) fn row(&self, id: &SymbolId) -> Option<&Row> {
        self.rows
            .binary_search_by(|(held, _)| held.cmp(id))
            .ok()
            .map(|at| &self.rows[at].1)
    }
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

/// The rows of `seeds` and of every alias target they reach, in this pass.
fn rows_in_pass(env: &mut Env<'_>, seeds: Vec<SymbolId>) -> Vec<(SymbolId, Row)> {
    let mut pending = seeds;
    let mut visited: HashSet<SymbolId> = HashSet::new();
    let mut rows = Vec::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let row = row_for(env, &id);
        if !row.alias_target.is_empty() {
            pending.push(row.alias_target.clone());
        }
        rows.push((id, row));
    }
    rows
}

/// Runs one pass on this thread, under a fresh file checker, with `ids` as
/// the pass's identity state (its keys are taken, its pointer maps purged).
pub(crate) fn run_pass(
    ctx: &Context,
    generation: &Generation,
    census: Option<&PropertyWrites>,
    ids: &mut Identities,
    work: &PassWork<'_>,
) -> Result<PassOutput, BackendError> {
    check(ctx)?;
    let program = generation.program();
    let file = absolute(&work.path)
        .ok()
        .and_then(|path| program.get_source_file(&path));
    let mut references = ReferenceIndex::for_pass();
    let mut inputs = ProgramInputs::default();
    let mut unavailable = Unavailable::default();
    let scope = generation.file_checker();
    let outcome = (|| -> Result<PassOutput, BackendError> {
        if work.run.is_some()
            && let Some(file) = file
        {
            // The file's own check first, under its checker, so every answer
            // inside the file is the one tsc's source-order check computes.
            let _ = generation.semantic_diagnostics(file);
            check(ctx)?;
        }
        generation.with_checker(|checker| {
            let mut env = Env {
                checker,
                program,
                ids: &mut *ids,
                inputs: &mut inputs,
            };
            let facts = if work.locations.is_empty() {
                Vec::new()
            } else {
                crate::source_facts::async_facts_in_pass(ctx, &mut env, &work.locations)?
            };
            let mut deferrals = Deferrals {
                library_writes: census.cloned(),
                ..Deferrals::default()
            };
            let result = match work.run {
                Some(run) => Some(crate::semantic_runs::run(
                    ctx,
                    &mut env,
                    run,
                    &mut unavailable,
                    &mut deferrals,
                )?),
                None => None,
            };
            deferrals.library_writes = None;
            let contribution = match file {
                Some(file) if work.scan && !file.is_declaration_file() => {
                    let path = clean(file.file_name());
                    references.scan_file(&mut env, file);
                    references.take_file(&path).map(|found| (path, found))
                }
                _ => None,
            };
            let mut rows = match &result {
                Some(result) => rows_in_pass(&mut env, seeds(result, &facts)),
                None => Vec::new(),
            };
            // By id: `PassOutput::row` searches them.
            rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            Ok(PassOutput {
                result,
                facts,
                deferrals,
                references: contribution,
                rows,
                keys: Vec::new(),
                unavailable: Unavailable::default(),
            })
        })
    })();
    // Invariant 1 at the drop: nothing may point into the region once it is
    // freed, so the maps go first.
    ids.purge_scope_pointers();
    let keys = ids.take_keys();
    drop(scope);
    let mut output = outcome?;
    output.keys = keys;
    output.unavailable = unavailable;
    Ok(output)
}

/// What [`run_works`] hands back: each work's outcome, by index, and the
/// runtime paths the workers resolved.
pub(crate) struct WorkOutcome {
    pub outputs: Vec<Result<PassOutput, BackendError>>,
    pub runtime_paths: Vec<HashMap<String, String>>,
}

/// Stack of a pass worker: the main thread's (the checker recurses deeply).
const WORKER_STACK: usize = 16 << 20;

/// Runs `works` on `workers` threads (or on this one for a single worker or
/// a single work). Outputs are index-aligned with `works`.
pub(crate) fn run_works(
    ctx: &Context,
    generation: &Generation,
    seed: &PassSeed,
    census: Option<&PropertyWrites>,
    works: &[&PassWork<'_>],
    workers: usize,
) -> WorkOutcome {
    let workers = workers.min(works.len());
    if workers <= 1 {
        let mut ids = Identities::from_seed(seed.clone());
        let outputs = works
            .iter()
            .map(|work| run_pass(ctx, generation, census, &mut ids, work))
            .collect();
        return WorkOutcome {
            outputs,
            runtime_paths: vec![ids.take_runtime_paths()],
        };
    }
    // The program's lazily filled caches land in their own region before
    // any worker's checker exists.
    generation.prime_caches();
    let next = AtomicUsize::new(0);
    type Done = (
        Vec<(usize, Result<PassOutput, BackendError>)>,
        HashMap<String, String>,
    );
    let done: Vec<Done> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                std::thread::Builder::new()
                    .name("typefacts-pass".to_owned())
                    .stack_size(WORKER_STACK)
                    .spawn_scoped(scope, || {
                        let mut ids = Identities::from_seed(seed.clone());
                        let mut local = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(work) = works.get(index) else {
                                break;
                            };
                            local.push((index, run_pass(ctx, generation, census, &mut ids, work)));
                        }
                        (local, ids.take_runtime_paths())
                    })
            })
            .collect();
        let mut done = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle {
                Ok(handle) => match handle.join() {
                    Ok(result) => done.push(result),
                    Err(panic) => std::panic::resume_unwind(panic),
                },
                // A thread that could not start: its share runs here.
                Err(_) => {
                    let mut ids = Identities::from_seed(seed.clone());
                    let mut local = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(work) = works.get(index) else {
                            break;
                        };
                        local.push((index, run_pass(ctx, generation, census, &mut ids, work)));
                    }
                    done.push((local, ids.take_runtime_paths()));
                }
            }
        }
        done
    });
    let mut outputs: Vec<Option<Result<PassOutput, BackendError>>> =
        (0..works.len()).map(|_| None).collect();
    let mut runtime_paths = Vec::with_capacity(done.len());
    for (local, paths) in done {
        for (index, output) in local {
            outputs[index] = Some(output);
        }
        runtime_paths.push(paths);
    }
    WorkOutcome {
        outputs: outputs
            .into_iter()
            .map(|output| output.expect("every work ran"))
            .collect(),
        runtime_paths,
    }
}

// --- read sets -------------------------------------------------------------

type Bits = Vec<u64>;

fn bits_new(len: usize) -> Bits {
    vec![0; len.div_ceil(64)]
}

fn bit_set(bits: &mut Bits, index: u32) {
    bits[index as usize / 64] |= 1 << (index % 64);
}

fn bit_get(bits: &Bits, index: u32) -> bool {
    bits.get(index as usize / 64)
        .is_some_and(|word| word & (1 << (index % 64)) != 0)
}

fn bits_intersect(left: &Bits, right: &Bits) -> bool {
    left.iter().zip(right).any(|(a, b)| a & b != 0)
}

/// The program's files and resolved edges, declaration files included.
pub(crate) struct Graph {
    paths: Vec<String>,
    ids: HashMap<String, u32>,
    edges: Vec<Vec<u32>>,
    /// Membership and every edge, so a change of either is seen.
    digest: Digest,
}

impl Graph {
    pub(crate) fn build(program: &Program) -> Self {
        let mut paths: Vec<String> = program
            .source_files()
            .iter()
            .map(|file| clean(file.file_name()))
            .collect();
        paths.sort();
        paths.dedup();
        let ids: HashMap<String, u32> = paths
            .iter()
            .enumerate()
            .map(|(index, path)| (path.clone(), u32::try_from(index).unwrap_or(u32::MAX)))
            .collect();
        // Resolutions are keyed by the importing file's `Path`.
        let by_key: HashMap<&str, u32> = program
            .source_files()
            .iter()
            .filter_map(|file| {
                ids.get(&clean(file.file_name()))
                    .map(|id| (file.path().as_str(), *id))
            })
            .collect();
        let mut edges: Vec<Vec<u32>> = vec![Vec::new(); paths.len()];
        let target = |resolved: &str| -> Option<u32> {
            if resolved.is_empty() {
                return None;
            }
            let file = program.get_source_file_for_resolved_module(resolved)?;
            ids.get(&clean(file.file_name())).copied()
        };
        program.for_each_resolved_module(
            &mut |resolution, _, _, from| {
                if let (Some(from), Some(to)) = (
                    by_key.get(from.as_str()),
                    target(resolution.resolved_file_name),
                ) {
                    edges[*from as usize].push(to);
                }
            },
            None,
        );
        program.for_each_resolved_type_reference_directive(
            &mut |resolution, _, _, from| {
                if let (Some(from), Some(to)) = (
                    by_key.get(from.as_str()),
                    target(resolution.resolved_file_name),
                ) {
                    edges[*from as usize].push(to);
                }
            },
            None,
        );
        let mut hasher = Sha256::new();
        for (index, list) in edges.iter_mut().enumerate() {
            list.sort_unstable();
            list.dedup();
            hasher.update(paths[index].as_bytes());
            hasher.update([0]);
            for to in list.iter() {
                hasher.update(to.to_le_bytes());
            }
            hasher.update([1]);
        }
        Self {
            paths,
            ids,
            edges,
            digest: hasher.finalize().into(),
        }
    }

    /// `seeds` and every file they reach, added to `bits`.
    fn close(&self, seeds: impl IntoIterator<Item = u32>, bits: &mut Bits) {
        let mut pending: Vec<u32> = Vec::new();
        for seed in seeds {
            if !bit_get(bits, seed) {
                bit_set(bits, seed);
                pending.push(seed);
            }
        }
        while let Some(at) = pending.pop() {
            for to in &self.edges[at as usize] {
                if !bit_get(bits, *to) {
                    bit_set(bits, *to);
                    pending.push(*to);
                }
            }
        }
    }
}

// --- reuse -----------------------------------------------------------------

/// A kept pass output with what it was computed from.
struct Entry {
    /// `None` for a pass without a run.
    demands: Option<Vec<EntityDemand>>,
    locations: Vec<Location>,
    scan: bool,
    reads: Bits,
    output: Arc<PassOutput>,
}

impl Entry {
    fn serves(&self, work: &PassWork<'_>) -> bool {
        self.scan == work.scan
            && self.locations == work.locations
            && match (&self.demands, work.run) {
                (None, None) => true,
                (Some(demands), Some(run)) => demands.as_slice() == run.demands,
                _ => false,
            }
    }
}

/// Counters of reuse, per table.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReuseStats {
    /// Passes served from a kept output.
    pub reused: usize,
    /// Passes run.
    pub ran: usize,
    /// Kept outputs dropped at this table because something they read changed.
    pub invalidated: usize,
    /// Whether every kept output was dropped (rebuild, options, `Ambient`,
    /// membership or resolution).
    pub cleared: bool,
    /// Paths whose `(Text, Exports)` changed since the previous table.
    pub changed_paths: usize,
    /// Reused passes recomputed by [`ENV_VERIFY_REUSE`], and how many differed.
    pub verified: usize,
    pub mismatches: usize,
    /// Worker threads used.
    pub workers: usize,
}

/// Pass outputs kept across tables, with what decides their validity.
pub(crate) struct Reuse {
    enabled: bool,
    pub(crate) verify: bool,
    /// Bumped at every program change that did not reuse the chain's parse
    /// (a rebuild, a rebase): every kept output is dropped.
    chain: u64,
    graph_chain: u64,
    graph: Option<Graph>,
    options: Option<Digest>,
    ambient: Option<Digest>,
    ambient_reads: Bits,
    /// `(Text, Exports)` per path id at the previous table.
    snapshot: Vec<(Option<Digest>, Digest)>,
    entries: HashMap<String, Entry>,
    /// Each file's half of the census, by its text.
    census_files: HashMap<String, (Digest, Arc<HashSet<String>>)>,
    census: Option<PropertyWrites>,
    pub(crate) stats: ReuseStats,
}

impl Default for Reuse {
    fn default() -> Self {
        Self {
            enabled: !env_is_one(ENV_NO_REUSE),
            verify: env_is_one(ENV_VERIFY_REUSE),
            chain: 0,
            graph_chain: 0,
            graph: None,
            options: None,
            ambient: None,
            ambient_reads: Vec::new(),
            snapshot: Vec::new(),
            entries: HashMap::new(),
            census_files: HashMap::new(),
            census: None,
            stats: ReuseStats::default(),
        }
    }
}

impl Reuse {
    /// The program chain was rebuilt (not a parse-reusing update).
    pub(crate) fn program_rebuilt(&mut self) {
        self.chain += 1;
    }

    /// Drops every kept output.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.stats.cleared = true;
    }

    #[cfg(test)]
    pub(crate) fn set_enabled_for_test(&mut self, enabled: bool, verify: bool) {
        self.enabled = enabled;
        self.verify = verify;
    }

    /// Starts a table: drops every kept output whose read set changed.
    pub(crate) fn refresh(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
        exported: &HashMap<DurableRef, Preserved>,
    ) {
        self.stats = ReuseStats::default();
        self.census = None;
        let graph = Graph::build(program);
        let same_graph = self.graph_chain == self.chain
            && self
                .graph
                .as_ref()
                .is_some_and(|held| held.digest == graph.digest);
        if !same_graph {
            self.clear();
            self.snapshot.clear();
        }
        self.graph_chain = self.chain;
        let options = inputs.options(program);
        let ambient = inputs.ambient(program);
        if self.options != Some(options) || self.ambient != Some(ambient) {
            self.clear();
        }
        self.options = Some(options);
        self.ambient = Some(ambient);
        let mut snapshot = Vec::with_capacity(graph.paths.len());
        for path in &graph.paths {
            snapshot.push((
                inputs.text_at(program, path),
                inputs.exports(exported, path),
            ));
        }
        let mut changed = bits_new(graph.paths.len());
        let mut changed_count = 0;
        for (index, now) in snapshot.iter().enumerate() {
            if self.snapshot.get(index) != Some(now) {
                bit_set(&mut changed, u32::try_from(index).unwrap_or(u32::MAX));
                changed_count += 1;
            }
        }
        self.stats.changed_paths = changed_count;
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| !bits_intersect(&entry.reads, &changed));
        self.stats.invalidated = before - self.entries.len();
        self.snapshot = snapshot;
        let mut ambient_reads = bits_new(graph.paths.len());
        let ambient_files: Vec<u32> = program
            .source_files()
            .iter()
            .filter(|file| inputs.is_ambient(**file))
            .filter_map(|file| graph.ids.get(&clean(file.file_name())).copied())
            .collect();
        graph.close(ambient_files, &mut ambient_reads);
        self.ambient_reads = ambient_reads;
        self.graph = Some(graph);
        if !self.enabled {
            self.entries.clear();
        }
    }

    /// The program's property-write census, from each file's memoized half.
    pub(crate) fn census(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
    ) -> PropertyWrites {
        if let Some(census) = &self.census {
            return Arc::clone(census);
        }
        let mut writes: HashSet<String> = HashSet::new();
        let mut present: HashSet<String> = HashSet::new();
        for file in program.source_files() {
            if file.is_declaration_file() {
                continue;
            }
            let path = clean(file.file_name());
            let text = inputs.text(*file);
            let half = match self.census_files.get(&path) {
                Some((digest, half)) if *digest == text => Arc::clone(half),
                _ => {
                    let half = Arc::new(file_property_writes(*file));
                    self.census_files
                        .insert(path.clone(), (text, Arc::clone(&half)));
                    half
                }
            };
            writes.extend(half.iter().cloned());
            present.insert(path);
        }
        self.census_files.retain(|path, _| present.contains(path));
        let census = Arc::new(writes);
        self.census = Some(Arc::clone(&census));
        census
    }

    /// The kept output that serves `work`, if every census answer it recorded
    /// still holds.
    pub(crate) fn lookup(
        &mut self,
        work: &PassWork<'_>,
        program: &Program,
        inputs: &mut ProgramInputs,
    ) -> Option<Arc<PassOutput>> {
        if !work.reusable {
            return None;
        }
        let entry = self.entries.get(&work.path)?;
        if !entry.serves(work) {
            return None;
        }
        let output = Arc::clone(&entry.output);
        if !output.deferrals.census.is_empty() {
            let census = self.census(program, inputs);
            if output
                .deferrals
                .census
                .iter()
                .any(|(name, written)| census.contains(name) != *written)
            {
                self.entries.remove(&work.path);
                self.stats.invalidated += 1;
                return None;
            }
        }
        Some(output)
    }

    /// Keeps `output` for `work`'s path, with its read set.
    pub(crate) fn keep(&mut self, work: &PassWork<'_>, output: &Arc<PassOutput>) {
        if !self.enabled || !work.reusable {
            return;
        }
        let Some(graph) = &self.graph else {
            return;
        };
        let Some(id) = graph.ids.get(&clean(&work.path)).copied() else {
            // Not a program file: its answer is its absence, a membership
            // fact; nothing is kept.
            return;
        };
        let mut reads = self.ambient_reads.clone();
        graph.close([id], &mut reads);
        self.entries.insert(
            work.path.clone(),
            Entry {
                demands: work.run.map(|run| run.demands.to_vec()),
                locations: work.locations.clone(),
                scan: work.scan,
                reads,
                output: Arc::clone(output),
            },
        );
    }

    /// The read set of `path` at this table, as paths (tests).
    #[cfg(test)]
    pub(crate) fn read_set_for_test(&self, path: &str) -> Vec<String> {
        let Some(graph) = &self.graph else {
            return Vec::new();
        };
        let Some(id) = graph.ids.get(path).copied() else {
            return Vec::new();
        };
        let mut reads = self.ambient_reads.clone();
        graph.close([id], &mut reads);
        graph
            .paths
            .iter()
            .enumerate()
            .filter(|(index, _)| bit_get(&reads, u32::try_from(*index).unwrap_or(u32::MAX)))
            .map(|(_, path)| path.clone())
            .collect()
    }
}

// --- merge -----------------------------------------------------------------

/// What a work position means across tables: its outputs are compared
/// position by position only while the work list is the same.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkId {
    path: String,
    run: bool,
    scan: bool,
    reusable: bool,
}

impl WorkId {
    pub(crate) fn of(work: &PassWork<'_>) -> Self {
        Self {
            path: work.path.clone(),
            run: work.run.is_some(),
            scan: work.scan,
            reusable: work.reusable,
        }
    }
}

/// The table's pass outputs merged, kept across tables and patched by the
/// positions whose output changed: the union of their keys (one id, one key,
/// or a collision), and per id the positions whose pass computed its row
/// (the first in path order answers).
#[derive(Default)]
pub(crate) struct Merged {
    works: Vec<WorkId>,
    outputs: Vec<Arc<PassOutput>>,
    keys: Arc<TableKeys>,
    providers: HashMap<SymbolId, Vec<u32>>,
    /// Whether the rows answer queries (false from a program change until
    /// the next table).
    pub(crate) serving: bool,
}

/// One merge: whether it started over, and each changed position's old and
/// new output.
pub(crate) struct MergeUpdate {
    pub full: bool,
    pub changed: Vec<(Option<Arc<PassOutput>>, Arc<PassOutput>)>,
    /// Rows a later provider computed again, and how many differed
    /// (`per_file::ENV_VERIFY_ROWS`).
    pub rows_verified: usize,
    pub row_mismatches: usize,
    /// Every id whose served row ([`Merged::row`]) differs from the one the
    /// previous table served (gained, lost or changed); `None` for a merge
    /// that started over.
    pub changed_rows: Option<Vec<SymbolId>>,
}

impl Merged {
    /// Whether `other` merged the same keys and rows (an incrementally
    /// updated merge against a fresh one, `ENV_VERIFY_REUSE`).
    pub(crate) fn same_as(&self, other: &Merged) -> bool {
        self.works == other.works
            && *self.keys == *other.keys
            && self.providers == other.providers
            && self.outputs.len() == other.outputs.len()
            && self
                .outputs
                .iter()
                .zip(&other.outputs)
                .all(|(left, right)| Arc::ptr_eq(left, right))
    }

    /// The row of `id` from the first pass in path order that computed one.
    #[must_use]
    pub(crate) fn row(&self, id: &SymbolId) -> Option<&Row> {
        let first = *self.providers.get(id)?.first()?;
        self.outputs.get(first as usize)?.row(id)
    }

    /// Every id whose served row's alias-canonical id is in `canonical`.
    pub(crate) fn ids_with_canonical(&self, canonical: &HashSet<SymbolId>) -> Vec<SymbolId> {
        self.providers
            .keys()
            .filter(|id| {
                self.row(id)
                    .is_some_and(|row| canonical.contains(&row.canonical))
            })
            .cloned()
            .collect()
    }

    /// Ids with a row.
    #[must_use]
    pub(crate) fn row_count(&self) -> usize {
        self.providers.len()
    }

    /// The key the merged passes minted for `id`.
    #[must_use]
    pub(crate) fn key(&self, id: &SymbolId) -> Option<&Arc<Key>> {
        self.keys.get(id).map(|(key, _)| key)
    }

    /// The merged keys, to attach to the project's identity state.
    #[must_use]
    pub(crate) fn keys(&self) -> Arc<TableKeys> {
        Arc::clone(&self.keys)
    }

    /// Merges `outputs` (aligned with `works`). The caller has detached the
    /// keys from every identity state (else they are copied once). `Err`
    /// names an id two passes minted for two keys; the state is then
    /// inconsistent and the caller drops it.
    pub(crate) fn update(
        &mut self,
        works: Vec<WorkId>,
        outputs: Vec<Arc<PassOutput>>,
        verify_rows: bool,
    ) -> Result<MergeUpdate, SymbolId> {
        let full = self.works != works;
        if full {
            self.keys = Arc::default();
            self.providers.clear();
            self.outputs.clear();
        }
        let mut changed: Vec<(usize, Option<Arc<PassOutput>>)> = Vec::new();
        for (at, output) in outputs.iter().enumerate() {
            match self.outputs.get(at) {
                Some(old) if Arc::ptr_eq(old, output) => {}
                Some(old) => changed.push((at, Some(Arc::clone(old)))),
                None => changed.push((at, None)),
            }
        }
        // A served row can move only for an id a changed position held or
        // holds: every other id keeps its providers and their outputs.
        // Snapshotted before the merge, compared after it.
        let served_before: Option<Vec<(SymbolId, Option<Row>)>> = (!full).then(|| {
            let mut candidates: Vec<&SymbolId> = changed
                .iter()
                .flat_map(|(at, old)| {
                    old.iter()
                        .flat_map(|old| old.rows.iter())
                        .chain(outputs[*at].rows.iter())
                        .map(|(id, _)| id)
                })
                .collect();
            candidates.sort_unstable();
            candidates.dedup();
            candidates
                .into_iter()
                .map(|id| (id.clone(), self.row(id).cloned()))
                .collect()
        });
        // Every changed position leaves before any enters, so an id moving
        // between passes is never seen twice.
        let keys = Arc::make_mut(&mut self.keys);
        for (at, old) in &changed {
            let Some(old) = old else {
                continue;
            };
            for (id, _) in &old.keys {
                if let Some(entry) = keys.get_mut(id) {
                    entry.1 -= 1;
                    if entry.1 == 0 {
                        keys.remove(id);
                    }
                }
            }
            let at = u32::try_from(*at).unwrap_or(u32::MAX);
            for (id, _) in &old.rows {
                if let Some(positions) = self.providers.get_mut(id) {
                    positions.retain(|position| *position != at);
                    if positions.is_empty() {
                        self.providers.remove(id);
                    }
                }
            }
        }
        for (at, _) in &changed {
            let output = &outputs[*at];
            for (id, key) in &output.keys {
                match keys.get_mut(id) {
                    Some(entry) if entry.0 != *key => return Err(id.clone()),
                    Some(entry) => entry.1 += 1,
                    None => {
                        keys.insert(id.clone(), (Arc::clone(key), 1));
                    }
                }
            }
            let at = u32::try_from(*at).unwrap_or(u32::MAX);
            for (id, _) in &output.rows {
                let positions = self.providers.entry(id.clone()).or_default();
                if let Err(slot) = positions.binary_search(&at) {
                    positions.insert(slot, at);
                }
            }
        }
        self.works = works;
        self.outputs = outputs;
        let changed_rows = served_before.map(|before| {
            before
                .into_iter()
                .filter(|(id, row)| self.row(id) != row.as_ref())
                .map(|(id, _)| id)
                .collect()
        });
        let (mut rows_verified, mut row_mismatches) = (0, 0);
        if verify_rows {
            for (at, _) in &changed {
                for (id, row) in &self.outputs[*at].rows {
                    let Some(first) = self.row(id) else {
                        continue;
                    };
                    if std::ptr::eq(first, row) {
                        continue;
                    }
                    rows_verified += 1;
                    if !crate::per_file::rows_agree(first, row) {
                        row_mismatches += 1;
                    }
                }
            }
        }
        Ok(MergeUpdate {
            full,
            changed: changed
                .into_iter()
                .map(|(at, old)| (old, Arc::clone(&self.outputs[at])))
                .collect(),
            rows_verified,
            row_mismatches,
            changed_rows,
        })
    }
}
