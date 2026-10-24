//! The tsrs-backed project (Go: `tsgo/project.go`, the compiler half S3 ports).
//!
//! `Project` implements [`Backend`](crate::backend::Backend) for everything
//! that is not a fact family: opening a configured project over the overlay
//! file system, the program, `sources` and source digests, the
//! `modules` graph, and transactional `update` with the affected set, the
//! retained import graph and the declaration-shape cutoff.
//!
//! The symbol families (S4a: `resolve_alias`, `declarations`, `symbol_evidence`,
//! `references_batch`, `changed_references`, and `semantic_demand_runs` for the
//! symbol, reference-space and structural-accessor facts) answer from the
//! per-file passes (`per_file`, `passes`; ADR 0398): each file's facts come
//! from a checker of its own, and `symbols` questions from the rows and the
//! merged reference index those passes left. The `Source*` families, async function
//! facts and the per-file source-fact memo (`sourceFactsMemo`) are in
//! `source_facts`; the update path evicts the memo beside the reference index.
//!
//! One deliberate difference from Go: Go memoizes source digests by
//! `*ast.SourceFile` pointer. Here the compiler's nodes are arena handles
//! whose addresses are reused after a free, so a pointer-keyed memo could
//! return a stale digest for a different file. Digests are recomputed on each
//! call instead (SHA-256 over the text; the program's files are in memory).

use crate::backend::{
    BackendError, ClosureBackend, ModuleGraph, ModuleInventory, OpenBackend, SymbolEvidence,
    UpdateOutcome,
};
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::identity::{CurrentExports, Env, Identities, Preserved};
use crate::import_graph::{
    ImportGraph, affected_from_reverse_deps, every_source_path, global_scope_change,
};
use crate::inputs::ProgramInputs;
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SymbolId};
use crate::module_graph::{normalize_type_script_path, type_script_path_dir};
use crate::nodes::node_at;
use crate::overlay::OverlayFs;
use crate::references::ReferenceIndex;
use crate::semantic_runs::Unavailable;
use crate::shape::{
    DeclarationShape, declaration_shape_for, declaration_shape_with_symbols_for, exported_symbols,
    has_global_or_module_augmentation, preserve_export_identities,
};
use crate::trace::SharedTrace;
use crate::unsafe_boundary::{Generation, HostLease, UpdateKind};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tsrs_ast::{
    Node, get_source_file_of_node, is_class_declaration, is_external_module,
    is_function_declaration, is_variable_declaration,
};
use tsrs_compiler::new_compiler_host;
use tsrs_core::arena::Region;
use tsrs_core::{CompilerOptions, P};
use tsrs_scanner::skip_trivia;
use tsrs_tsoptions::{ParseConfigHost, get_parsed_command_line_of_config_file};
use tsrs_vfs::{Entries, FS, FileInfo, bundled, osvfs};
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};
use typefacts::{
    AsyncFunctionFact, Declaration, Location, SourceBinding, SourceCall, SourceDigest,
    SourceFunction, SymbolFact,
};

/// Go: `ErrClosed`.
pub const ERR_CLOSED: &str = "type facts project is closed";

/// A full rebuild replaces the program chain after this many incremental
/// updates. Since each file version has a region of its own, freed once no head
/// reads it (`FileRegions`), the incremental path keeps about half a kilobyte
/// per update (measured: 3,000 updates of a 1,000-file chain, RSS 45 to 47 MiB);
/// the rebuild is a safety net for whatever that measurement missed, and a
/// fresh heap. It costs a whole program build, so it is rare. Plan 3.3.
pub const REBASE_AFTER_UPDATES: u32 = 1024;

/// The configuration parser keeps a `&'static` host (the module resolver
/// requires it), so one host per project is leaked, and its file system points
/// at whichever overlay the build in progress reads. The leak is one small
/// object per opened project, never per rebuild.
struct ConfigFs {
    current: RwLock<Arc<OverlayFs>>,
}

impl ConfigFs {
    fn overlay(&self) -> Arc<OverlayFs> {
        Arc::clone(
            &self
                .current
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl FS for ConfigFs {
    fn use_case_sensitive_file_names(&self) -> bool {
        self.overlay().use_case_sensitive_file_names()
    }
    fn file_exists(&self, path: &str) -> bool {
        self.overlay().file_exists(path)
    }
    fn read_file(&self, path: &str) -> Option<String> {
        self.overlay().read_file(path)
    }
    fn write_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.overlay().write_file(path, data)
    }
    fn append_file(&self, path: &str, data: &str) -> Result<(), String> {
        self.overlay().append_file(path, data)
    }
    fn remove(&self, path: &str) -> Result<(), String> {
        self.overlay().remove(path)
    }
    fn chtimes(&self, path: &str, a_time: SystemTime, m_time: SystemTime) -> Result<(), String> {
        self.overlay().chtimes(path, a_time, m_time)
    }
    fn directory_exists(&self, path: &str) -> bool {
        self.overlay().directory_exists(path)
    }
    fn get_accessible_entries(&self, path: &str) -> Entries {
        self.overlay().get_accessible_entries(path)
    }
    fn stat(&self, path: &str) -> Option<FileInfo> {
        self.overlay().stat(path)
    }
    fn realpath(&self, path: &str) -> String {
        self.overlay().realpath(path)
    }
}

struct ConfigHost {
    fs: ConfigFs,
    directory: String,
}

impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &self.fs
    }
    fn get_current_directory(&self) -> &str {
        &self.directory
    }
}

