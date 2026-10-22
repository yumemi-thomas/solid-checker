//! The tsrs-backed project (Go: `tsgo/project.go`, the compiler half S3 ports).
//!
//! `Project` implements [`Backend`](crate::backend::Backend) for everything
//! that is not a fact family: opening a configured project over the overlay
//! file system, the single-checker program, `sources` and source digests, the
//! `modules` graph, and transactional `update` with the affected set, the
//! retained import graph and the declaration-shape cutoff.
//!
//! The symbol families (S4a: `resolve_alias`, `declarations`, `symbol_evidence`,
//! `references_batch`, `changed_references`, and `semantic_demand_runs` for the
//! symbol, reference-space and structural-accessor facts) answer from the
//! identity state (`identity`) and the reference index (`references`) this
//! module keeps beside the program. `async_functions_at` is still S4 and
//! answers an error naming the missing slice, never an empty or guessed fact:
//! an empty answer would claim "no async function here", which is a fact the
//! backend does not hold. The source-fact memo (`sourceFactsMemo`) arrives
//! with the `Source*` families.
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
use crate::model::{
    SemanticDemandRun, SemanticDemandRunResult, SemanticScope, SymbolId, blank_symbol,
};
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
use crate::unsafe_boundary::{Generation, UpdateKind};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tsrs_ast::{
    Node, SymbolFlags, get_source_file_of_node, is_class_declaration, is_external_module,
    is_function_declaration, is_variable_declaration,
};
use tsrs_compiler::new_compiler_host;
use tsrs_core::{CompilerOptions, P};
use tsrs_scanner::skip_trivia;
use tsrs_tsoptions::{ParseConfigHost, get_parsed_command_line_of_config_file};
use tsrs_vfs::{Entries, FS, FileInfo, bundled, osvfs};
use typefacts::producer::source_digest;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile, SymbolQuery};
use typefacts::{AsyncFunctionFact, Declaration, Location, SourceDigest, SymbolFact};

/// Go: `ErrClosed`.
pub const ERR_CLOSED: &str = "type facts project is closed";

/// A full rebuild replaces the program chain after this many incremental
/// updates, so the memory the incremental path cannot return (the edited
/// files' trees, which share the chain's regions) stays bounded. Plan 3.3.
pub const REBASE_AFTER_UPDATES: u32 = 64;

/// What the backend answers for a family that arrives in S4.
fn unavailable(operation: &str) -> BackendError {
    BackendError(format!(
        "type facts backend: {operation} is not available in the tsrs backend yet (migration slice S4)"
    ))
}

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
    /// How the candidate program was made (`None` for a no-op update).
    pub kind: Option<UpdateKind>,
    /// Whether the chain was rebuilt after this update (the rebase policy).
    pub rebased: bool,
}

