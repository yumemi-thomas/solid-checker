//! Per-file passes as functions of what their checker read (instant-edits
//! P3): a pass's pointer-free output is reused while everything it can have
//! read is unchanged, and the passes that must run do so on a pool of worker
//! threads (ADR 0398); `per_file` drives it.
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
//! `(Text, Exports, Resolutions)` with the previous table's, by path, and
//! drops every entry whose read set holds a changed path, or a path that
//! left the program. `Resolutions(path)` is every module and type-reference
//! resolution the program recorded for the file (specifier, mode, resolved
//! name and package, resolved or not) and the program files they land on.
//! Options or `Ambient` changing drops every entry.
//!
//! This holds across a rebuilt program chain (an edit that changes imports,
//! a deleted or new file, the rebase policy): a kept entry's read set is the
//! closure of its file over the edges of its members, and every member kept
//! its resolutions, so the new program's closure is the same set of paths;
//! a member's text, exports and resolutions are the inputs the pass read
//! from it; and the global contributors (every checker's merged globals and
//! augmentations) are `Ambient`, compared in program order. A file the edit
//! newly imports enters only the read sets of the files that now reach it,
//! each of which reaches the edited file (whose text moved). The entry's
//! read set is computed again over the new graph and must be the same paths
//! (fail closed otherwise). [`ENV_VERIFY_REUSE`] recomputes every reused pass
//! and compares it with what reuse would serve.
//!
//! # The pool
//!
//! [`run_works`] runs passes on `N` scoped worker threads (default
//! [`pool_size`]: every core up to 4, else `cores - 2`, at most 12; [`ENV_WORKERS`]). Each worker builds its own identity
//! state from a [`crate::identity::PassSeed`] and installs its own file
//! checker per pass. Outputs land by index, so the caller merges them in path
//! order whatever the schedule. The program is shared read-only: its lazily
//! filled caches are primed first (`Generation::prime_caches`); tsrs files,
//! binder symbols and nodes are frozen after binding and its source-file
//! lazies are `OnceLock`s or locked and allocated in the file's own region.

use crate::backend::BackendError;
use crate::binder_key::Key;
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::identity::{Env, Identities, PassSeed, TableKeys};
use crate::inputs::{Digest, ProgramInputs};
use crate::model::{SemanticDemandRun, SymbolId};
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

/// With [`ENV_VERIFY_REUSE`]: a file every reuse mismatch is appended to.
pub const ENV_VERIFY_REUSE_LOG: &str = "SOLID_TYPEFACTS_PER_FILE_VERIFY_REUSE_LOG";

/// `1`: no pass output is reused (every pass runs every table).
pub const ENV_NO_REUSE: &str = "SOLID_TYPEFACTS_PER_FILE_NO_REUSE";

fn env_is_one(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| value == "1")
}

/// Whether [`ENV_VERIFY_REUSE`] asks every incremental answer to be checked
/// against its recomputation.
pub(crate) fn verify_reuse() -> bool {
    env_is_one(ENV_VERIFY_REUSE)
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
    pool_size(std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get))
}