/// The `update` stage timings Go reports to the trace (`typefacts:update`),
/// kept on the project so tests can read them.
#[derive(Clone, Copy, Debug, Default)]
pub struct UpdateMetrics {
    pub total: Duration,
    pub overlay: Duration,
    pub old_shape: Duration,
    pub old_shape_cached: bool,
    pub program: Duration,
    pub new_shape: Duration,
    pub leaf_cutoff: bool,
    pub semantic_cutoff: bool,
    pub affected: Duration,
    pub invalidation: Duration,
    /// The identity state's rebuild (the exports scan) after the commit.
    pub identities: Duration,
    /// How the candidate program was made (`None` for a no-op update).
    pub kind: Option<UpdateKind>,
    /// Whether the chain was rebuilt after this update (the rebase policy).
    pub rebased: bool,
}

pub struct Project {
    pub(crate) trace: SharedTrace,
    config_path: String,
    fs: Arc<OverlayFs>,
    pub(crate) generation: Option<Generation>,
    /// After `generation`: dropped after every configuration parsed through it.
    config_host: HostLease<ConfigHost>,
    versions: HashMap<String, u64>,
    update_count: u64,
    graph: ImportGraph,
    /// Diagnostic-free exported contracts of the accepted program generation.
    /// Incremental updates need only compute the candidate's shape; semantically
    /// affected files are evicted and broad rebuilds clear the cache.
    declaration_shapes: HashMap<String, DeclarationShape>,
    pub last_update: UpdateMetrics,
    /// Symbol identity (Go: `idsBySymbol`, `symbolsByID`, `durableRefs`,
    /// `exportedIdentities`).
    pub(crate) ids: Identities,
    /// The reference index (Go: `referenceIndex`), merged from the passes'
    /// contributions.
    pub(crate) references: ReferenceIndex,
    /// Digests of the accepted program's inputs (pass reuse, `passes`).
    pub(crate) inputs: ProgramInputs,
    /// Demanded facts left absent because their family is not ported yet.
    pub unavailable: Unavailable,
    /// Per-file `Source*` facts across generations (Go: `sourceFactsMemo`).
    pub(crate) source_memo: HashMap<String, crate::source_facts::FileFactsMemo>,
    /// The per-file passes' state (`per_file`, ADR 0398).
    pub(crate) per_file: crate::per_file::PerFile,
}

impl Project {
    /// Go: `OpenProjectWithConfig`. `config_source`, when present, is the bytes
    /// of `config_path`: an overlay at that exact path, so the configuration is
    /// anchored where it would be on disk (its directory, default type roots,
    /// `${configDir}`) while nothing is written there, and every later rebuild
    /// reads the same bytes.
    pub fn open(
        ctx: &Context,
        config_path: &str,
        config_source: Option<&str>,
        trace: SharedTrace,
    ) -> Result<Self, String> {
        let absolute_path =
            absolute(config_path).map_err(|error| format!("resolve tsconfig path: {error}"))?;
        let config_path = normalize_type_script_path(&absolute_path);
        let base: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
        let fs = Arc::new(OverlayFs::new(base));
        if let Some(source) = config_source {
            fs.set(&config_path, source);
        }
        let config_host = HostLease::new(ConfigHost {
            fs: ConfigFs {
                current: RwLock::new(Arc::clone(&fs)),
            },
            directory: type_script_path_dir(&config_path),
        });
        let generation = build_program(ctx, &config_path, &fs, &config_host)?;
        let mut project = Self {
            trace,
            config_path,
            fs,
            config_host,
            versions: HashMap::new(),
            generation: Some(generation),
            update_count: 0,
            graph: ImportGraph::default(),
            declaration_shapes: HashMap::new(),
            last_update: UpdateMetrics::default(),
            ids: Identities::new(1),
            references: ReferenceIndex::default(),
            inputs: ProgramInputs::default(),
            unavailable: Unavailable::default(),
            source_memo: HashMap::new(),
            per_file: crate::per_file::PerFile::new(),
        };
        // Go: `collectExportedIdentities` seeds the symbol store at open.
        let generation = project.generation.as_ref().expect("just built");
        let program = generation.program();
        // A checker of its own, freed after (the exported identities are
        // pointer-free), so the program's single checker is never made.
        let pass = generation.file_checker();
        generation.with_checker(|checker| project.ids.seed_exported(program, checker));
        project.ids.purge_scope_pointers();
        drop(pass);
        project.graph = ImportGraph::rebuild(program);
        Ok(project)
    }