pub struct Project {
    trace: SharedTrace,
    config_path: String,
    fs: Arc<OverlayFs>,
    config_host: &'static ConfigHost,
    versions: HashMap<String, u64>,
    generation: Option<Generation>,
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
    /// The reference index (Go: `referenceIndex`).
    pub(crate) references: ReferenceIndex,
    /// Demanded facts left absent because their family is not ported yet.
    pub unavailable: Unavailable,
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
        let config_host: &'static ConfigHost = Box::leak(Box::new(ConfigHost {
            fs: ConfigFs {
                current: RwLock::new(Arc::clone(&fs)),
            },
            directory: type_script_path_dir(&config_path),
        }));
        let generation = build_program(ctx, &config_path, &fs, config_host)?;
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
            unavailable: Unavailable::default(),
        };
        // Go: `collectExportedIdentities` seeds the symbol store at open.
        let generation = project.generation.as_ref().expect("just built");
        let program = generation.program();
        generation.with_checker(|checker| project.ids.seed_exported(program, checker));
        project.graph = ImportGraph::rebuild(program);
        Ok(project)
    }

    fn live(&self) -> Result<&Generation, BackendError> {
        self.generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))
    }

    /// Runs `f` with the generation's checker, identity state and reference
    /// index (Go: `ensureCheckerLocked` under the project lock). Nothing `f`
    /// returns may hold a tsrs handle past this call.
    fn with_env<R>(
        &mut self,
        f: impl FnOnce(&mut Env<'_>, &mut ReferenceIndex) -> R,
    ) -> Result<R, BackendError> {
        let generation = self
            .generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))?;
        let program = generation.program();
        let (ids, references) = (&mut self.ids, &mut self.references);
        Ok(generation.with_checker(|checker| {
            f(
                &mut Env {
                    checker,
                    program,
                    ids,
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
            Ok(env.id_for(symbol))
        })?
    }

    /// Go: `References`: the canonical symbol's reference locations in
    /// (path, start byte) order. An id whose declaration no longer
    /// re-resolves fails closed, before any retained index entry could answer
    /// for it.
    pub fn references(&mut self, ctx: &Context, id: &str) -> Result<Vec<Location>, BackendError> {
        check(ctx)?;
        let id: SymbolId = id.into();
        self.with_env(|env, references| {
            let target = env
                .symbol_for(&id)
                .ok_or_else(|| BackendError(format!("not found: symbol {id}")))?;
            let canonical = env.canonical_symbol(target);
            let canonical = env.id_for(canonical);
            references.ensure(env);
            references.mark_used(&canonical);
            Ok(references.locations(&canonical))
        })?
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
    config_host: &'static ConfigHost,
) -> Result<Generation, String> {
    if let Some(reason) = ctx.err() {
        return Err(reason.to_owned());
    }
    let directory = type_script_path_dir(config_path);
    *config_host
        .fs
        .current
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::clone(fs);
    let host = compiler_host(&directory, fs);
    let (config, diagnostics) = get_parsed_command_line_of_config_file(
        config_path,
        Some(&CompilerOptions::default()),
        None,
        config_host,
        None,
    );
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
    Ok(Generation::build(P::new(config), host))
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
        let generation = self.live()?;
        let mut digests: Vec<SourceDigest> = generation
            .program()
            .source_files()
            .iter()
            .filter(|file| !file.is_declaration_file())
            .map(|file| source_digest(&clean(file.file_name()), file.text().as_bytes()))
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
        // A candidate build points the configuration host at the candidate
        // overlay; whether it was accepted or abandoned, the host must read
        // the accepted one afterwards.
        self.point_config_host_at_accepted_overlay();
        outcome
    }

    /// Go: `Close`.
    fn close(&mut self) -> Result<(), BackendError> {
        if self.generation.is_none() {
            return Err(BackendError(ERR_CLOSED.to_owned()));
        }
        // The identity maps hold handles into the generation about to go.
        self.ids.close();
        self.references.reset();
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
        self.with_env(|env, _| {
            let symbol = env.symbol_for(&id)?;
            if !symbol.flags().intersects(SymbolFlags::Alias) {
                return None;
            }
            let original = env.checker.get_aliased_symbol(symbol);
            Some(env.id_for(original))
        })
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
        self.with_env(|env, _| {
            let symbol = env.symbol_for(&id)?;
            let declarations = declarations_for_symbol(symbol);
            (!declarations.is_empty()).then_some(declarations)
        })
    }

    /// Go: `SemanticDemandRuns`.
    fn semantic_demand_runs(
        &mut self,
        ctx: &Context,
        runs: &[SemanticDemandRun<'_>],
        scope: &SemanticScope<'_>,
    ) -> Result<Vec<SemanticDemandRunResult>, BackendError> {
        check(ctx)?;
        let mut unavailable = std::mem::take(&mut self.unavailable);
        let outcome = self.with_env(|env, references| {
            crate::semantic_runs::run(ctx, env, references, runs, scope, &mut unavailable)
        });
        for (family, count) in unavailable.0 {
            *self.unavailable.0.entry(family).or_default() += count;
        }
        outcome?
    }

    /// Async facts are S4 item 7. Asked about no locations the answer is
    /// empty (the closure always asks, even when no demand wants async facts);
    /// asked about any location it fails rather than claim "no async function
    /// here", which the backend does not know.
    fn async_functions_at(
        &mut self,
        ctx: &Context,
        locations: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        check(ctx)?;
        self.live()?;
        if locations.is_empty() {
            return Ok(Vec::new());
        }
        Err(unavailable("async_functions_at"))
    }

    /// Go: `ReferencesBatch`: merges the retained per-file fragments once and
    /// amortizes durable-id lookup, alias canonicalization and allocation.
    fn references_batch(
        &mut self,
        ctx: &Context,
        ids: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
        check(ctx)?;
        self.with_env(|env, references| {
            references.ensure(env);
            let mut result = HashMap::with_capacity(ids.len());
            for id in ids {
                let Some(target) = env.symbol_for(id) else {
                    continue;
                };
                let canonical = env.canonical_symbol(target);
                let canonical = env.id_for(canonical);
                references.mark_used(&canonical);
                result.insert(id.clone(), references.locations(&canonical));
            }
            result
        })
    }

    /// Go: `ChangedReferences`: exposes the reference index's
    /// generation-stable invalidation set. It never consumes the delta, so
    /// cancelled analyses and retries in the same generation observe the same
    /// answer.
    fn changed_references(&mut self, ctx: &Context) -> Result<(Vec<SymbolId>, bool), BackendError> {
        check(ctx)?;
        self.with_env(|env, references| {
            references.ensure(env);
            match references.changed() {
                Some(ids) => (ids, true),
                None => (Vec::new(), false),
            }
        })
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
        if !generation.has_checker() {
            return;
        }
        self.ids.release_pointers();
        self.references.release_analysis_state();
        generation.release_checker();
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
        self.with_env(|env, references| {
            if queries.iter().any(|query| query.references) {
                references.ensure(env);
            }
            let mut facts = Vec::with_capacity(queries.len());
            for query in queries {
                check(ctx)?;
                let mut fact = blank_symbol(query.id.clone());
                let Some(symbol) = env.symbol_for(&query.id) else {
                    facts.push(fact);
                    continue;
                };
                if !query.references_only {
                    if symbol.flags().intersects(SymbolFlags::Alias) {
                        let original = env.checker.get_aliased_symbol(symbol);
                        fact.alias_target = env.id_for(original);
                    }
                    fact.declarations = declarations_for_symbol(symbol).into();
                }
                if query.references {
                    let canonical = env.canonical_symbol(symbol);
                    let canonical = env.id_for(canonical);
                    references.mark_used(&canonical);
                    fact.references = references.locations(&canonical).into();
                }
                facts.push(fact);
            }
            Ok(facts)
        })?
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
        let incremental = incremental && !incremental_path.is_empty();

        let generation = self.live()?;
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
            build_program(ctx, &self.config_path, &candidate_fs, self.config_host)
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
        accepted.with_checker(|checker| {
            ids.after_update(
                generation_number,
                program,
                checker,
                incremental.then_some(incremental_path.as_str()),
                current_exports,
            );
        });

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
        // An incremental update keeps reference contributions (and, with the
        // `Source*` families, source facts) of files outside the affected set:
        // their text is unchanged and every durable identity they reference
        // declares in an unchanged file (a changed declaring file would have
        // put the referencing file in the affected set). Files that left the
        // program are evicted now, not when they are next queried, so an entry
        // cannot go stale while its file is outside the program and be reused
        // if the file later re-enters. A full rebuild (deletes, tsconfig
        // changes, multi-file updates) can change resolution outside the
        // module graph: it fails closed and sweeps the durable identities of
        // files that left the program.
        if incremental {
            let dropped: std::collections::HashSet<&str> =
                affected.iter().map(String::as_str).collect();
            self.references.invalidate(
                &affected,
                &|path| {
                    program
                        .get_source_file(path)
                        .is_some_and(|file| !file.is_declaration_file())
                },
                &|path| !dropped.contains(path) && program.get_source_file(path).is_some(),
            );
            for path in &affected {
                self.declaration_shapes.remove(&clean(path));
            }
            if let Some(shape) = new_shape {
                self.declaration_shapes
                    .insert(incremental_path.clone(), shape);
            }
        } else {
            self.references.reset();
            self.ids.sweep_durable(program);
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
                ],
            );
        }
        Ok(UpdateOutcome { affected })
    }

    fn point_config_host_at_accepted_overlay(&self) {
        *self
            .config_host
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
        let fresh = Generation::build(generation.command_line(), host);
        // The identity maps and the generation-scoped reference ids name the
        // chain about to be freed.
        self.ids.release_pointers();
        self.references.rebase();
        generation.commit(fresh);
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
fn validate_location(location: &Location, source: &str) -> Result<(), BackendError> {
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
fn declaration_kind(node: tsrs_core::P<Node>) -> &'static str {
    if is_variable_declaration(node) {
        "variable"
    } else if is_function_declaration(node) {
        "function"
    } else if is_class_declaration(node) {
        "class"
    } else {
        "declaration"
    }
}

fn wire_symbol_name(name: &str) -> String {
    match name.strip_prefix('\u{7f}') {
        Some(rest) => format!("__{rest}"),
        None => name.to_owned(),
    }
}

/// Go: `declarationsForSymbol`.
fn declarations_for_symbol(symbol: tsrs_core::P<tsrs_ast::Symbol>) -> Vec<Declaration> {
    let mut declarations = Vec::with_capacity(symbol.declarations().len());
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
            // Go's codec spells the internal-name marker `__` at the wire seam
            // (`wireSymbolName`); tsrs's marker is U+007F, so spell it here.
            name: wire_symbol_name(symbol.name()).into(),
            kind: declaration_kind(*node).into(),
            location: Location {
                path: clean(file.file_name()).into(),
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
