//! The tsrs-backed project (Go: `tsgo/project.go`, the compiler half S3 ports).
//!
//! `Project` implements [`Backend`](crate::backend::Backend) for everything
//! that is not a fact family: opening a configured project over the overlay
//! file system, the single-checker program, `sources` and source digests, the
//! `modules` graph, and transactional `update` with the affected set, the
//! retained import graph and the declaration-shape cutoff.
//!
//! The fact families (`semantic_demand_runs`, `async_functions_at`,
//! `references_batch`, `changed_references`, `resolve_alias`, `declarations`
//! and so the default `symbol_evidence`) are S4. Until then they answer an
//! error naming the missing slice, never an empty or guessed fact: an empty
//! `resolve_alias` would claim "this symbol has no alias", which is a fact the
//! backend does not hold. The retained stores that exist only to serve those
//! families (`exportedIdentities`, `idsBySymbol`, `sourceFactsMemo`,
//! `durableRefs`, `referenceIndex`, `sweepDurableIdentities`) are S4's; the
//! seams they hook into are marked `S4:` below.
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
use crate::import_graph::{
    ImportGraph, affected_from_reverse_deps, every_source_path, global_scope_change,
};
use crate::model::{SemanticDemandRun, SemanticDemandRunResult, SemanticScope, SymbolId};
use crate::module_graph::{normalize_type_script_path, type_script_path_dir};
use crate::overlay::OverlayFs;
use crate::shape::{
    DeclarationShape, declaration_shape_for, exported_durable_symbols,
    has_global_or_module_augmentation, preserve_export_identities,
};
use crate::trace::SharedTrace;
use crate::unsafe_boundary::{Generation, UpdateKind};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tsrs_ast::is_external_module;
use tsrs_compiler::new_compiler_host;
use tsrs_core::{CompilerOptions, P};
use tsrs_tsoptions::{ParseConfigHost, get_parsed_command_line_of_config_file};
use tsrs_vfs::{Entries, FS, FileInfo, bundled, osvfs};
use typefacts::producer::source_digest;
use typefacts::v3::{FileChange, ModuleGraphRequest, SourceFile};
use typefacts::{AsyncFunctionFact, Declaration, Location, SourceDigest};

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
        };
        // S4: `collectExportedIdentities` seeds the symbol store here.
        project.graph =
            ImportGraph::rebuild(project.generation.as_ref().expect("just built").program());
        Ok(project)
    }

    fn live(&self) -> Result<&Generation, BackendError> {
        self.generation
            .as_ref()
            .ok_or_else(|| BackendError(ERR_CLOSED.to_owned()))
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
        self.generation = None;
        self.graph = ImportGraph::default();
        self.declaration_shapes.clear();
        Ok(())
    }

    fn resolve_alias(&mut self, _: &Context, _: &str) -> Result<Option<SymbolId>, BackendError> {
        Err(unavailable("resolve_alias"))
    }

    fn declarations(
        &mut self,
        _: &Context,
        _: &str,
    ) -> Result<Option<Vec<Declaration>>, BackendError> {
        Err(unavailable("declarations"))
    }

    fn semantic_demand_runs(
        &mut self,
        _: &Context,
        _: &[SemanticDemandRun<'_>],
        _: &SemanticScope<'_>,
    ) -> Result<Vec<SemanticDemandRunResult>, BackendError> {
        Err(unavailable("semantic_demand_runs"))
    }

    fn async_functions_at(
        &mut self,
        _: &Context,
        _: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        Err(unavailable("async_functions_at"))
    }

    fn references_batch(
        &mut self,
        _: &Context,
        _: &[SymbolId],
    ) -> Result<HashMap<SymbolId, Vec<Location>>, BackendError> {
        Err(unavailable("references_batch"))
    }

    fn changed_references(&mut self, _: &Context) -> Result<(Vec<SymbolId>, bool), BackendError> {
        Err(unavailable("changed_references"))
    }

    /// Go: `ReleaseAnalysisState`. S3 holds no checker-owned maps; the checker
    /// itself goes, and the next shape computation recreates it. (S4 clears
    /// `idsBySymbol`, `symbolsByID` and the other pointer-keyed caches here.)
    fn release_analysis_state(&mut self) {
        if let Some(generation) = &mut self.generation {
            generation.release_checker();
        }
    }
}

impl SymbolEvidence for Project {}

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
        let stage = Instant::now();
        if no_importers
            && let Some(file) = candidate.program().get_source_file(&incremental_path)
            && is_external_module(file)
            && !has_global_or_module_augmentation(file)
            && exported_durable_symbols(&candidate, file).is_some()
        {
            // S4: `currentExports` re-keys the symbol maps here.
            semantic_cutoff = true;
            leaf_cutoff = true;
        }
        let mut new_shape: Option<DeclarationShape> = None;
        if let Some(old) = &old_shape {
            new_shape = declaration_shape_for(&candidate, &incremental_path);
            if let Some(new) = &new_shape {
                semantic_cutoff = preserve_export_identities(old, new);
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
        let program = self.generation.as_ref().expect("just committed").program();

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
        // S4: an incremental update keeps source facts and reference
        // contributions of files outside the affected set; a full rebuild
        // clears them and sweeps durable identities of files that left the
        // program.
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
        generation.commit(fresh);
        true
    }
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