    pub(crate) fn live(&self) -> Result<&Generation, BackendError> {
        self.generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))
    }

    /// Runs `f` with the generation's checker, identity state and reference
    /// index (Go: `ensureCheckerLocked` under the project lock). Nothing `f`
    /// returns may hold a tsrs handle past this call.
    pub(crate) fn with_env<R>(
        &mut self,
        f: impl FnOnce(&mut Env<'_>, &mut ReferenceIndex) -> R,
    ) -> Result<R, BackendError> {
        let generation = self
            .generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))?;
        if !generation.in_file_checker() {
            // A question asked outside any pass gets a scratch pass of its
            // own.
            self.per_file.stats.scratch_passes += 1;
            return self.in_file_pass(|this| this.with_env(f))?;
        }
        let program = generation.program();
        let (ids, references, inputs) = (&mut self.ids, &mut self.references, &mut self.inputs);
        Ok(generation.with_checker(|checker| {
            f(
                &mut Env {
                    checker,
                    program,
                    ids,
                    inputs,
                },
                references,
            )
        }))
    }

    /// Go: `SymbolAt`: the symbol at the first byte of `location`.
    pub fn symbol_at(
        &mut self,
        ctx: &Context,
        location: &Location,
    ) -> Result<SymbolId, BackendError> {
        check(ctx)?;
        self.with_env(|env, _| {
            let absolute_path = absolute(&location.path)
                .map_err(|error| BackendError(format!("resolve source path: {error}")))?;
            let file = env
                .program
                .get_source_file(&absolute_path)
                .ok_or_else(|| BackendError(format!("not found: source file {absolute_path}")))?;
            validate_location(location, file.text())?;
            let node = node_at(file, to_u32(location.start_byte)?);
            let symbol = env
                .checker
                .get_symbol_at_location_exported(node)
                .ok_or_else(|| {
                    BackendError(format!(
                        "not found: symbol at byte {} (node kind {:?}, range {}:{})",
                        location.start_byte,
                        node.kind(),
                        node.pos().source_offset().map_or(-1, i64::from),
                        node.end().source_offset().map_or(-1, i64::from),
                    ))
                })?;
            let id = env.id_for(symbol);
            if id.is_empty() {
                // No exact key (`Identities::derived_keys`): no id to answer.
                return Err(BackendError(format!(
                    "not found: symbol at byte {} has no exact id",
                    location.start_byte
                )));
            }
            Ok(id)
        })?
    }

    /// Go: `DescribeTypeAt` (the optional `TypeDescriber` capability): the
    /// descriptor of the type at the node at the first byte of `location`.
    pub fn describe_type_at(
        &mut self,
        ctx: &Context,
        location: &Location,
    ) -> Result<typefacts::TypeDescriptor, BackendError> {
        check(ctx)?;
        self.with_env(|env, _| {
            let absolute_path = absolute(&location.path)
                .map_err(|error| BackendError(format!("resolve source path: {error}")))?;
            let file = env
                .program
                .get_source_file(&absolute_path)
                .ok_or_else(|| BackendError(format!("not found: source file {absolute_path}")))?;
            validate_location(location, file.text())?;
            let node = node_at(file, to_u32(location.start_byte)?);
            let value = env.checker.get_type_at_location(node);
            Ok(crate::type_facts::type_descriptor_for(env.checker, value))
        })?
    }

    /// Go: `References`: the canonical symbol's reference locations in
    /// (path, start byte) order. An id whose declaration no longer
    /// re-resolves fails closed, before any retained index entry could answer
    /// for it.
    pub fn references(&mut self, ctx: &Context, id: &str) -> Result<Vec<Location>, BackendError> {
        check(ctx)?;
        let id: SymbolId = id.into();
        self.per_file_references(ctx, &id)?
            .ok_or_else(|| BackendError(format!("not found: symbol {id}")))
    }

    /// The identity state, for tests.
    #[must_use]
    pub fn identities(&self) -> &Identities {
        &self.ids
    }

    /// The reference index, for tests.
    #[must_use]
    pub fn reference_index(&self) -> &ReferenceIndex {
        &self.references
    }

    /// The accepted program generation's number: 1 after open, +1 per
    /// accepted update.
    #[must_use]
    pub fn generation_number(&self) -> u64 {
        self.update_count + 1
    }

    /// The accepted program's generation, for tests of the lifecycle.
    #[must_use]
    pub fn current(&self) -> Option<&Generation> {
        self.generation.as_ref()
    }

    /// The accepted generation, mutably, for tests of the lifecycle contract.
    #[cfg(test)]
    pub(crate) fn current_mut_for_test(&mut self) -> &mut Generation {
        self.generation.as_mut().expect("open")
    }

    #[cfg(test)]
    pub(crate) fn release_analysis_state_for_test(&mut self) {
        ClosureBackend::release_analysis_state(self);
    }

    /// The retained shape of `path`, if one is cached.
    #[must_use]
    pub fn cached_shape(&self, path: &str) -> Option<&DeclarationShape> {
        self.declaration_shapes.get(path)
    }

    /// Resolved dependencies of `path` in the retained import graph.
    #[must_use]
    pub fn forward_dependencies(&self, path: &str) -> Vec<String> {
        self.graph.forward.get(path).cloned().unwrap_or_default()
    }
}