/// The pool for `cores` cores: every core up to 4, then two left for the
/// client and the IR beside the producer, at most 12. The calling thread
/// only waits on the pool, so on a small machine leaving cores idle costs
/// more than sharing them (a 4-core runner: 2 workers made an edit 1.47x
/// slower than 4).
#[must_use]
pub fn pool_size(cores: usize) -> usize {
    cores.saturating_sub(2).max(cores.min(4)).clamp(1, 12)
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
    /// The run's entity rows, before the deferred answers complete them.
    pub run: Option<crate::completion::PassRows>,
    pub facts: Vec<AsyncFunctionFact>,
    /// `library_writes` is always `None` here, and the spaces and library
    /// deferrals are the run's patches.
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
            let run = match (work.run, result) {
                (Some(run), Some(result)) => {
                    let patches = crate::completion::Patches::of(&result.entities, &deferrals);
                    deferrals.spaces = Vec::new();
                    deferrals.library = Vec::new();
                    let symbols = result
                        .entities
                        .iter()
                        .filter(|entity| !entity.symbol.is_empty())
                        .count();
                    let durable = result.durable;
                    let rows =
                        crate::contributions::prepare_rows(run.path, run.demands, result.entities)
                            .map_err(BackendError)?;
                    Some(crate::completion::PassRows {
                        rows: rows.into(),
                        durable,
                        symbols,
                        patches,
                    })
                }
                _ => None,
            };
            Ok(PassOutput {
                run,
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
pub(crate) const WORKER_STACK: usize = 16 << 20;

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

/// `bits` over an old index space, in the new one (`moved[old] = new`);
/// `None` when a member has no new index.
fn translate(bits: &Bits, moved: &[u32], len: usize) -> Option<Bits> {
    let mut out = bits_new(len);
    for (word_at, word) in bits.iter().enumerate() {
        let mut word = *word;
        while word != 0 {
            let bit = word.trailing_zeros();
            word &= word - 1;
            let old = word_at * 64 + bit as usize;
            let new = *moved.get(old)?;
            if new == u32::MAX {
                return None;
            }
            bit_set(&mut out, new);
        }
    }
    Some(out)
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
    /// Per path, `Resolutions(path)`: every resolution recorded for the file
    /// and the paths its edges land on.
    resolutions: Vec<Digest>,
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
            resolutions: Vec::new(),
        }
    }

    /// Fills `Resolutions(path)` for every path: every resolution record of
    /// the file, spelled out and sorted (the program iterates them in hash
    /// order), and the paths its edges land on.
    fn compute_resolutions(&mut self, program: &Program) {
        let by_key: HashMap<&str, u32> = program
            .source_files()
            .iter()
            .filter_map(|file| {
                self.ids
                    .get(&clean(file.file_name()))
                    .map(|id| (file.path().as_str(), *id))
            })
            .collect();
        let mut records: Vec<Vec<String>> = vec![Vec::new(); self.paths.len()];
        program.for_each_resolved_module(
            &mut |resolution, name, mode, from| {
                if let Some(from) = by_key.get(from.as_str()) {
                    records[*from as usize].push(format!(
                        "m\0{name}\0{mode:?}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{:?}\0{}",
                        resolution.resolved_file_name,
                        resolution.original_path,
                        resolution.extension,
                        resolution.resolved_using_ts_extension,
                        resolution.resolved_using_extra_extensions,
                        resolution.is_external_library_import,
                        resolution.is_custom_resolution,
                        resolution.package_id,
                        resolution.alternate_result,
                    ));
                }
            },
            None,
        );
        program.for_each_resolved_type_reference_directive(
            &mut |resolution, name, mode, from| {
                if let Some(from) = by_key.get(from.as_str()) {
                    records[*from as usize].push(format!(
                        "t\0{name}\0{mode:?}\0{}\0{}\0{}\0{}\0{:?}",
                        resolution.primary,
                        resolution.resolved_file_name,
                        resolution.original_path,
                        resolution.is_external_library_import,
                        resolution.package_id,
                    ));
                }
            },
            None,
        );
        self.resolutions = records
            .into_iter()
            .zip(&self.edges)
            .map(|(mut file, edges)| {
                file.sort_unstable();
                let mut resolved = Sha256::new();
                for record in &file {
                    resolved.update(record.as_bytes());
                    resolved.update([0xff]);
                }
                // By path, not by index: indexes move with membership.
                for to in edges {
                    resolved.update(self.paths[*to as usize].as_bytes());
                    resolved.update([0]);
                }
                resolved.finalize().into()
            })
            .collect();
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
    /// `None` for a pass without a run: the retained demands when the run
    /// had them (compared by identity first), else a copy.
    demands: Option<Arc<[EntityDemand]>>,
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
                (Some(demands), Some(run)) => {
                    run.shared
                        .is_some_and(|shared| Arc::ptr_eq(shared, demands))
                        || **demands == *run.demands
                }
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
    graph: Option<Graph>,
    /// Whether a program change since the last table did not reuse the
    /// chain's parse (a rebuild, a rebase), so resolutions may have moved;
    /// a parse-reusing update keeps every file's resolutions (tsrs replaces
    /// a file in place only with the same imports).
    rebuilt: bool,
    options: Option<Digest>,
    ambient: Option<Digest>,
    ambient_reads: Bits,
    /// `(Text, Exports, Resolutions)` per path at the previous table.
    snapshot: HashMap<String, PathInputs>,
    entries: HashMap<String, Entry>,
    /// Each file's half of the census, by its text.
    census_files: HashMap<String, (Digest, Arc<HashSet<String>>)>,
    census: Option<PropertyWrites>,
    /// The census kept between tables as counts per name (with the set),
    /// patched by the files a parse-reusing update replaced.
    census_writes: Option<(HashMap<String, u32>, PropertyWrites)>,
    /// The files parse-reusing updates replaced since the last table.
    replaced: HashSet<String>,
    /// The global contributors at the last table, by clean path.
    ambient_paths: HashSet<String>,
    /// The program's non-declaration files, by clean path, in program
    /// order, until the chain is rebuilt.
    program_paths: Option<Vec<String>>,
    pub(crate) stats: ReuseStats,
}

impl Default for Reuse {
    fn default() -> Self {
        Self {
            enabled: !env_is_one(ENV_NO_REUSE),
            verify: env_is_one(ENV_VERIFY_REUSE),
            graph: None,
            rebuilt: true,
            options: None,
            ambient: None,
            ambient_reads: Vec::new(),
            snapshot: HashMap::new(),
            entries: HashMap::new(),
            census_files: HashMap::new(),
            census: None,
            census_writes: None,
            replaced: HashSet::new(),
            ambient_paths: HashSet::new(),
            program_paths: None,
            stats: ReuseStats::default(),
        }
    }
}

/// What a pass can have read from one path: `Text`, `Exports`,
/// `Resolutions`.
type PathInputs = (Option<Digest>, Digest, Digest);

impl Reuse {
    /// The program chain was rebuilt (not a parse-reusing update): the next
    /// table computes every file's resolutions again.
    pub(crate) fn program_rebuilt(&mut self) {
        self.rebuilt = true;
        self.program_paths = None;
    }

    /// The program's non-declaration files, by clean path, in program
    /// order: a parse-reusing update keeps the files and their order.
    pub(crate) fn program_paths(&mut self, program: &Program) -> &[String] {
        self.program_paths.get_or_insert_with(|| {
            program
                .source_files()
                .iter()
                .filter(|file| !file.is_declaration_file())
                .map(|file| clean(file.file_name()))
                .collect()
        })
    }

    /// A parse-reusing update replaced `path`'s file in place: every other
    /// file kept its parse, text and resolutions.
    pub(crate) fn file_replaced(&mut self, path: &str) {
        self.replaced.insert(clean(path));
    }

    /// `path` and every program file that reaches it over a resolution
    /// edge, by clean path, while the graph of the last table still
    /// describes the program (no rebuilt chain since); `None` otherwise.
    pub(crate) fn reaching(&self, path: &str) -> Option<Vec<String>> {
        if self.rebuilt {
            return None;
        }
        let graph = self.graph.as_ref()?;
        let target = *graph.ids.get(path)?;
        let mut reverse: Vec<Vec<u32>> = vec![Vec::new(); graph.paths.len()];
        for (from, edges) in graph.edges.iter().enumerate() {
            for to in edges {
                reverse[*to as usize].push(u32::try_from(from).unwrap_or(u32::MAX));
            }
        }
        let mut seen = bits_new(graph.paths.len());
        let mut pending = vec![target];
        bit_set(&mut seen, target);
        let mut found = Vec::new();
        while let Some(at) = pending.pop() {
            found.push(graph.paths[at as usize].clone());
            for from in &reverse[at as usize] {
                if !bit_get(&seen, *from) {
                    bit_set(&mut seen, *from);
                    pending.push(*from);
                }
            }
        }
        Some(found)
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

    /// Starts a table: drops every kept output whose read set changed (see
    /// the module docs), and moves the rest onto the current program's graph.
    pub(crate) fn refresh(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
        ids: &mut Identities,
        phases: &mut crate::trace::Phases,
    ) {
        self.stats = ReuseStats::default();
        self.census = None;
        let export_changes = ids.take_export_changes();
        let replaced = std::mem::take(&mut self.replaced);
        if let Some(export_changes) = &export_changes
            && self.refresh_in_place(program, inputs, ids, &replaced, export_changes)
        {
            phases.mark("inPlaceNs");
            if self.verify {
                self.verify_in_place(program, inputs, ids);
            }
            return;
        }
        self.census_writes = None;
        self.refresh_all(program, inputs, ids, phases);
    }

    /// [`Reuse::refresh`] after nothing but parse-reusing updates of
    /// non-global files: the program's files, edges and resolutions are the
    /// last table's, so its graph and global contributors are kept, and only
    /// the replaced files and the paths whose export identities moved are
    /// compared. `false` (nothing changed) when that does not hold.
    fn refresh_in_place(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
        ids: &mut Identities,
        replaced: &HashSet<String>,
        export_changes: &HashSet<String>,
    ) -> bool {
        if self.rebuilt || !self.enabled || typefacts::audit_off("in_place") {
            return false;
        }
        let Some(graph) = &self.graph else {
            return false;
        };
        if self.options != Some(inputs.options(program)) {
            return false;
        }
        let mut files = Vec::with_capacity(replaced.len());
        for path in replaced {
            let Some(file) = program.get_source_file(path) else {
                return false;
            };
            if clean(file.file_name()) != *path
                || !graph.ids.contains_key(path)
                || inputs.is_ambient(file)
                || self.ambient_paths.contains(path)
            {
                return false;
            }
            files.push(file);
        }
        let mut changed = bits_new(graph.paths.len());
        let mut changed_count = 0;
        let mut now: Vec<(u32, PathInputs)> = Vec::new();
        for path in replaced.iter().chain(export_changes) {
            let Some(id) = graph.ids.get(path).copied() else {
                return false;
            };
            let text = if replaced.contains(path) {
                files
                    .iter()
                    .find(|file| clean(file.file_name()) == *path)
                    .map(|file| inputs.text(*file))
            } else {
                self.snapshot.get(path).and_then(|held| held.0)
            };
            now.push((
                id,
                (
                    text,
                    ids.export_digest(path),
                    graph.resolutions[id as usize],
                ),
            ));
        }
        for (id, inputs_now) in now {
            let path = &graph.paths[id as usize];
            if self.snapshot.get(path) != Some(&inputs_now) {
                if !bit_get(&changed, id) {
                    changed_count += 1;
                }
                bit_set(&mut changed, id);
                self.snapshot.insert(path.clone(), inputs_now);
            }
        }
        self.stats.changed_paths = changed_count;
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| !bits_intersect(&entry.reads, &changed));
        self.stats.invalidated = before - self.entries.len();
        self.stats.cleared = before > 0 && self.entries.is_empty();
        // The census: the replaced files' halves.
        if let Some((counts, writes)) = &mut self.census_writes {
            let writes = Arc::make_mut(writes);
            for file in &files {
                if file.is_declaration_file() {
                    continue;
                }
                let path = clean(file.file_name());
                let text = inputs.text(*file);
                if let Some((digest, _)) = self.census_files.get(&path)
                    && *digest == text
                {
                    continue;
                }
                let half = Arc::new(file_property_writes(*file));
                let old = self
                    .census_files
                    .insert(path, (text, Arc::clone(&half)))
                    .map(|(_, half)| half);
                for name in old.iter().flat_map(|old| old.iter()) {
                    if let Some(count) = counts.get_mut(name) {
                        *count -= 1;
                        if *count == 0 {
                            counts.remove(name);
                            writes.remove(name);
                        }
                    }
                }
                for name in half.iter() {
                    let count = counts.entry(name.clone()).or_default();
                    *count += 1;
                    if *count == 1 {
                        writes.insert(name.clone());
                    }
                }
            }
        }
        true
    }

    /// [`ENV_VERIFY_REUSE`]: the kept graph, snapshot, global contributors
    /// and census against a refresh from nothing; a difference drops every
    /// kept output (fail closed).
    fn verify_in_place(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
        ids: &mut Identities,
    ) {
        let mut fresh = Graph::build(program);
        fresh.compute_resolutions(program);
        let kept = self.graph.as_ref();
        let mut same = kept.is_some_and(|kept| {
            kept.paths == fresh.paths
                && kept.edges == fresh.edges
                && kept.digest == fresh.digest
                && kept.resolutions == fresh.resolutions
        });
        if same {
            for (path, resolutions) in fresh.paths.iter().zip(&fresh.resolutions) {
                let now = (
                    inputs.text_at(program, path),
                    ids.export_digest(path),
                    *resolutions,
                );
                if self.snapshot.get(path) != Some(&now) {
                    same = false;
                    break;
                }
            }
            same &= self.snapshot.len() == fresh.paths.len();
        }
        if same && let Some(paths) = &self.program_paths {
            let fresh: Vec<String> = program
                .source_files()
                .iter()
                .filter(|file| !file.is_declaration_file())
                .map(|file| clean(file.file_name()))
                .collect();
            same = *paths == fresh;
        }
        if same && let Some((_, writes)) = &self.census_writes {
            let writes = Arc::clone(writes);
            self.census_writes = None;
            let census = self.census(program, inputs);
            same = *census == *writes;
        }
        if !same {
            crate::per_file::reuse_mismatch("the kept read-set inputs");
            self.entries.clear();
            self.census_writes = None;
            self.census = None;
            self.program_rebuilt();
        }
    }

    /// [`Reuse::refresh`] from the program itself.
    fn refresh_all(
        &mut self,
        program: &Program,
        inputs: &mut ProgramInputs,
        ids: &mut Identities,
        phases: &mut crate::trace::Phases,
    ) {
        inputs.prime(program, workers_from_env());
        phases.mark("primeNs");
        let mut graph = Graph::build(program);
        phases.mark("edgesNs");
        match &self.graph {
            // A parse-reusing update kept every resolution: the same graph
            // carries the same records.
            Some(old) if !self.rebuilt && old.digest == graph.digest => {
                graph.resolutions.clone_from(&old.resolutions);
            }
            _ => graph.compute_resolutions(program),
        }
        phases.mark("graphNs");
        self.rebuilt = false;
        let options = inputs.options(program);
        let ambient = inputs.ambient(program);
        if self.options != Some(options) || self.ambient != Some(ambient) {
            self.clear();
        }
        self.options = Some(options);
        self.ambient = Some(ambient);
        let snapshot: HashMap<String, PathInputs> = graph
            .paths
            .iter()
            .zip(&graph.resolutions)
            .map(|(path, resolutions)| {
                (
                    path.clone(),
                    (
                        inputs.text_at(program, path),
                        ids.export_digest(path),
                        *resolutions,
                    ),
                )
            })
            .collect();
        phases.mark("snapshotNs");
        let mut changed_count = 0;
        for (path, now) in &snapshot {
            if self.snapshot.get(path) != Some(now) {
                changed_count += 1;
            }
        }
        changed_count += self
            .snapshot
            .keys()
            .filter(|path| !snapshot.contains_key(*path))
            .count();
        self.stats.changed_paths = changed_count;
        let mut ambient_reads = bits_new(graph.paths.len());
        self.ambient_paths.clear();
        let mut ambient_files: Vec<u32> = Vec::new();
        for file in program.source_files() {
            if inputs.is_ambient(*file) {
                let path = clean(file.file_name());
                if let Some(id) = graph.ids.get(&path) {
                    ambient_files.push(*id);
                }
                self.ambient_paths.insert(path);
            }
        }
        graph.close(ambient_files, &mut ambient_reads);
        phases.mark("ambientNs");
        let before = self.entries.len();
        match self.graph.take() {
            Some(old) if !self.entries.is_empty() => {
                // A path changed when its inputs moved or it left the
                // program, in the old graph's index space.
                let mut changed = bits_new(old.paths.len());
                for (index, path) in old.paths.iter().enumerate() {
                    if snapshot.get(path) != self.snapshot.get(path) {
                        bit_set(&mut changed, u32::try_from(index).unwrap_or(u32::MAX));
                    }
                }
                self.entries
                    .retain(|_, entry| !bits_intersect(&entry.reads, &changed));
                phases.mark("retainNs");
                if old.digest != graph.digest {
                    // Same members, same edges: the closure over the new
                    // graph is the same set of paths. Checked, not assumed.
                    // Old indexes to new ones, by path (none when the
                    // program kept its files).
                    let same_paths = old.paths == graph.paths;
                    let moved: Vec<u32> = if same_paths {
                        Vec::new()
                    } else {
                        old.paths
                            .iter()
                            .map(|path| graph.ids.get(path).copied().unwrap_or(u32::MAX))
                            .collect()
                    };
                    self.entries.retain(|path, entry| {
                        let Some(id) = graph.ids.get(path).copied() else {
                            return false;
                        };
                        let mut reads = ambient_reads.clone();
                        graph.close([id], &mut reads);
                        let held = if same_paths {
                            Some(std::mem::take(&mut entry.reads))
                        } else {
                            translate(&entry.reads, &moved, graph.paths.len())
                        };
                        if held.as_ref() != Some(&reads) {
                            return false;
                        }
                        entry.reads = reads;
                        true
                    });
                }
            }
            _ => self.entries.clear(),
        }
        phases.mark("closuresNs");
        self.stats.invalidated = before - self.entries.len();
        self.stats.cleared = self.stats.cleared || (before > 0 && self.entries.is_empty());
        self.snapshot = snapshot;
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
        if let Some((_, writes)) = &self.census_writes {
            let census = Arc::clone(writes);
            self.census = Some(Arc::clone(&census));
            return census;
        }
        let mut counts: HashMap<String, u32> = HashMap::new();
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
            for name in half.iter() {
                *counts.entry(name.clone()).or_default() += 1;
            }
            writes.extend(half.iter().cloned());
            present.insert(path);
        }
        self.census_files.retain(|path, _| present.contains(path));
        let census = Arc::new(writes);
        self.census = Some(Arc::clone(&census));
        self.census_writes = Some((counts, Arc::clone(&census)));
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
        if !work.reusable || typefacts::audit_off("pass_reuse") {
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
                demands: work.run.map(|run| {
                    run.shared
                        .map_or_else(|| Arc::from(run.demands), Arc::clone)
                }),
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
    /// The served rows' alias-canonical ids, reversed: per canonical id, the
    /// ids whose served row names it. Built by the first query after a merge
    /// that started over, then moved by the served rows each update changes
    /// ([`Merged::update`]); `None` until built.
    canonical_index: Option<HashMap<SymbolId, Vec<SymbolId>>>,
    /// Whether the rows answer queries (false from a program change until
    /// the next table).
    pub(crate) serving: bool,
}

/// One merge: whether it started over, and each changed position's old and
/// new output.
pub(crate) struct MergeUpdate {
    pub full: bool,
    pub changed: Vec<(usize, Option<Arc<PassOutput>>, Arc<PassOutput>)>,
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

    /// Every id whose served row's alias-canonical id is in `canonical`
    /// (the same set as [`Merged::ids_with_canonical_scan`]).
    pub(crate) fn ids_with_canonical(&mut self, canonical: &HashSet<SymbolId>) -> Vec<SymbolId> {
        if typefacts::audit_off("canonical_index") {
            return self.ids_with_canonical_scan(canonical);
        }
        let index = match self.canonical_index.take() {
            Some(index) => index,
            None => self.canonical_index_of_rows(),
        };
        let ids = canonical
            .iter()
            .flat_map(|canonical| index.get(canonical).into_iter().flatten().cloned())
            .collect();
        self.canonical_index = Some(index);
        ids
    }

    /// [`Merged::ids_with_canonical`] by scanning every id: the reference it
    /// is verified against (`ENV_VERIFY_REUSE`, and every test build).
    pub(crate) fn ids_with_canonical_scan(&self, canonical: &HashSet<SymbolId>) -> Vec<SymbolId> {
        self.providers
            .keys()
            .filter(|id| {
                self.row(id)
                    .is_some_and(|row| canonical.contains(&row.canonical))
            })
            .cloned()
            .collect()
    }

    /// The served rows' canonical ids, reversed, from every id.
    fn canonical_index_of_rows(&self) -> HashMap<SymbolId, Vec<SymbolId>> {
        let mut index: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
        for id in self.providers.keys() {
            if let Some(row) = self.row(id) {
                index
                    .entry(row.canonical.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
        index
    }

    /// Moves `id` from the bucket of its served row's old canonical id to the
    /// bucket of its new one (`None`: no served row).
    fn reindex(
        index: &mut HashMap<SymbolId, Vec<SymbolId>>,
        id: &SymbolId,
        before: Option<&SymbolId>,
        after: Option<&SymbolId>,
    ) {
        if before == after {
            return;
        }
        if let Some(canonical) = before
            && let Some(ids) = index.get_mut(canonical)
        {
            if let Some(at) = ids.iter().position(|other| other == id) {
                ids.swap_remove(at);
            }
            if ids.is_empty() {
                index.remove(canonical);
            }
        }
        if let Some(canonical) = after {
            index.entry(canonical.clone()).or_default().push(id.clone());
        }
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
        let full = typefacts::audit_off("merge") || self.works != works;
        if full {
            self.keys = Arc::default();
            self.providers.clear();
            self.outputs.clear();
            self.canonical_index = None;
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
        let changed_rows = match served_before {
            Some(before) => {
                let moved: Vec<(SymbolId, Option<Row>)> = before
                    .into_iter()
                    .filter(|(id, row)| self.row(id) != row.as_ref())
                    .collect();
                if self.canonical_index.is_some() {
                    for (id, before) in &moved {
                        let before = before.as_ref().map(|row| &row.canonical);
                        let after = self.row(id).map(|row| row.canonical.clone());
                        if let Some(index) = self.canonical_index.as_mut() {
                            Self::reindex(index, id, before, after.as_ref());
                        }
                    }
                }
                Some(moved.into_iter().map(|(id, _)| id).collect())
            }
            None => None,
        };
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
                .map(|(at, old)| (at, old, Arc::clone(&self.outputs[at])))
                .collect(),
            rows_verified,
            row_mismatches,
            changed_rows,
        })
    }
}

#[cfg(test)]
mod pool_size_tests {
    #[test]
    fn small_machines_use_every_core_and_big_ones_keep_two() {
        let sizes: Vec<usize> = [1, 2, 3, 4, 5, 6, 8, 14, 32]
            .into_iter()
            .map(super::pool_size)
            .collect();
        assert_eq!(sizes, [1, 2, 3, 4, 4, 4, 6, 12, 12]);
    }
}

#[cfg(test)]
mod canonical_index_tests {
    use super::*;

    fn symbol(name: &str) -> SymbolId {
        Arc::from(name)
    }

    /// A pass output providing `(id, canonical)` rows, sorted by id as
    /// `PassOutput::row` searches them.
    fn output(rows: &[(usize, usize)], ids: &[SymbolId]) -> Arc<PassOutput> {
        let mut rows = rows.to_vec();
        rows.sort_unstable_by_key(|(id, _)| *id);
        rows.dedup_by_key(|(id, _)| *id);
        Arc::new(PassOutput {
            rows: rows
                .into_iter()
                .map(|(id, canonical)| {
                    let row = crate::per_file::Row {
                        found: true,
                        alias_target: ids[id].clone(),
                        declarations: Arc::from(Vec::new()),
                        canonical: ids[canonical].clone(),
                    };
                    (ids[id].clone(), row)
                })
                .collect(),
            ..Default::default()
        })
    }

    /// Random merges, incremental and starting over, with queries between
    /// them: the reverse index answers every canonical set as the scan does.
    #[test]
    fn the_reverse_index_answers_what_the_scan_answers() {
        const IDS: usize = 24;
        const PASSES: usize = 5;
        let ids: Vec<SymbolId> = (0..IDS).map(|i| symbol(&format!("s{i}"))).collect();
        let mut works: Vec<WorkId> = (0..PASSES)
            .map(|i| WorkId {
                path: format!("f{i}.ts"),
                run: true,
                scan: false,
                reusable: true,
            })
            .collect();
        let mut outputs: Vec<Arc<PassOutput>> = (0..PASSES).map(|_| output(&[], &ids)).collect();
        let mut merged = Merged::default();
        // A small deterministic generator: the same cases on every run.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            usize::try_from(state % bound as u64).unwrap_or(0)
        };
        for step in 0..600 {
            // A work list that changed starts the merge over.
            if step % 97 == 96 {
                let at = next(PASSES);
                works[at].reusable = !works[at].reusable;
            }
            for _ in 0..=next(3) {
                let at = next(PASSES);
                let count = next(7);
                let rows: Vec<(usize, usize)> =
                    (0..count).map(|_| (next(IDS), next(IDS))).collect();
                outputs[at] = output(&rows, &ids);
            }
            merged
                .update(works.clone(), outputs.clone(), false)
                .expect("no output has keys, so no key can collide");
            // Queried on some edits only: the index is built lazily, then
            // moved by every update that follows.
            if step % 3 != 0 {
                continue;
            }
            for _ in 0..3 {
                let count = next(4);
                let canonical: HashSet<SymbolId> =
                    (0..=count).map(|_| ids[next(IDS)].clone()).collect();
                let mut indexed = merged.ids_with_canonical(&canonical);
                let mut scanned = merged.ids_with_canonical_scan(&canonical);
                indexed.sort_unstable();
                scanned.sort_unstable();
                assert_eq!(indexed, scanned, "step {step}");
            }
        }
    }
}