/// Go: `buildProgram`: parse the configuration through `fs`, build the
/// program, bind it. Errors carry Go's exact texts.
fn build_program(
    ctx: &Context,
    config_path: &str,
    fs: &Arc<OverlayFs>,
    config_host: &HostLease<ConfigHost>,
) -> Result<Generation, String> {
    if let Some(reason) = ctx.err() {
        return Err(reason.to_owned());
    }
    let directory = type_script_path_dir(config_path);
    let config_host = config_host.as_static();
    *config_host
        .fs
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::clone(fs);
    let host = compiler_host(&directory, fs);
    // The configuration lives in a region of its own, which the generation
    // keeps: replacing the generation frees it, and the vectors and strings
    // it owns, instead of leaving them in the thread arena and the heap.
    // Declared first, so it outlives everything below that reads it.
    let config_region = Region::new(256 << 10);
    let (config, diagnostics) = {
        let _scope = config_region.enter();
        get_parsed_command_line_of_config_file(
            config_path,
            Some(&CompilerOptions::default()),
            None,
            config_host,
            None,
        )
    };
    if !diagnostics.is_empty() {
        return Err(format!(
            "parse tsconfig: {}",
            diagnostics
                .iter()
                .map(|diagnostic| format!("TS{}: {}", diagnostic.code(), diagnostic.localize()))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let Some(config) = config else {
        return Err("parse tsconfig: no configuration returned".to_owned());
    };
    if !config.errors.is_empty() {
        return Err(format!(
            "parse tsconfig: {} configuration error(s)",
            config.errors.len()
        ));
    }
    let config = {
        let _scope = config_region.enter();
        P::new(config)
    };
    Ok(Generation::build(config_region, config, host))
}

pub(crate) fn compiler_host(
    directory: &str,
    fs: &Arc<OverlayFs>,
) -> Arc<dyn tsrs_compiler::CompilerHost> {
    new_compiler_host(
        directory,
        Arc::clone(fs) as Arc<dyn FS>,
        &bundled::lib_path(),
        None,
        None,
        None,
    )
}

/// [`source_digest`] from a SHA-256 already computed.
fn source_digest_of(path: &str, sha256: &crate::inputs::Digest) -> SourceDigest {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hash = String::with_capacity(7 + 64);
    hash.push_str("sha256:");
    for byte in sha256 {
        hash.push(char::from(HEX[usize::from(byte >> 4)]));
        hash.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    SourceDigest {
        path: path.into(),
        sha256: typefacts::SourceHash::parse(hash).expect("a SHA-256 digest is canonical"),
    }
}

fn lossy(source: &[u8]) -> String {
    String::from_utf8_lossy(source).into_owned()
}

impl ClosureBackend for Project {
    /// Go: `SourceFiles`: every non-declaration program file with its text,
    /// path-ordered.
    fn source_files(&mut self, ctx: &Context) -> Result<Vec<SourceFile>, BackendError> {
        check(ctx)?;
        let generation = self.live()?;
        let mut files: Vec<SourceFile> = generation
            .program()
            .source_files()
            .iter()
            .filter(|file| !file.is_declaration_file())
            .map(|file| SourceFile {
                path: clean(file.file_name()),
                local: false,
                source: file.text().as_bytes().to_vec(),
            })
            .collect();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    /// Go: `SourceDigests`: the semantic table's compact source inventory,
    /// hashed straight from the program's text (see the module docs for why
    /// there is no pointer-keyed memo).
    fn source_digests(&mut self, ctx: &Context) -> Result<Vec<SourceDigest>, BackendError> {
        check(ctx)?;
        let generation = self
            .generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))?;
        // `Text(file)` is the SHA-256 the digest spells, memoized while the
        // file's parse is reused.
        let inputs = &mut self.inputs;
        let mut digests: Vec<SourceDigest> = generation
            .program()
            .source_files()
            .iter()
            .filter(|file| !file.is_declaration_file())
            .map(|file| source_digest_of(&clean(file.file_name()), &inputs.text(*file)))
            .collect();
        digests.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(digests)
    }

    /// Go: `Update`.
    fn update(
        &mut self,
        ctx: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        let outcome = self.update_inner(ctx, changes);
        // An abandoned candidate's new file versions are dead now; after a
        // commit this finds nothing more.
        if let Some(generation) = &self.generation {
            generation.sweep_dead_files();
        }
        // A candidate build points the configuration host at the candidate
        // overlay; whether it was accepted or abandoned, the host must read
        // the accepted one afterwards.
        self.point_config_host_at_accepted_overlay();
        outcome
    }

    fn discard_derived_state(&mut self) {
        self.ids.release_pointers();
        // Per-file passes keep their merged reference index across the
        // program change, to patch it by the files that changed.
        if let Some(index) = self.references.take_pass_index() {
            self.per_file.stashed_index = Some(index);
        }
        // It describes the replaced program now: only a patch may use it.
        self.per_file.stashed_current = false;
        self.references.reset();
        self.evict_source_memo();
        self.per_file.clear_rows();
    }

    /// Go: `Close`.
    fn close(&mut self) -> Result<(), BackendError> {
        if self.generation.is_none() {
            return Err(BackendError(ERR_CLOSED.to_owned()));
        }
        // The identity maps hold handles into the generation about to go.
        self.ids.close();
        self.references.forget();
        self.inputs = ProgramInputs::default();
        self.source_memo.clear();
        self.generation = None;
        self.graph = ImportGraph::default();
        self.declaration_shapes.clear();
        Ok(())
    }

    /// Go: `ResolveAlias`. A symbol that is unknown, is not an alias, or has
    /// no target all answer `ErrNotFound`, which is `None` here.
    fn resolve_alias(&mut self, ctx: &Context, id: &str) -> Result<Option<SymbolId>, BackendError> {
        check(ctx)?;
        let id: SymbolId = id.into();
        Ok(self.per_file_row_answer(&id)?.0)
    }

    /// Go: `Declarations`. No declarations is `ErrNotFound`, like an unknown
    /// symbol.
    fn declarations(
        &mut self,
        ctx: &Context,
        id: &str,
    ) -> Result<Option<Vec<Declaration>>, BackendError> {
        check(ctx)?;
        let id: SymbolId = id.into();
        Ok(self.per_file_row_answer(&id)?.1)
    }

    /// Go: `SemanticDemandRuns`.
    fn semantic_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
    ) -> Result<Vec<SemanticDemandRunResult>, BackendError> {
        check(ctx)?;
        Ok(self.per_file_demand_runs(ctx, runs, &[])?.0)
    }

    /// Go: `AsyncFunctionsAt` (`source_facts`).
    fn async_functions_at(
        &mut self,
        ctx: &Context,
        locations: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        self.async_functions_at_locations(ctx, locations)
    }

    fn per_file_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        async_locations: &[Location],
    ) -> Result<(Vec<SemanticDemandRunResult>, Vec<AsyncFunctionFact>), BackendError> {
        Project::per_file_demand_runs(self, ctx, runs, async_locations)
    }

    fn source_calls(&mut self, ctx: &Context, path: &str) -> Result<Vec<SourceCall>, BackendError> {
        Project::source_calls(self, ctx, path)
    }

    fn source_bindings(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<SourceBinding>, BackendError> {
        Project::source_bindings(self, ctx, path)
    }

    fn source_functions(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<SourceFunction>, BackendError> {
        Project::source_functions(self, ctx, path)
    }

    fn source_async_functions(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        Project::source_async_functions(self, ctx, path)
    }

    /// Go: `ReferencesBatch`: merges the retained per-file fragments once and
    /// amortizes durable-id lookup, alias canonicalization and allocation.
    fn references_batch(
        &mut self,
        ctx: &Context,
        ids: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
        check(ctx)?;
        self.per_file_references_batch(ctx, ids)
    }

    /// Go: `ChangedReferences`: the merged passes name every changed answer
    /// (ADR 0389), or the table is not exact.
    fn changed_references(&mut self, ctx: &Context) -> Result<(Vec<SymbolId>, bool), BackendError> {
        check(ctx)?;
        self.ensure_references_per_file(ctx)?;
        Ok(self
            .per_file_symbol_changes()
            .map_or_else(|| (Vec::new(), false), |ids| (ids, true)))
    }

    /// Go: `ReleaseAnalysisState`: drops checker-expanded query state once its
    /// immutable facts have transferred into the closure. Pointer-keyed
    /// identity maps must go with the checker: retaining them both pins
    /// checker memory and prevents a recreated equivalent symbol from
    /// reclaiming its durable id. Durable declaration refs and compact
    /// reference contributions are pointer-free and remain the rehydration
    /// boundary.
    fn release_analysis_state(&mut self) {
        let Some(generation) = &mut self.generation else {
            return;
        };
        // No pass is live here, so no handle points into a file checker; the
        // rows and the merged index are the analysis state.
        let state = &mut self.per_file;
        state.clear_rows();
        self.ids.purge_scope_pointers();
        // Per-file passes keep their merged reference index (pointer-free)
        // for this generation's reference questions and the next table's
        // patch: what a rescan of every file would build again.
        if let Some(index) = self.references.take_pass_index() {
            state.stashed_index = Some(index);
            state.stashed_current = true;
        }
        self.references.release_analysis_state();
        if generation.has_checker() {
            generation.release_checker();
        }
    }

    fn binder_retention(&mut self) -> crate::references::BinderRetention {
        self.references.take_retention()
    }

    fn tracks_content_versions(&self) -> bool {
        self.per_file_content_version("").is_some()
    }

    fn content_version(&self, path: &str) -> Option<u64> {
        self.per_file_content_version(path).flatten()
    }
}

impl SymbolEvidence for Project {
    /// Go: `SymbolEvidence`: resolves a complete closure worklist batch under
    /// one checker lease. A missing or stale identity returns an id-only row,
    /// matching the historical closure's independent `ErrNotFound` handling.
    fn symbol_evidence(
        &mut self,
        ctx: &Context,
        queries: &[SymbolQuery],
    ) -> Result<Vec<SymbolFact>, BackendError> {
        check(ctx)?;
        self.per_file_symbol_evidence(ctx, queries)
    }
}

impl ModuleGraph for Project {
    fn module_graph(
        &mut self,
        ctx: &Context,
        demand: &ModuleGraphRequest,
    ) -> Result<ModuleInventory, BackendError> {
        check(ctx)?;
        let generation = self.live()?;
        crate::module_graph::module_graph(ctx, generation.program(), demand)
    }
}

impl Project {
    /// The body of `update`; see [`ClosureBackend::update`].
    #[allow(clippy::too_many_lines)]
    fn update_inner(
        &mut self,
        ctx: &Context,
        changes: &[FileChange],
    ) -> Result<UpdateOutcome, BackendError> {
        let update_started = Instant::now();
        check(ctx)?;
        self.live()?;

        let stage = Instant::now();
        let candidate_fs = Arc::new(self.fs.duplicate());
        let mut candidate_versions = self.versions.clone();
        let mut changed_paths: Vec<String> = Vec::with_capacity(changes.len());
        let mut incremental_path = String::new();
        let mut incremental = true;
        for change in changes {
            let absolute_path = absolute(&change.path)
                .map_err(|error| BackendError(format!("resolve changed path: {error}")))?;
            if let Some(version) = candidate_versions.get(&absolute_path)
                && change.version <= *version
            {
                continue;
            }
            if !incremental_path.is_empty() || change.deleted || absolute_path == self.config_path {
                incremental = false;
            } else {
                incremental_path.clone_from(&absolute_path);
            }
            candidate_versions.insert(absolute_path.clone(), change.version);
            if change.deleted {
                candidate_fs.delete(&absolute_path);
            } else {
                candidate_fs.set(&absolute_path, &lossy(&change.source));
            }
            changed_paths.push(absolute_path);
        }
        if changed_paths.is_empty() {
            return Ok(UpdateOutcome {
                affected: Vec::new(),
            });
        }
        let overlay_duration = stage.elapsed();
        let mut incremental = incremental && !incremental_path.is_empty();

        let generation = self.live()?;
        if incremental
            && generation
                .program()
                .get_source_file(&incremental_path)
                .is_none()
        {
            // A file entering the program (a restored deletion, a new file) can
            // satisfy an import that was unresolved when retained answers were
            // computed. An unresolved specifier records no import edge, so the
            // reverse-dependency walk and the leaf cutoff cannot see that
            // importer. Take the full-rebuild path, whose affected set unions
            // the new program's edges (which now include the resolved import).
            // Go: the same guard in `project.Update` (3991f6919).
            incremental = false;
            incremental_path.clear();
        }
        let no_importers = incremental
            && self
                .graph
                .reverse
                .get(&incremental_path)
                .is_none_or(std::collections::HashSet::is_empty);

        // The old program's contract, before the edit lands.
        let stage = Instant::now();
        let mut old_shape: Option<DeclarationShape> = None;
        let mut old_shape_cached = false;
        if incremental && !no_importers {
            if let Some(cached) = self.declaration_shapes.get(&incremental_path) {
                old_shape = Some(cached.clone());
                old_shape_cached = true;
            } else {
                old_shape = declaration_shape_for(generation, &incremental_path);
            }
            check(ctx)?;
        }
        let old_shape_duration = stage.elapsed();

        // The candidate program. The accepted generation stays intact until
        // the candidate is committed below.
        let stage = Instant::now();
        let candidate = if incremental
            && generation
                .program()
                .get_source_file(&incremental_path)
                .is_some()
        {
            check(ctx)?;
            let host = compiler_host(&type_script_path_dir(&self.config_path), &candidate_fs);
            generation.prepare_update(&incremental_path, host)
        } else {
            build_program(ctx, &self.config_path, &candidate_fs, &self.config_host)
                .map_err(BackendError)?
        };
        let program_duration = stage.elapsed();

        let mut semantic_cutoff = false;
        let mut leaf_cutoff = false;
        // Go: `currentExports`, the candidate's canonical export symbols with
        // the identity each export slot keeps; `None` is `!currentExportsKnown`.
        let mut current_exports: Option<CurrentExports> = None;
        let stage = Instant::now();
        if no_importers
            && let Some(file) = candidate.program().get_source_file(&incremental_path)
            && is_external_module(file)
            && !has_global_or_module_augmentation(file)
            && let Some(exports) = exported_symbols(&candidate, file)
        {
            current_exports = Some(
                exports
                    .into_iter()
                    .map(|exported| {
                        (
                            exported.symbol,
                            Preserved {
                                id: exported.export.id.as_str().into(),
                                reference: exported.export.reference,
                            },
                        )
                    })
                    .collect(),
            );
            semantic_cutoff = true;
            leaf_cutoff = true;
        }
        let mut new_shape: Option<DeclarationShape> = None;
        if let Some(old) = &old_shape {
            if let Some((shape, symbols)) =
                declaration_shape_with_symbols_for(&candidate, &incremental_path)
            {
                semantic_cutoff = preserve_export_identities(old, &shape);
                current_exports = Some(
                    shape
                        .exports
                        .iter()
                        .zip(symbols)
                        .map(|(export, symbol)| {
                            (
                                symbol,
                                Preserved {
                                    id: export.id.as_str().into(),
                                    reference: export.reference.clone(),
                                },
                            )
                        })
                        .collect(),
                );
                new_shape = Some(shape);
            }
            // A cancelled update drops the candidate, which frees only what it
            // allocated; the accepted generation is untouched.
            check(ctx)?;
        }
        let new_shape_duration = stage.elapsed();

        // Everything the affected set needs from the old program has to be read
        // now: committing frees it.
        let stage = Instant::now();
        let touches_global_scope =
            global_scope_change(&changed_paths, &[generation.program(), candidate.program()]);
        let all_paths = if touches_global_scope && !semantic_cutoff {
            every_source_path(&[generation.program(), candidate.program()])
        } else {
            Vec::new()
        };
        let candidate_kind = candidate.kind();

        // Commit: nothing below can fail.
        let generation = self.generation.as_mut().expect("checked live above");
        generation.commit(candidate);
        // Only a parse-reusing update keeps every other file's text as read.
        let reused = candidate_kind == UpdateKind::Reused;
        self.inputs
            .program_changed(reused.then_some(incremental_path.as_str()));
        if !reused {
            // A rebuilt chain read the file system again. Kept passes are
            // checked path by path against the new program's text, exports
            // and resolutions at the next table (`passes::Reuse::refresh`).
            self.ids.forget_runtime_paths();
            self.per_file.reuse.program_rebuilt();
        }
        self.fs = candidate_fs;
        self.versions = candidate_versions;
        self.update_count += 1;
        let generation_number = self.update_count + 1;
        let accepted = self.generation.as_ref().expect("just committed");
        let program = accepted.program();
        // The identity maps held handles into the replaced generation; they
        // restart from the candidate's own symbols (Go: the `currentExports`
        // re-keying) before anything can read them.
        let ids = &mut self.ids;
        let identities_started = Instant::now();
        // The exports scan in a checker of its own, freed after (its output
        // is pointer-free).
        let pass = accepted.file_checker();
        accepted.with_checker(|checker| {
            ids.after_update(
                generation_number,
                program,
                checker,
                incremental.then_some(incremental_path.as_str()),
                current_exports,
            );
        });
        ids.purge_scope_pointers();
        drop(pass);
        let identities_duration = identities_started.elapsed();

        // The retained graph must advance on every accepted update, even ones
        // whose affected set is decided without a walk, or a later generation
        // walks stale edges. `old_reverse` keeps the pre-update edges alive
        // for the rebuild path, where the union of both generations' graphs
        // preserves the prior behavior: an importer whose edge only the old
        // program knew (a deleted dependency, a redirected resolution) still
        // lands in the set.
        let graph_patched = incremental && self.graph.edges_unchanged(program, &incremental_path);
        let old_reverse = if graph_patched {
            None
        } else {
            let rebuilt = ImportGraph::rebuild(program);
            let previous = std::mem::replace(&mut self.graph, rebuilt);
            Some(previous.reverse)
        };
        let mut affected = if semantic_cutoff {
            // A diagnostic-free declaration emit proves that the module's
            // exported TypeScript shape is unchanged, and every external export
            // slot was paired bijectively with its prior canonical id. Retained
            // importer facts can therefore keep those ids even when declaration
            // spans inside the edited module moved.
            changed_paths.clone()
        } else if touches_global_scope {
            // A change inside the shared global scope can be referenced from
            // anywhere with no import edge to follow. Fail closed, exactly as
            // the multi-file and delete paths do.
            all_paths
        } else if let Some(old_reverse) = &old_reverse {
            affected_from_reverse_deps(&changed_paths, &[old_reverse, &self.graph.reverse])
        } else {
            // A content edit changes only the edited file's own outgoing
            // edges, and a walk rooted at that file never traverses them, so
            // the patched graph alone already equals the two-generation union.
            affected_from_reverse_deps(&changed_paths, &[&self.graph.reverse])
        };
        affected.sort();
        let affected_duration = stage.elapsed();

        let stage = Instant::now();
        // No type-derived answer survives an update (ADR 0364): not the
        // source-fact memo, not this generation's reference index, and not the
        // identity maps (`Identities::after_update`). What carries over is the
        // program's own incremental reuse, the declaration shapes that decide
        // the affected set, and the per-file passes kept with their read sets
        // (`per_file`, `passes`), which are checked before any reuse.
        ClosureBackend::discard_derived_state(self);
        if incremental {
            for path in &affected {
                self.declaration_shapes.remove(&clean(path));
            }
            if let Some(shape) = new_shape {
                self.declaration_shapes
                    .insert(incremental_path.clone(), shape);
            }
        } else {
            self.declaration_shapes.clear();
        }
        let invalidation_duration = stage.elapsed();

        let rebased = self.rebase_if_due();
        self.last_update = UpdateMetrics {
            total: update_started.elapsed(),
            overlay: overlay_duration,
            old_shape: old_shape_duration,
            old_shape_cached,
            program: program_duration,
            new_shape: new_shape_duration,
            leaf_cutoff,
            semantic_cutoff,
            affected: affected_duration,
            invalidation: invalidation_duration,
            identities: identities_duration,
            kind: Some(candidate_kind),
            rebased,
        };
        if let Some(trace) = &self.trace {
            let metrics = self.last_update;
            let nanos = |duration: Duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX);
            trace.metrics(
                "update",
                &[
                    ("totalNs", nanos(metrics.total)),
                    ("overlayNs", nanos(metrics.overlay)),
                    ("oldShapeNs", nanos(metrics.old_shape)),
                    ("oldShapeCached", i64::from(metrics.old_shape_cached)),
                    ("programNs", nanos(metrics.program)),
                    ("newShapeNs", nanos(metrics.new_shape)),
                    ("leafCutoff", i64::from(metrics.leaf_cutoff)),
                    ("affectedNs", nanos(metrics.affected)),
                    ("invalidationNs", nanos(metrics.invalidation)),
                    ("identitiesNs", nanos(metrics.identities)),
                ],
            );
        }
        Ok(UpdateOutcome { affected })
    }

    fn point_config_host_at_accepted_overlay(&self) {
        *self
            .config_host
            .as_static()
            .fs
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::clone(&self.fs);
    }

    /// The rebase policy (plan 3.3): after [`REBASE_AFTER_UPDATES`]
    /// incremental updates the chain is replaced by a fresh build of the same
    /// content, so everything the chain accumulated is freed at once. The
    /// parsed configuration is reused (Go's `UpdateProgram` fallback does the
    /// same), the overlay is the accepted one, and the import graph and
    /// retained shapes describe unchanged content, so nothing observable
    /// changes. S4 must treat it like `ReleaseAnalysisState`: the program is a
    /// different arena, so checker-owned symbol maps do not survive it.
    fn rebase_if_due(&mut self) -> bool {
        let Some(generation) = &mut self.generation else {
            return false;
        };
        if generation.updates_since_build() < REBASE_AFTER_UPDATES {
            return false;
        }
        let host = compiler_host(&type_script_path_dir(&self.config_path), &self.fs);
        let fresh = generation.rebuilt(host);
        // The identity maps and the generation-scoped reference ids name the
        // chain about to be freed.
        self.ids.release_pointers();
        self.ids.forget_runtime_paths();
        self.per_file.reuse.program_rebuilt();
        generation.commit(fresh);
        self.inputs.program_changed(None);
        true
    }
}

fn to_u32(value: u64) -> Result<u32, BackendError> {
    u32::try_from(value)
        .map_err(|_| BackendError(format!("source byte offset {value} is out of range")))
}

fn utf8_boundary(source: &str, offset: usize) -> bool {
    offset == 0 || offset == source.len() || source.is_char_boundary(offset)
}

/// Go: `validateLocation`. `source` is valid UTF-8 by construction.
pub(crate) fn validate_location(location: &Location, source: &str) -> Result<(), BackendError> {
    let (start, end) = (location.start_byte, location.end_byte);
    if end < start || end > source.len() as u64 {
        return Err(BackendError(format!(
            "source byte range [{start},{end}) is outside [0,{})",
            source.len()
        )));
    }
    if !utf8_boundary(source, start as usize) || !utf8_boundary(source, end as usize) {
        return Err(BackendError(format!(
            "source byte range [{start},{end}) does not fall on UTF-8 boundaries"
        )));
    }
    Ok(())
}

/// Go: `declarationKind`.
/// The declaration's kind, one shared string per kind: every retained row's
/// declarations hold one.
fn declaration_kind(node: tsrs_core::P<Node>) -> Arc<str> {
    static KINDS: std::sync::OnceLock<[Arc<str>; 4]> = std::sync::OnceLock::new();
    let kinds =
        KINDS.get_or_init(|| ["variable", "function", "class", "declaration"].map(Arc::from));
    let kind = if is_variable_declaration(node) {
        0
    } else if is_function_declaration(node) {
        1
    } else if is_class_declaration(node) {
        2
    } else {
        3
    };
    Arc::clone(&kinds[kind])
}

/// The clean path of `file_name` as a shared string. Declarations name few
/// distinct files (the libraries' most), and each retained row holds its
/// declarations' paths. Content-keyed, so a path is never another file's; a
/// thread's map is cleared once it holds 4,096 paths.
fn declaration_path(file_name: &str) -> Arc<str> {
    thread_local! {
        static PATHS: std::cell::RefCell<HashMap<Box<str>, Arc<str>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    PATHS.with(|paths| {
        let mut paths = paths.borrow_mut();
        if let Some(path) = paths.get(file_name) {
            return Arc::clone(path);
        }
        if paths.len() >= 4096 {
            paths.clear();
        }
        let path: Arc<str> = clean(file_name).into();
        paths.insert(file_name.into(), Arc::clone(&path));
        path
    })
}

/// Go: `wireSymbolName`. Go marks an internal symbol name with a leading
/// `0xFE` byte and spells it `__x` at the wire seam; tsrs's marker is U+007F,
/// which is valid UTF-8 and would reach the wire unnoticed. Every symbol name
/// the producer writes (declaration names in both evidence sections and the
/// symbol table, and the required-property names of parameter object shapes
/// once resolved calls are ported) must go through this. The checker's
/// counters are left out (`#<class id>@x`, `@sym@<symbol id>`, ADR 0367):
/// they follow binding and checking order, not the program.
pub(crate) fn wire_symbol_name(name: &str) -> String {
    let name = crate::durable::display_name(name);
    match name.strip_prefix('\u{7f}') {
        Some(rest) => format!("__{rest}"),
        None => name.into_owned(),
    }
}

/// The declarations an id's row reports. A multi-origin property's are its
/// origins' declarations (ADR 0371), in `(path, start, end)` order: the
/// properties sharing a `Set` id list them in their constituents' order,
/// which follows type ids.
pub(crate) fn id_declarations(
    env: &Env<'_>,
    id: &SymbolId,
    symbol: tsrs_core::P<tsrs_ast::Symbol>,
) -> Vec<Declaration> {
    let mut declarations = declarations_for_symbol(symbol);
    if env.ids.is_set_id(id) {
        declarations.sort_by(|left, right| {
            (
                &left.location.path,
                left.location.start_byte,
                left.location.end_byte,
            )
                .cmp(&(
                    &right.location.path,
                    right.location.start_byte,
                    right.location.end_byte,
                ))
        });
    }
    declarations
}

/// Go: `declarationsForSymbol`.
pub(crate) fn declarations_for_symbol(symbol: tsrs_core::P<tsrs_ast::Symbol>) -> Vec<Declaration> {
    let mut declarations = Vec::with_capacity(symbol.declarations().len());
    // Go's codec spells the internal-name marker `__` at the wire seam
    // (`wireSymbolName`); tsrs's marker is U+007F, so spell it here. One
    // string for every declaration of the symbol.
    let mut name: Option<Arc<str>> = None;
    for node in symbol.declarations() {
        let Some(file) = get_source_file_of_node(*node) else {
            continue;
        };
        let name_node = node.name().unwrap_or(*node);
        let (Some(pos), Some(end)) = (
            name_node.pos().source_offset(),
            name_node.end().source_offset(),
        ) else {
            continue;
        };
        declarations.push(Declaration {
            name: Arc::clone(name.get_or_insert_with(|| wire_symbol_name(symbol.name()).into())),
            kind: declaration_kind(*node),
            location: Location {
                path: declaration_path(file.file_name()),
                start_byte: u64::from(skip_trivia(file.text(), pos)),
                end_byte: u64::from(end),
            },
        });
    }
    declarations
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

/// The opener the real binary hands the service loop (Go: `OpenProjectWithConfig`
/// behind `main`'s `open`). The returned backend is created and used on the
/// worker thread.
pub fn opener(trace: SharedTrace) -> OpenBackend {
    Box::new(move |ctx, project_id, config_source| {
        Project::open(ctx, project_id, config_source, trace)
            .map(|project| Box::new(project) as Box<dyn crate::backend::Backend>)
    })
}

#[cfg(test)]
mod digest_tests {
    use sha2::{Digest as _, Sha256};

    /// A memoized `Text` digest spells the same source digest as hashing the
    /// text again.
    #[test]
    fn source_digest_from_a_memoized_text_digest_is_the_same() {
        for text in ["", "export const a = 1;\n", "\u{65e5}\u{672c}"] {
            let digest: crate::inputs::Digest = Sha256::digest(text.as_bytes()).into();
            assert_eq!(
                super::source_digest_of("/p/a.ts", &digest),
                typefacts::producer::source_digest("/p/a.ts", text.as_bytes())
            );
        }
    }
}
