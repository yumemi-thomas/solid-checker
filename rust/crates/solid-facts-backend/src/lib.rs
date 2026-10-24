//! Rust-led orchestration of Oxc AST facts, Solid execution facts, and
//! tsrs semantic facts.

#[cfg(not(feature = "dialect-v2"))]
compile_error!("solid-facts-backend requires at least one dialect feature");

mod cache;
pub mod check;

/// Whether published artifacts are flushed to stable storage before they are
/// made visible. Atomic visibility never changes: every publication still
/// writes to a temporary path and renames it into place, so a reader sees a
/// whole file or nothing. What `SOLID_CHECKER_DURABLE_WRITES=none` relaxes is
/// crash durability — the `fsync` (an `F_FULLFSYNC` full-disk flush on Apple
/// platforms) of the file and its directory before the rename. That is the
/// right trade only where the published artifact is scratch, as in the
/// ecosystem benchmark, whose catalogs live in a temporary directory removed
/// seconds later; the product default flushes. Read once per process.
pub fn durable_writes_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        !matches!(
            std::env::var("SOLID_CHECKER_DURABLE_WRITES")
                .ok()
                .as_deref()
                .map(str::trim),
            Some("none") | Some("0")
        )
    })
}

/// `File::sync_all` under the process durability policy; see
/// [`durable_writes_enabled`].
pub fn durable_sync(file: &std::fs::File) -> std::io::Result<()> {
    if durable_writes_enabled() {
        file.sync_all()
    } else {
        Ok(())
    }
}

mod demand_plan;
pub mod dependency_program;
pub mod dependency_sources;
mod diagnostics;
pub mod dialect;
mod host_config;
pub mod host_constants;
mod host_inline;
mod host_invocation;
mod host_loadable;
mod host_plugins;
mod inferred_host;
pub mod installed_bytes;
mod package_requirements;
mod release_scope;
pub mod runtime_resolution;
pub mod source_redirects;
mod wire;

pub use cache::{CacheStats, FactsCache};
pub use diagnostics::{
    DiagnosticAnalysis, DiagnosticSession, DiagnosticTimings, Metrics, RequestedRuleEnablement,
    Snapshot, SnapshotEvidence, SnapshotFinding, SnapshotFix, SnapshotTextEdit, SourceLocation,
    analysis_metrics, analyze_project_inputs_measured_with_enablement,
    discovered_rule_options_path, imported_package_manifest_paths, imported_package_roots,
    semantic_demand_options_for_enablement, source_location, unsupported_runtime_snapshot,
};
pub use host_config::{tsconfig_extends_chain, tsconfig_extends_witness};
pub use inferred_host::{
    InferredHost, inferred_host_directory_digest, inferred_host_input_digest,
    inferred_host_input_paths, inferred_host_input_paths_for_project,
    inferred_host_input_paths_for_sources, inferred_project_host_with_note,
};
pub use package_requirements::{external_package_boundary, unsupplied_package_boundary};
/// The shared analysis input an in-memory adapter builds without the
/// filesystem.
pub use solid_reactive_ir::{
    AnalysisInputs, RuntimeEnvironment, package_boundary::PackageBoundary,
};
pub use wire::{
    SemanticDemandGroup, SourceChange, SourceFile, TypeFactsExchangeTimings, TypeFactsProvider,
};

#[must_use]
pub fn default_typefacts_executable() -> String {
    if let Some(value) = std::env::var_os("SOLID_TYPEFACTS_BIN")
        && !value.is_empty()
    {
        return value.to_string_lossy().into_owned();
    }
    let name = if cfg!(windows) {
        "solid-typefacts.exe"
    } else {
        "solid-typefacts"
    };
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        let sibling = directory.join(name);
        if sibling.is_file() {
            return sibling.to_string_lossy().into_owned();
        }
    }
    name.into()
}

/// Optional semantic facts needed only by explicitly enabled rule families.
/// Default certification and WASM analysis request none of these facts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticDemandOptions {
    pub array_map_receiver_types: bool,
}

impl SemanticDemandOptions {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
    };

    pub const PREFERENCES: Self = Self {
        array_map_receiver_types: true,
    };
}

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use solid_facts::compiler::{AnalysisRequest, CompilerOptions, ExecutionMap};
use solid_facts::core::{Generation, SourceHash, Span};

use crate::dialect::Dialect;
use solid_facts::resolution::{AttestedImport, AttestedImportIndex, ImportResolution};
use solid_facts::{FileFacts, ProjectFacts, TypeScriptChanges, TypeScriptTable};
use thiserror::Error;

pub use solid_facts::compiler::{CompilerFactsProvider, CompilerProviderError};

/// The [`ExecutionMap`] for a `.json` module: no dialect compiler runs on it.
///
/// `solid_facts::ast::is_json_module_path` already routed this file's syntax
/// facts to [`solid_facts::ast::AstFacts::empty`] instead of the JS/JSX
/// parser; a JSON module has no tracked/untracked regions, ownership
/// regions, callback roles, or JSX operations for the same reason its
/// syntax table is empty -- there is no executable code for the compiler to
/// trace. Asking the dialect compiler (a JS/JSX grammar) to analyze JSON
/// text would either error or, worse, silently misparse it; recording the
/// proven-empty map is the correct fact, not a fallback.
fn inert_execution_map(source_hash: SourceHash) -> ExecutionMap {
    ExecutionMap::inert(source_hash)
}

/// The compiler facts of one source file, or, when the dialect compiler
/// refuses the file, [`ExecutionMap::refused`] (ADR 0360): the file keeps its
/// syntax and Type Facts, carries no compiler facts, and the analysis of the
/// rest goes on. Every other compiler error (no trace, a trace this checker
/// cannot trust) stays fatal.
fn file_execution_map(
    compiler: &mut (impl CompilerFactsProvider + ?Sized),
    request: &AnalysisRequest,
) -> Result<ExecutionMap, BackendError> {
    if solid_facts::ast::is_json_module_path(&request.path) {
        return Ok(inert_execution_map(request.source_hash.clone()));
    }
    match compiler.analyze(request) {
        Ok(execution) => Ok(execution),
        Err(CompilerProviderError::Refused(message)) => {
            Ok(ExecutionMap::refused(request.source_hash.clone(), message))
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("generation must be non-zero")]
    Generation,
    #[error("process error: {0}")]
    Process(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("AST facts error: {0}")]
    Ast(#[from] solid_facts::ast::AstFactsError),
    #[error("compiler facts error: {0}")]
    Compiler(#[from] solid_facts::compiler::CompilerFactsError),
    #[error("TypeFacts error: {0}")]
    TypeFacts(#[from] typefacts::TypeFactsError),
    #[error("TypeFacts session error: {0}")]
    TypeFactsSession(#[from] typefacts::SessionError),
    #[error("fact join error: {0}")]
    Join(#[from] solid_facts::JoinError),
    #[error("the Solid compiler returned no semantic trace")]
    MissingExecutionMap,
    #[error("native Solid compiler facts error: {0}")]
    NativeCompiler(String),
    #[error("TypeFacts service {code}: {message}")]
    TypeFactsService { code: String, message: String },
    #[error("reactive IR error: {0}")]
    ReactiveIr(#[from] solid_reactive_ir::BuildError),
    #[error("runtime premise error: {0}")]
    RuntimePremise(String),
    #[error("TypeFacts compatibility handshake failed: {0}")]
    Handshake(String),
    #[error("rule options error: {0}")]
    RuleOptions(String),
    #[error("dialect {dialect:?} produced unknown rule identities: {rules:?}")]
    UnknownRuleIdentity {
        dialect: &'static str,
        rules: Vec<String>,
    },
    #[error("analysis cancelled")]
    Cancelled,
}

impl BackendError {
    #[must_use]
    pub fn is_typefacts_transport_failure(&self) -> bool {
        matches!(
            self,
            Self::Process(_) | Self::Io(_) | Self::TypeFacts(typefacts::TypeFactsError::Io(_))
        )
    }
}

/// Adapter errors fold into the backend's existing variants so callers and
/// tests keep seeing the messages they always saw.
impl From<CompilerProviderError> for BackendError {
    fn from(error: CompilerProviderError) -> Self {
        match error {
            CompilerProviderError::MissingExecutionMap => Self::MissingExecutionMap,
            CompilerProviderError::Native(message) | CompilerProviderError::Refused(message) => {
                Self::NativeCompiler(message)
            }
            CompilerProviderError::Facts(source) => Self::Compiler(source),
        }
    }
}

const TYPEFACTS_RECOVERY_ATTEMPTS: u32 = 3;

/// The checker's handle on one retained TypeFacts producer process.
///
/// A thin adapter over [`typefacts::Session`]: the crate owns the transport,
/// the handshake, retained demands and delta application, and this maps its
/// vocabulary onto the backend's error and timing types.
pub struct TypeFactsSession {
    session: typefacts::Session,
    last_exchange_timings: Option<TypeFactsExchangeTimings>,
    last_table_changes: Option<TypeScriptChanges>,
}

impl TypeFactsSession {
    /// Starts a producer and opens the project on it.
    ///
    /// `arguments` carries producer-specific flags only; the session appends
    /// the `-project` argument itself.
    pub fn open(
        executable: &str,
        project_id: &str,
        arguments: &[String],
    ) -> Result<Self, BackendError> {
        let mut producer = typefacts::Producer::at(executable);
        for argument in arguments {
            producer = producer.with_arg(argument);
        }
        let session = typefacts::Session::open(producer, project_id, Vec::new())?;
        Ok(Self {
            session,
            last_exchange_timings: None,
            last_table_changes: None,
        })
    }

    /// A session over a producer program its owner already opened and carries
    /// frame by frame (`typefacts::FrameBridge`): the owned WASM producer.
    /// Every answer decodes, validates and expands exactly as a process
    /// session's; the session never relaunches, replays or certifies.
    pub fn open_bridged(
        bridge: Box<dyn typefacts::FrameBridge + Send>,
        project_id: &str,
    ) -> Result<Self, BackendError> {
        let session = typefacts::Session::open_bridged(bridge, project_id)?;
        Ok(Self {
            session,
            last_exchange_timings: None,
            last_table_changes: None,
        })
    }

    /// The project's configured source set, as the TypeScript program sees it.
    pub fn configured_sources(&mut self) -> Result<Vec<SourceFile>, BackendError> {
        Ok(self
            .session
            .configured_sources()?
            .into_iter()
            .map(|source| SourceFile {
                path: source.path,
                source: String::from_utf8_lossy(&source.source).into_owned().into(),
                compiler_options: CompilerOptions::default(),
            })
            .collect())
    }

    /// The resolved module graph of the open generation: every file the
    /// analyzing program included, and — where the demand asks — where each
    /// specifier of a named file resolved.
    ///
    /// A read of the retained program, exactly like
    /// [`Self::configured_sources`]: it carries no state token, edits no
    /// retained demand set, and leaves a materialized analysis untouched. It is
    /// what turns a closure record from a reconstruction into an attestation,
    /// and it is issued only where a caller asks for one — never on an ordinary
    /// analysis run, which has no consumer for it.
    ///
    /// The producer fails the request rather than answering a partial
    /// inventory, and
    /// [`ModuleGraph::is_complete`](typefacts::ModuleGraph::is_complete) is the
    /// caller's signal that a scoped answer covered less than it asked for.
    /// Neither may be reconciled against a weaker local reconstruction.
    pub fn module_graph(
        &mut self,
        demand: &typefacts::ModuleGraphDemand,
    ) -> Result<typefacts::ModuleGraph, BackendError> {
        Ok(self.session.module_graph(demand)?)
    }

    /// The validity this program answers for the call demanded at each of
    /// `locations` (`None` where it answers no resolved call), in order. One
    /// analysis whose demand set is exactly these calls: it replaces any
    /// retained one, so it is meant for a session that is otherwise only read
    /// (`configured_sources`, `module_graph`).
    pub fn call_validities(
        &mut self,
        locations: &[typefacts::Location],
    ) -> Result<Vec<Option<typefacts::ResolvedCallValidity>>, BackendError> {
        let demand = typefacts::AnalysisDemand {
            entities: locations
                .iter()
                .map(|location| typefacts::v3::EntityDemand {
                    location: location.clone(),
                    resolved_call: true,
                    ..typefacts::v3::EntityDemand::default()
                })
                .collect(),
        };
        let table = self.session.analyze(&demand)?;
        Ok(locations
            .iter()
            .map(|location| {
                table
                    .entities_for_path(&location.path)
                    .iter()
                    .find(|entity| entity.location == *location)
                    .and_then(|entity| entity.resolved_call.as_ref())
                    .map(|call| call.validity)
            })
            .collect())
    }

    /// Applies an overlay and advances the generation.
    pub fn update(&mut self, changes: Vec<typefacts::v3::FileChange>) -> Result<(), BackendError> {
        self.session.update(changes)?;
        Ok(())
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.session.generation()
    }

    #[must_use]
    pub fn cancellation_handle(&self) -> Option<typefacts::Cancellation> {
        self.session.cancellation_handle()
    }

    fn record(&mut self) {
        self.last_exchange_timings =
            self.session
                .take_last_exchange_timings()
                .map(|timings| TypeFactsExchangeTimings {
                    roundtrip: timings.roundtrip,
                    request_send: timings.request_send,
                    request_bytes: timings.request_bytes,
                    response_decode: timings.response_decode,
                    response_bytes: timings.response_bytes,
                    server_request_decode: timings.server_request_decode,
                    server_analyze: timings.server_analyze,
                    server_async: timings.server_async,
                    server_demand: timings.server_demand,
                    server_assembly: timings.server_assembly,
                    server_sort: timings.server_sort,
                    server_close_symbols: timings.server_close_symbols,
                    server_materialized: timings.server_materialized,
                    server_retained_files: timings.server_retained_files,
                    server_recomputed_files: timings.server_recomputed_files,
                    server_non_durable_files: timings.server_non_durable_files,
                });
        self.last_table_changes =
            self.session
                .take_last_table_changes()
                .map(|changes| TypeScriptChanges {
                    unchanged: changes.unchanged,
                    entity_paths: changes.entity_paths,
                    symbol_ids: changes.symbol_ids,
                    file_paths: changes.file_paths,
                });
    }
}

impl TypeFactsProvider for TypeFactsSession {
    fn semantic_grouped(
        &mut self,
        groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError> {
        let borrowed = groups
            .iter()
            .filter_map(|group| {
                group.shared_demands.map_or_else(
                    || typefacts::DemandGroup::new(group.demands),
                    typefacts::DemandGroup::shared,
                )
            })
            .collect::<Vec<_>>();
        let table = self.session.analyze_groups(&borrowed)?;
        self.record();
        Ok(TypeScriptTable::retained(table))
    }

    fn take_last_exchange_timings(&mut self) -> Option<TypeFactsExchangeTimings> {
        self.last_exchange_timings.take()
    }

    fn take_last_table_changes(&mut self) -> Option<TypeScriptChanges> {
        self.last_table_changes.take()
    }
}

/// A retained editor session with both Oxc and the Solid compiler running
/// in-process. tsrs is the only process boundary.
pub struct NativeIncrementalSession {
    dialect: &'static Dialect,
    project_id: String,
    generation: u64,
    sources: HashMap<String, SourceFile>,
    cache: FactsCache,
    last_facts: Option<Arc<ProjectFacts>>,
    typescript: TypeFactsSession,
    known_paths: HashSet<String>,
    last_build_timings: NativeBuildTimings,
    last_import_identity: ImportIdentityMeasurement,
    semantic_demand_options: SemanticDemandOptions,
    /// Per path, the digest of the program's text where the session's source
    /// is a span-preserving rewrite of it (ADR 0166's host-constant fold, as
    /// [`build_project_native_measured_with_program_hashes`] takes it).
    program_hashes: HashMap<String, SourceHash>,
    facts_transform: Option<FactsTransform>,
}

/// An edited path's previous source and program digest, restored when the
/// edit never lands.
type DisplacedSource = (String, Option<SourceFile>, Option<SourceHash>);

/// A step every generation's joined facts pass through before their import
/// identities are attested, with the session's own Type Facts program (ADR
/// 0296: a dependency-source program's roots and redirects). It runs on every
/// path that produces facts, so no generation's facts can miss it.
pub type FactsTransform =
    Box<dyn FnMut(&mut ProjectFacts, &mut TypeFactsSession) -> Result<(), BackendError>>;

/// The importing files whose module specifiers are attested: every file of
/// the program.
///
/// Two consumers read the answer:
/// - A package is matched by installed identity, which needs the compiler's
///   own resolution of the specifier.
/// - The IR asks which module each specifier loads before it trusts that no
///   namespace object, barrel or dynamic import reaches a module (ADR 0219).
///   That includes relative specifiers, since only the compiler maps
///   `./x.js` to `x.ts`.
///
/// Both questions hold for any file, so the scope is not narrowed to files
/// with a bare specifier. It is one request per program generation, and its
/// import half is proportional to the files asked about.
///
/// The scope deliberately does **not** consult package discovery. The
/// attestation is computed once per program generation and a retained session
/// reuses it across checks, while packages are re-discovered on every check.
/// A scope keyed on today's packages would answer for a package that
/// appeared afterwards by *silently omitting* its files, which is name-only
/// binding restored by accident.
///
/// An empty answer means the program has no files, and the caller then asks
/// for nothing.
#[must_use]
pub fn import_identity_scope(facts: &ProjectFacts) -> Vec<String> {
    facts
        .files
        .iter()
        .map(|file| file.path.as_str().to_owned())
        .collect()
}

/// What one import-identity attestation cost and covered.
///
/// Reported under `SOLID_CHECKER_TIMINGS` so the operation's cost is
/// attributable rather than guessed at, and so a scope the program could not
/// answer for is countable instead of silently failing closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ImportIdentityMeasurement {
    /// Importing files asked about.
    pub requested: usize,
    /// Importing files the answer covered.
    pub attested: usize,
    /// Requested files the program does not hold under that path. Every
    /// specifier in one of these is refused, so a non-zero count is a plumbing
    /// defect and not a project property.
    pub unknown: usize,
    /// Specifiers attested.
    pub specifiers: usize,
    /// Files the program included, which the operation always answers for.
    pub modules: usize,
}

/// Asks the compiler where each specifier in `scope` resolves, and to which
/// installed package.
///
/// One round trip, both halves read off the already-built program: the module
/// inventory (unconditional — it is the operation's reason to exist) and the
/// import provenance of the requested files only. `packages: true` adds the
/// owning manifest and the resolver's own recorded identity to each row, which
/// is the fact a package is bound by.
///
/// A requested file the program does not hold is reported by the producer
/// rather than dropped, and it is recorded here as *not covered*: the
/// boundary then refuses every specifier in it. Presenting an unanswered file as "this
/// file imports nothing" would silently restore name-only binding for it.
pub fn attest_import_identities(
    typescript: &mut TypeFactsSession,
    scope: &[String],
) -> Result<(AttestedImportIndex, ImportIdentityMeasurement), BackendError> {
    let graph = typescript.module_graph(
        &typefacts::ModuleGraphDemand::default()
            .import_paths(scope.iter().cloned())
            .with_packages(),
    )?;
    let unknown = graph
        .unknown_import_paths
        .iter()
        .map(|path| path.to_string())
        .collect::<HashSet<_>>();
    let mut rows = HashMap::<&str, Vec<AttestedImport>>::new();
    for import in &graph.imports {
        rows.entry(&import.specifier.path)
            .or_default()
            .push(AttestedImport {
                span: solid_facts::core::Span::new(
                    u32::try_from(import.specifier.start_byte).unwrap_or(u32::MAX),
                    u32::try_from(import.specifier.end_byte).unwrap_or(u32::MAX),
                ),
                text: import.text.as_ref().into(),
                resolution: match import.resolution {
                    typefacts::ModuleResolution::Unresolved => ImportResolution::Unresolved,
                    typefacts::ModuleResolution::Relative => ImportResolution::Relative,
                    typefacts::ModuleResolution::NodeModules => ImportResolution::NodeModules,
                    typefacts::ModuleResolution::NonRelative => ImportResolution::NonRelative,
                },
                resolved_path: Arc::clone(&import.resolved_path),
                included_path: Arc::clone(&import.included_path),
                symlink_path: Arc::clone(&import.symlink_path),
                extension: Arc::clone(&import.extension),
                // An empty name is a manifest that declares none — the
                // `{"type":"module"}` file a published package ships beside its
                // output — and is carried as absent rather than as the empty
                // string, so no comparison can read it as a disagreement.
                package_name: import
                    .package
                    .as_ref()
                    .map(|package| package.name.as_ref())
                    .filter(|name| !name.is_empty())
                    .map(Into::into),
                package_version: import
                    .package
                    .as_ref()
                    .map(|package| package.version.as_ref())
                    .filter(|version| !version.is_empty())
                    .map(Into::into),
                package_manifest: import
                    .package
                    .as_ref()
                    .map(|package| Arc::clone(&package.manifest_path)),
                resolver_package_name: import
                    .resolver_package
                    .as_ref()
                    .map(|package| package.name.as_ref())
                    .filter(|name| !name.is_empty())
                    .map(Into::into),
                resolver_package_version: import
                    .resolver_package
                    .as_ref()
                    .map(|package| package.version.as_ref())
                    .filter(|version| !version.is_empty())
                    .map(Into::into),
            });
    }
    let mut index = AttestedImportIndex::default();
    let mut specifiers = 0;
    let mut attested = 0;
    for path in scope {
        if unknown.contains(path) {
            continue;
        }
        let file = rows.remove(path.as_str()).unwrap_or_default();
        specifiers += file.len();
        attested += 1;
        index.insert_file(path.as_str(), file);
    }
    Ok((
        index,
        ImportIdentityMeasurement {
            requested: scope.len(),
            attested,
            unknown: unknown.len(),
            specifiers,
            modules: graph.modules.len(),
        },
    ))
}

impl NativeIncrementalSession {
    pub fn open(
        dialect: &'static Dialect,
        project_id: String,
        sources: Vec<SourceFile>,
        typescript: TypeFactsSession,
    ) -> Result<Self, BackendError> {
        Ok(Self::from_sources(dialect, project_id, sources, typescript))
    }

    /// Opens the project and returns the session together with its configured
    /// sources, so callers can seed their own bookkeeping. `TypeFactsSession`
    /// has already issued the `open`, so callers must not open again.
    pub fn open_pipelined(
        dialect: &'static Dialect,
        project_id: String,
        mut typescript: TypeFactsSession,
    ) -> Result<(Self, Vec<SourceFile>), BackendError> {
        let sources = typescript.configured_sources()?;
        let session = Self::from_sources(dialect, project_id, sources.clone(), typescript);
        Ok((session, sources))
    }

    /// A session whose `sources` are, where `program_hashes` names a path, a
    /// span-preserving rewrite of the text the Type Facts program holds. The
    /// join compares those digests instead, exactly as
    /// [`build_project_native_measured_with_program_hashes`] does for a
    /// one-shot build. An edit replaces a path's text with the program's own,
    /// so it drops that path's entry.
    #[must_use]
    pub fn open_rewritten(
        dialect: &'static Dialect,
        project_id: String,
        sources: Vec<SourceFile>,
        program_hashes: HashMap<String, SourceHash>,
        typescript: TypeFactsSession,
    ) -> Self {
        let mut session = Self::from_sources(dialect, project_id, sources, typescript);
        session.program_hashes = program_hashes;
        session
    }

    /// Install the [`FactsTransform`] every later generation passes through.
    /// Facts already joined are discarded, so none escapes it.
    pub fn set_facts_transform(&mut self, transform: FactsTransform) {
        self.facts_transform = Some(transform);
        self.last_facts = None;
    }

    fn from_sources(
        dialect: &'static Dialect,
        project_id: String,
        sources: Vec<SourceFile>,
        typescript: TypeFactsSession,
    ) -> Self {
        Self {
            dialect,
            project_id,
            generation: 1,
            known_paths: sources.iter().map(|source| source.path.clone()).collect(),
            sources: sources
                .into_iter()
                .map(|source| (source.path.clone(), source))
                .collect(),
            cache: FactsCache::default(),
            last_facts: None,
            typescript,
            last_build_timings: NativeBuildTimings::default(),
            last_import_identity: ImportIdentityMeasurement::default(),
            semantic_demand_options: SemanticDemandOptions::NONE,
            program_hashes: HashMap::new(),
            facts_transform: None,
        }
    }

    /// Select facts needed by explicitly enabled preferences. Enabling a
    /// demand after a same-generation default analysis invalidates only the
    /// joined facts; AST/compiler caches remain reusable. Disabling can reuse
    /// an existing superset until the next source edit.
    pub fn set_semantic_demand_options(&mut self, options: SemanticDemandOptions) {
        if options.array_map_receiver_types
            && !self.semantic_demand_options.array_map_receiver_types
        {
            self.last_facts = None;
        }
        self.semantic_demand_options = options;
    }

    /// One edit exchange: the update that advances the generation always
    /// lands, and only the analyze half is cancellable. Local preparation
    /// (Oxc parse, native compiler facts, demand assembly) overlaps the
    /// update round trip; the analyze request is sent once the service has
    /// acknowledged the new generation. A transport failure restarts the
    /// service, replays local state, and retries exactly the half that
    /// failed.
    pub fn edit(
        &mut self,
        changes: Vec<SourceChange>,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Arc<ProjectFacts>, BackendError> {
        check_cancelled(cancelled)?;
        if changes.is_empty() {
            return self.analyze_with_recovery(cancelled);
        }
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(BackendError::Generation)?;
        self.known_paths
            .extend(changes.iter().map(|change| change.path.clone()));
        let wire_changes = changes
            .iter()
            .map(|change| typefacts::v3::FileChange {
                path: change.path.clone(),
                version: change.version,
                source: change
                    .source
                    .as_deref()
                    .map_or_else(Vec::new, |source| source.as_bytes().to_vec()),
                deleted: change.source.is_none(),
            })
            .collect::<Vec<_>>();
        // Apply the overlay locally before anything is sent: demand assembly
        // reads these sources while the update round trip is in flight. The
        // displaced entries restore the overlay if the update never lands.
        let mut displaced = Vec::with_capacity(changes.len());
        for change in changes {
            displaced.push((
                change.path.clone(),
                self.sources.get(&change.path).cloned(),
                self.program_hashes.remove(&change.path),
            ));
            self.cache.invalidate_path(&change.path);
            if let Some(source) = change.source {
                self.sources.insert(
                    change.path.clone(),
                    SourceFile {
                        path: change.path,
                        source: source.into(),
                        compiler_options: change.compiler_options,
                    },
                );
            } else {
                self.sources.remove(&change.path);
            }
        }
        let mut update_landed = false;
        let mut attempt = 0_u32;
        loop {
            let result = self.edit_attempt(
                next_generation,
                &wire_changes,
                &mut update_landed,
                cancelled,
            );
            if update_landed {
                self.generation = next_generation;
            }
            match result {
                Err(error)
                    if error.is_typefacts_transport_failure()
                        && attempt < TYPEFACTS_RECOVERY_ATTEMPTS =>
                {
                    std::thread::sleep(Duration::from_millis(25_u64 << attempt));
                    attempt += 1;
                    if let Err(recovery) = self.recover_typefacts() {
                        if !update_landed {
                            self.restore_overlay(displaced);
                        }
                        return Err(BackendError::Process(format!(
                            "{error}; recovery failed: {recovery}"
                        )));
                    }
                }
                Err(error) => {
                    if !update_landed {
                        self.restore_overlay(displaced);
                    }
                    return Err(error);
                }
                Ok(facts) => {
                    let facts = Arc::new(self.attested(facts)?);
                    self.last_facts = Some(Arc::clone(&facts));
                    return Ok(facts);
                }
            }
        }
    }

    /// One attempt at the edit exchange. When the update has not landed yet,
    /// it is written first and awaited by the type-facts provider right
    /// before the analyze request goes out — or after the build returns, if
    /// the build never reached the semantic stage. A written update is
    /// always awaited: the service applies it regardless, so returning
    /// without committing the generation would desynchronize the session.
    fn edit_attempt(
        &mut self,
        next_generation: u64,
        wire_changes: &[typefacts::v3::FileChange],
        update_landed: &mut bool,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<ProjectFacts, BackendError> {
        // The session applies the overlay and advances its own generation.
        // `Session` owns transport recovery, so a failure here is already
        // past its internal restart-and-replay.
        if !*update_landed {
            self.typescript.update(wire_changes.to_vec())?;
            *update_landed = true;
            check_cancelled(cancelled)?;
        }
        let mut sources = self.sources.values().cloned().collect::<Vec<_>>();
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let changed_paths = wire_changes
            .iter()
            .map(|change| change.path.clone())
            .collect::<HashSet<_>>();
        let retained = self
            .last_facts
            .as_deref()
            .filter(|facts| facts.generation.get() == self.generation)
            .map(|facts| RetainedFileFacts {
                previous: facts,
                changed_paths: &changed_paths,
            });
        let result = build_project_native_cached_measured_inner(
            self.dialect,
            self.project_id.clone(),
            next_generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            cancelled,
            retained,
            self.semantic_demand_options,
            &self.program_hashes,
        );
        result.map(|(facts, timings)| {
            self.last_build_timings = timings;
            facts
        })
    }

    fn restore_overlay(&mut self, displaced: Vec<DisplacedSource>) {
        for (path, previous, program_hash) in displaced {
            self.cache.invalidate_path(&path);
            if let Some(hash) = program_hash {
                self.program_hashes.insert(path.clone(), hash);
            }
            match previous {
                Some(source) => {
                    self.sources.insert(path, source);
                }
                None => {
                    self.sources.remove(&path);
                }
            }
        }
    }

    fn analyze_with_recovery(
        &mut self,
        cancelled: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Arc<ProjectFacts>, BackendError> {
        let mut attempt = 0_u32;
        loop {
            let result = match cancelled {
                Some(flag) => self.analyze_cancellable(flag),
                None => self.analyze(),
            };
            match result {
                Err(error)
                    if error.is_typefacts_transport_failure()
                        && attempt < TYPEFACTS_RECOVERY_ATTEMPTS =>
                {
                    std::thread::sleep(Duration::from_millis(25_u64 << attempt));
                    attempt += 1;
                    if let Err(recovery) = self.recover_typefacts() {
                        return Err(BackendError::Process(format!(
                            "{error}; recovery failed: {recovery}"
                        )));
                    }
                }
                other => return other,
            }
        }
    }

    pub fn analyze(&mut self) -> Result<Arc<ProjectFacts>, BackendError> {
        if let Some(facts) = self
            .last_facts
            .as_ref()
            .filter(|facts| facts.generation.get() == self.generation)
        {
            self.last_build_timings = NativeBuildTimings::default();
            return Ok(Arc::clone(facts));
        }
        let mut sources = self.sources.values().cloned().collect::<Vec<_>>();
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let (facts, timings) = build_project_native_cached_measured_inner(
            self.dialect,
            self.project_id.clone(),
            self.generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            None,
            None,
            self.semantic_demand_options,
            &self.program_hashes,
        )?;
        self.last_build_timings = timings;
        let facts = Arc::new(self.attested(facts)?);
        self.last_facts = Some(Arc::clone(&facts));
        Ok(facts)
    }

    pub fn analyze_cancellable(
        &mut self,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Arc<ProjectFacts>, BackendError> {
        check_cancelled(Some(cancelled))?;
        if let Some(facts) = self
            .last_facts
            .as_ref()
            .filter(|facts| facts.generation.get() == self.generation)
        {
            self.last_build_timings = NativeBuildTimings::default();
            return Ok(Arc::clone(facts));
        }
        let mut sources = self.sources.values().cloned().collect::<Vec<_>>();
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let (facts, timings) = build_project_native_cached_measured_inner(
            self.dialect,
            self.project_id.clone(),
            self.generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            Some(cancelled),
            None,
            self.semantic_demand_options,
            &self.program_hashes,
        )?;
        self.last_build_timings = timings;
        let facts = Arc::new(self.attested(facts)?);
        self.last_facts = Some(Arc::clone(&facts));
        Ok(facts)
    }

    /// Attaches this program's attested import identities to the facts it
    /// produced, so a package reaching them is bound to the install
    /// each specifier resolves to rather than to its name.
    ///
    /// Every path that produces `ProjectFacts` from this session passes through
    /// here. A retained session reuses one generation's facts across many
    /// checks, so the attestation is computed once per generation with it — and
    /// it must not be conditional on anything that can change between checks
    /// (see [`import_identity_scope`]).
    fn attested(&mut self, mut facts: ProjectFacts) -> Result<ProjectFacts, BackendError> {
        if let Some(transform) = &mut self.facts_transform {
            transform(&mut facts, &mut self.typescript)?;
        }
        let scope = import_identity_scope(&facts);
        if scope.is_empty() {
            // This generation asked for nothing, and the measurement must say
            // so rather than leave the previous generation's counts standing.
            self.last_import_identity = ImportIdentityMeasurement::default();
            return Ok(facts);
        }
        let (index, measurement) = attest_import_identities(&mut self.typescript, &scope)?;
        self.last_import_identity = measurement;
        facts.resolved_imports = Some(index);
        Ok(facts)
    }

    /// What the last attestation on this session cost and covered.
    #[must_use]
    pub const fn last_import_identity(&self) -> ImportIdentityMeasurement {
        self.last_import_identity
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    #[must_use]
    pub const fn last_build_timings(&self) -> NativeBuildTimings {
        self.last_build_timings
    }

    #[must_use]
    pub fn cancellation_handle(&self) -> Option<typefacts::Cancellation> {
        self.typescript.cancellation_handle()
    }

    /// Every file the open program holds, by path. This session's source set
    /// is fixed when it opens, so a retained caller compares this across an
    /// edit: an edited import can add a file to the program or drop one.
    pub fn program_modules(&mut self) -> Result<Vec<String>, BackendError> {
        Ok(self
            .typescript
            .module_graph(&typefacts::ModuleGraphDemand::default())?
            .modules
            .into_iter()
            .map(|module| module.path.to_string())
            .collect())
    }

    /// The open program's module graph with each file's distinct non-relative
    /// import resolutions: what a retained caller observes to notice that the
    /// filesystem now resolves the program differently (a relinked package, an
    /// edited manifest). Relative imports and repeated specifiers read nothing
    /// more, and all of them together can exceed the frame limit.
    pub fn resolution_graph(&mut self) -> Result<typefacts::ModuleGraph, BackendError> {
        self.typescript
            .module_graph(&typefacts::ModuleGraphDemand::with_distinct_bare_imports())
    }

    /// Recovery is the session's own concern now.
    ///
    /// `typefacts::Session` restarts the producer and replays every retained
    /// generation from inside the failing exchange, so by the time an error
    /// reaches here it has already been through that. Kept so callers that
    /// used to drive recovery explicitly still compile.
    pub fn recover_typefacts(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}

pub fn build_project(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    compiler: &mut (impl CompilerFactsProvider + ?Sized),
    typescript: &mut impl TypeFactsProvider,
) -> Result<ProjectFacts, BackendError> {
    build_project_cached(
        dialect,
        project_id,
        generation,
        sources,
        compiler,
        typescript,
        &mut FactsCache::default(),
    )
}

pub fn build_project_native(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_measured(dialect, project_id, generation, sources, typescript)
        .map(|(facts, _)| facts)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeBuildTimings {
    pub source_analysis: Duration,
    pub source_files_reused: u64,
    pub source_files_recomputed: u64,
    pub ast_facts: Duration,
    pub compiler_facts: Duration,
    pub file_fact_assembly: Duration,
    pub type_facts: Duration,
    pub demand_assembly: Duration,
    pub request_assembly: Duration,
    pub semantic_demand_assembly: Duration,
    pub hydrate: Duration,
    pub join: Duration,
    pub exchange: TypeFactsExchangeTimings,
}

/// Splits `items` into one chunk per worker, runs `analyze` over each, and
/// concatenates the results in chunk order. `analyze` receives the index its
/// chunk's first item has in `items`.
///
/// When only one worker is available the whole pass runs inline on the calling
/// thread. That is a correctness requirement, not just an optimization: the
/// wasm32-wasip1 reactor build has no thread support, and `Scope::spawn`
/// panics with "failed to spawn thread" there rather than degrading. Every
/// chunked fan-out in this crate goes through this one shape so the fallback
/// cannot be forgotten at a new call site.
fn analyze_in_chunks<'items, T, R, E, F>(
    items: &'items [T],
    worker: &'static str,
    analyze: F,
) -> Result<Vec<R>, E>
where
    T: Sync,
    R: Send,
    E: Send,
    F: Fn(usize, &'items [T]) -> Result<Vec<R>, E> + Sync,
{
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(items.len().max(1));
    if workers <= 1 {
        return analyze(0, items);
    }
    let chunk_size = items.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .enumerate()
            .map(|(chunk_index, chunk)| {
                let analyze = &analyze;
                scope.spawn(move || analyze(chunk_index * chunk_size, chunk))
            })
            .collect::<Vec<_>>();
        let mut results = Vec::with_capacity(items.len());
        for handle in handles {
            results.extend(
                handle
                    .join()
                    .unwrap_or_else(|_| panic!("{worker} worker panicked"))?,
            );
        }
        Ok(results)
    })
}

pub fn build_project_native_measured(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    build_project_native_measured_with_demands(
        dialect,
        project_id,
        generation,
        sources,
        typescript,
        SemanticDemandOptions::NONE,
    )
}

pub fn build_project_native_measured_with_demands(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    semantic_demand_options: SemanticDemandOptions,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    build_project_native_measured_with_program_hashes(
        dialect,
        project_id,
        generation,
        sources,
        &HashMap::new(),
        typescript,
        semantic_demand_options,
    )
}

/// [`build_project_native_measured_with_demands`] for sources some of which are
/// a span-preserving rewrite of the text the Type Facts program holds (ADR
/// 0166's host-constant fold): `program_hashes` names, per path, the digest of
/// the program's text, which the join compares instead of the digest of the
/// text the facts were extracted from. Every span of a rewrite is the span of
/// the same position in the original, so a fact keyed by span joins as before.
pub fn build_project_native_measured_with_program_hashes(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    program_hashes: &HashMap<String, SourceHash>,
    typescript: &mut impl TypeFactsProvider,
    semantic_demand_options: SemanticDemandOptions,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    let project_id = project_id.into();
    let generation = Generation::new(generation).map_err(|_| BackendError::Generation)?;
    let source_files_recomputed = u64::try_from(sources.len()).unwrap_or(u64::MAX);
    let analysis_started = Instant::now();
    let mut analyzed = analyze_in_chunks(&sources, "native facts", |start, chunk| {
        let mut compiler = (dialect.compiler)();
        chunk
            .iter()
            .enumerate()
            .map(|(offset, file)| {
                let ast = solid_facts::ast::extract(&file.path, &file.source)?;
                let request = AnalysisRequest::new(
                    &file.path,
                    Arc::clone(&file.source),
                    file.compiler_options.clone(),
                );
                let execution = file_execution_map(compiler.as_mut(), &request)?;
                execution.validate(&file.source)?;
                let mut facts =
                    FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?;
                if let Some(hash) = program_hashes.get(&file.path) {
                    facts.source_hash = hash.clone();
                }
                Ok((start + offset, facts))
            })
            .collect::<Result<Vec<_>, BackendError>>()
    })?;
    analyzed.sort_by_key(|(index, _)| *index);
    let mut files = Vec::with_capacity(analyzed.len());
    let mut seeds = Vec::new();
    for (_, file) in analyzed {
        seeds.extend(file.compiler_seed_locations()?);
        seeds.extend(file.structural_seed_locations());
        files.push(file);
    }
    let source_analysis = analysis_started.elapsed();
    let type_facts_started = Instant::now();
    let demand_started = Instant::now();
    let request_started = Instant::now();
    let request_assembly = request_started.elapsed();
    let semantic_demand_started = Instant::now();
    let demands = semantic_demands(dialect, &files, semantic_demand_options)?;
    let semantic_demand_assembly = semantic_demand_started.elapsed();
    let demand_assembly = demand_started.elapsed();
    let table = typescript.semantic(demands)?;
    let exchange = typescript.take_last_exchange_timings().unwrap_or_default();
    let table_changes = typescript.take_last_table_changes();
    let type_facts = type_facts_started.elapsed();
    let hydrate_started = Instant::now();
    let table = hydrate_structural_file_facts(table, &files);
    let hydrate = hydrate_started.elapsed();
    let join_started = Instant::now();
    let mut facts =
        ProjectFacts::join(generation, project_id, files, table).map_err(BackendError::from)?;
    facts.typescript_changes = table_changes;
    let join = join_started.elapsed();
    Ok((
        facts,
        NativeBuildTimings {
            source_analysis,
            source_files_reused: 0,
            source_files_recomputed,
            // This compatibility path intentionally fuses AST and compiler
            // extraction in the same parallel workers.
            ast_facts: source_analysis,
            compiler_facts: Duration::ZERO,
            file_fact_assembly: Duration::ZERO,
            type_facts,
            demand_assembly,
            request_assembly,
            semantic_demand_assembly,
            hydrate,
            join,
            exchange,
        },
    ))
}

pub fn build_project_native_cached_measured(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    build_project_native_cached_measured_with_demands(
        dialect,
        project_id,
        generation,
        sources,
        typescript,
        cache,
        SemanticDemandOptions::NONE,
    )
}

pub fn build_project_native_cached_measured_with_demands(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
    semantic_demand_options: SemanticDemandOptions,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    build_project_native_cached_measured_inner(
        dialect,
        project_id,
        generation,
        sources,
        typescript,
        cache,
        None,
        None,
        semantic_demand_options,
        &HashMap::new(),
    )
}

struct RetainedFileFacts<'a> {
    previous: &'a ProjectFacts,
    changed_paths: &'a HashSet<String>,
}

// Private aggregation point for every native build variant; the argument
// list is the union of their options rather than a missing abstraction.
#[expect(clippy::too_many_arguments)]
fn build_project_native_cached_measured_inner(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
    retained: Option<RetainedFileFacts<'_>>,
    semantic_demand_options: SemanticDemandOptions,
    program_hashes: &HashMap<String, SourceHash>,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    let project_id = project_id.into();
    let generation = Generation::new(generation).map_err(|_| BackendError::Generation)?;
    check_cancelled(cancelled)?;
    let analysis_started = Instant::now();
    let mut files = std::iter::repeat_with(|| None)
        .take(sources.len())
        .collect::<Vec<Option<FileFacts>>>();
    let mut pending_indices = Vec::new();
    let mut pending_sources = Vec::new();
    let retained_by_path = retained.as_ref().map(|retained| {
        retained
            .previous
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<HashMap<_, _>>()
    });
    for (index, source) in sources.into_iter().enumerate() {
        let previous = retained_by_path
            .as_ref()
            .and_then(|files| files.get(source.path.as_str()).copied());
        if let Some(previous) = previous.filter(|_| {
            retained
                .as_ref()
                .is_some_and(|retained| !retained.changed_paths.contains(&source.path))
        }) {
            files[index] = Some(FileFacts {
                generation,
                path: previous.path.clone(),
                source_hash: previous.source_hash.clone(),
                source: source.source,
                ast: Arc::clone(&previous.ast),
                compiler: Arc::clone(&previous.compiler),
            });
        } else {
            pending_indices.push(index);
            pending_sources.push(source);
        }
    }
    let source_files_recomputed = u64::try_from(pending_sources.len()).unwrap_or(u64::MAX);
    let source_files_reused =
        u64::try_from(files.len().saturating_sub(pending_sources.len())).unwrap_or(u64::MAX);
    let ast_started = Instant::now();
    let prepared = prepare_ast_parallel(pending_sources, cache)?;
    let ast_facts = ast_started.elapsed();
    check_cancelled(cancelled)?;
    let compiler_started = Instant::now();
    let executions = prepare_native_compiler_parallel(dialect, &prepared, cache)?;
    let compiler_facts = compiler_started.elapsed();
    check_cancelled(cancelled)?;
    let assembly_started = Instant::now();
    for (((file, ast), execution), index) in
        prepared.into_iter().zip(executions).zip(pending_indices)
    {
        let mut facts = FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?;
        if let Some(hash) = program_hashes.get(&file.path) {
            facts.source_hash = hash.clone();
        }
        files[index] = Some(facts);
    }
    let files = files
        .into_iter()
        .map(|file| file.expect("every source was retained or recomputed"))
        .collect::<Vec<_>>();
    let file_fact_assembly = assembly_started.elapsed();
    let source_analysis = analysis_started.elapsed();
    let type_facts_started = Instant::now();
    check_cancelled(cancelled)?;
    let demand_started = Instant::now();
    let request_started = Instant::now();
    let request_assembly = request_started.elapsed();
    let semantic_demand_started = Instant::now();
    let demand_groups =
        semantic_demand_groups_cached(dialect, &files, cache, semantic_demand_options)?;
    let semantic_demand_assembly = semantic_demand_started.elapsed();
    let demand_assembly = demand_started.elapsed();
    let table = typescript.semantic_grouped(&demand_groups)?;
    let exchange = typescript.take_last_exchange_timings().unwrap_or_default();
    let table_changes = typescript.take_last_table_changes();
    check_cancelled(cancelled)?;
    let type_facts = type_facts_started.elapsed();
    let hydrate_started = Instant::now();
    cache.semantic_table = Some((generation.get(), table.clone()));
    let table = hydrate_structural_file_facts_cached(table, &files, cache);
    let hydrate = hydrate_started.elapsed();
    let join_started = Instant::now();
    let mut facts = ProjectFacts::join(generation, project_id, files, table)?;
    facts.typescript_changes = table_changes;
    let join = join_started.elapsed();
    Ok((
        facts,
        NativeBuildTimings {
            source_analysis,
            source_files_reused,
            source_files_recomputed,
            ast_facts,
            compiler_facts,
            file_fact_assembly,
            type_facts,
            demand_assembly,
            request_assembly,
            semantic_demand_assembly,
            hydrate,
            join,
            exchange,
        },
    ))
}

fn check_cancelled(cancelled: Option<&std::sync::atomic::AtomicBool>) -> Result<(), BackendError> {
    if cancelled.is_some_and(|cancelled| cancelled.load(std::sync::atomic::Ordering::Acquire)) {
        Err(BackendError::Cancelled)
    } else {
        Ok(())
    }
}

fn prepare_native_compiler_parallel(
    dialect: &'static Dialect,
    prepared: &[(SourceFile, Arc<solid_facts::ast::AstFacts>)],
    cache: &mut FactsCache,
) -> Result<Vec<Arc<ExecutionMap>>, BackendError> {
    let mut executions = vec![None; prepared.len()];
    let mut misses = Vec::new();
    for (index, (file, _)) in prepared.iter().enumerate() {
        let request = AnalysisRequest::new(
            &file.path,
            Arc::clone(&file.source),
            file.compiler_options.clone(),
        );
        let key = compiler_cache_key(dialect, &request)?;
        if let Some(execution) = cache.compiler.get(&key) {
            executions[index] = Some(execution.clone());
        } else {
            misses.push((index, key, file));
        }
    }
    let analyzed = analyze_in_chunks(&misses, "native compiler", |_, chunk| {
        let mut compiler = (dialect.compiler)();
        chunk
            .iter()
            .map(|(index, key, file)| {
                let request = AnalysisRequest::new(
                    &file.path,
                    Arc::clone(&file.source),
                    file.compiler_options.clone(),
                );
                let execution = file_execution_map(compiler.as_mut(), &request)?;
                execution.validate(&file.source)?;
                Ok((*index, key.clone(), execution))
            })
            .collect::<Result<Vec<_>, BackendError>>()
    })?;
    for (index, key, execution) in analyzed {
        let execution = Arc::new(execution);
        executions[index] = Some(Arc::clone(&execution));
        cache.compiler.insert(key, execution);
    }
    Ok(executions
        .into_iter()
        .map(|execution| execution.expect("every source was compiled or cached"))
        .collect())
}

pub fn build_project_cached(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    compiler: &mut (impl CompilerFactsProvider + ?Sized),
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
) -> Result<ProjectFacts, BackendError> {
    let project_id = project_id.into();
    let generation = Generation::new(generation).map_err(|_| BackendError::Generation)?;
    let mut files = Vec::with_capacity(sources.len());
    let mut seeds = Vec::new();
    let prepared = prepare_ast_parallel(sources, cache)?;
    for (file, ast) in prepared {
        let request =
            AnalysisRequest::new(&file.path, Arc::clone(&file.source), file.compiler_options);
        let compiler_key = compiler_cache_key(dialect, &request)?;
        let execution = if let Some(cached) = cache.compiler.get(&compiler_key) {
            cached.clone()
        } else {
            let execution = Arc::new(file_execution_map(compiler, &request)?);
            cache.compiler.insert(compiler_key, Arc::clone(&execution));
            execution
        };
        execution.validate(&file.source)?;
        let facts = FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?;
        seeds.extend(facts.compiler_seed_locations()?);
        seeds.extend(facts.structural_seed_locations());
        files.push(facts);
    }
    let table = typescript.semantic(semantic_demands_cached(
        dialect,
        &files,
        cache,
        SemanticDemandOptions::NONE,
    )?)?;
    let table = hydrate_structural_file_facts_cached(table, &files, cache);
    ProjectFacts::join(generation, project_id, files, table).map_err(Into::into)
}

fn semantic_demands(
    dialect: &'static Dialect,
    files: &[FileFacts],
    options: SemanticDemandOptions,
) -> Result<Vec<typefacts::v3::EntityDemand>, BackendError> {
    demand_plan::plan(dialect, files, options)
}

/// The dialect primitives a file imports, by the spelling a static callee names
/// them with: a named import from a module the vocabulary owns and exports the
/// primitive from, or a member of a namespace import of one.
///
/// Syntax only, so it decides *demands* and never what a callee is: a local that
/// shadows the import answers too, which costs the producer one more question
/// and proves nothing. What a call's callee actually is stays the semantic
/// lookup's.
pub(crate) struct PrimitiveImports<'a> {
    vocabulary: &'static dyn solid_dialect::Dialect,
    named: HashMap<&'a str, solid_dialect::Primitive>,
    namespaces: HashMap<&'a str, &'a str>,
}

impl<'a> PrimitiveImports<'a> {
    pub(crate) fn new(dialect: &'static Dialect, file: &'a FileFacts) -> Self {
        let vocabulary = dialect.vocabulary;
        let mut named = HashMap::new();
        let mut namespaces = HashMap::new();
        for import in &file.ast.imports {
            if !vocabulary.owns_module(&import.module) {
                continue;
            }
            for binding in &import.bindings {
                match binding.kind {
                    solid_facts::ast::ImportKind::Named => {
                        let Some(local) = file.source_text(binding.local.span) else {
                            continue;
                        };
                        let imported = binding.imported.as_deref().unwrap_or(local);
                        let Some(primitive) = vocabulary.primitive(imported) else {
                            continue;
                        };
                        if vocabulary
                            .export_modules(imported, solid_dialect::ExportPosition::Value)
                            .contains(&import.module.as_str())
                        {
                            named.insert(local, primitive);
                        }
                    }
                    solid_facts::ast::ImportKind::Namespace => {
                        if let Some(local) = file.source_text(binding.local.span) {
                            namespaces.insert(local, import.module.as_str());
                        }
                    }
                    _ => {}
                }
            }
        }
        Self {
            vocabulary,
            named,
            namespaces,
        }
    }

    /// The primitive a static callee's spelling denotes, if it names one.
    pub(crate) fn primitive(&self, static_callee: &str) -> Option<solid_dialect::Primitive> {
        if let Some(primitive) = self.named.get(static_callee) {
            return Some(*primitive);
        }
        let (namespace, property) = static_callee.split_once('.')?;
        let module = self.namespaces.get(namespace)?;
        let name = property.rsplit('.').next().unwrap_or(property);
        self.vocabulary
            .namespace_import_primitives(module)
            .contains(&name)
            .then(|| self.vocabulary.primitive(name))
            .flatten()
    }
}

fn structural_accessor_spans(dialect: &'static Dialect, file: &FileFacts) -> HashSet<Span> {
    let vocabulary = dialect.vocabulary;
    let imports = PrimitiveImports::new(dialect, file);
    let mut result = HashSet::new();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = file.ast.call_at(initializer) else {
            continue;
        };
        let Some(static_callee) = call.static_callee(&file.source) else {
            continue;
        };
        let primitive = imports.primitive(static_callee);
        if !primitive.is_some_and(|primitive| vocabulary.creates_reactive_source(primitive)) {
            continue;
        }
        let source = if binding.shape == solid_facts::ast::BindingShape::Array {
            binding.array_slots.first().and_then(Option::as_ref)
        } else {
            binding.names.first()
        };
        if let Some(source) = source {
            result.insert(source.span);
        }
    }
    result
}

fn semantic_demands_cached(
    dialect: &'static Dialect,
    files: &[FileFacts],
    cache: &mut FactsCache,
    options: SemanticDemandOptions,
) -> Result<Vec<typefacts::v3::EntityDemand>, BackendError> {
    let mut demands = Vec::new();
    let mut ordered_files = files.iter().collect::<Vec<_>>();
    ordered_files.sort_by(|left, right| left.path.cmp(&right.path));
    for file in ordered_files {
        // The plan is dialect-specific, so the dialect is part of the key.
        let key = format!(
            "{}\0{}\0{}\0{}",
            dialect.id, options.array_map_receiver_types, file.path, file.source_hash
        );
        let per_file = if let Some(cached) = cache.semantic_demands.get(&key) {
            cached
        } else {
            let generated: Arc<[typefacts::v3::EntityDemand]> =
                semantic_demands(dialect, std::slice::from_ref(file), options)?.into();
            cache.semantic_demands.insert(key.clone(), generated);
            cache
                .semantic_demands
                .get(&key)
                .expect("inserted semantic demand run")
        };
        demands.extend_from_slice(per_file);
    }
    Ok(demands)
}

fn semantic_demand_groups_cached<'a>(
    dialect: &'static Dialect,
    files: &'a [FileFacts],
    cache: &'a mut FactsCache,
    options: SemanticDemandOptions,
) -> Result<Vec<SemanticDemandGroup<'a>>, BackendError> {
    let mut ordered_files = files.iter().collect::<Vec<_>>();
    ordered_files.sort_by(|left, right| left.path.cmp(&right.path));
    let keys = ordered_files
        .iter()
        .map(|file| {
            format!(
                "{}\0{}\0{}\0{}",
                dialect.id, options.array_map_receiver_types, file.path, file.source_hash
            )
        })
        .collect::<Vec<_>>();
    for (file, key) in ordered_files.iter().zip(&keys) {
        if !cache.semantic_demands.contains_key(key) {
            cache.semantic_demands.insert(
                key.clone(),
                semantic_demands(dialect, std::slice::from_ref(*file), options)?.into(),
            );
        }
    }
    Ok(ordered_files
        .into_iter()
        .zip(keys)
        .map(|(file, key)| SemanticDemandGroup {
            path: file.path.as_str(),
            demands: cache
                .semantic_demands
                .get(&key)
                .expect("cached semantic demand run")
                .as_ref(),
            shared_demands: Some(
                cache
                    .semantic_demands
                    .get(&key)
                    .expect("cached semantic demand run"),
            ),
        })
        .collect())
}

fn typefacts_location(path: &str, span: solid_facts::core::Span) -> typefacts::Location {
    span.location(path)
}

fn callee_property_location(source: &str, callee: &typefacts::Location) -> typefacts::Location {
    let Ok(start) = usize::try_from(callee.start_byte) else {
        return callee.clone();
    };
    let Ok(end) = usize::try_from(callee.end_byte) else {
        return callee.clone();
    };
    let Some(text) = source.as_bytes().get(start..end) else {
        return callee.clone();
    };
    let Some(dot) = text.iter().rposition(|byte| *byte == b'.') else {
        return callee.clone();
    };
    typefacts::Location {
        path: callee.path.clone(),
        start_byte: u64::try_from(start + dot + 1).unwrap_or(callee.start_byte),
        end_byte: callee.end_byte,
    }
}

fn hydrate_structural_file_facts(table: TypeScriptTable, files: &[FileFacts]) -> TypeScriptTable {
    let files_by_path = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let mut table_files = table.files().cloned().collect::<Vec<_>>();
    for target in &mut table_files {
        let Some(file) = files_by_path.get(target.path.as_ref()).copied() else {
            continue;
        };
        target.functions = structural_functions(file).into();
    }
    table.with_files(table_files)
}

fn hydrate_structural_file_facts_cached(
    table: TypeScriptTable,
    files: &[FileFacts],
    cache: &mut FactsCache,
) -> TypeScriptTable {
    let files_by_path = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    let mut table_files = table.files().cloned().collect::<Vec<_>>();
    for target in &mut table_files {
        let Some(file) = files_by_path.get(target.path.as_ref()).copied() else {
            continue;
        };
        let key = format!("{}\0{}", file.path, file.source_hash);
        let functions = if let Some(cached) = cache.structural_functions.get(&key) {
            cached
        } else {
            cache
                .structural_functions
                .insert(key.clone(), structural_functions(file));
            cache
                .structural_functions
                .get(&key)
                .expect("inserted structural functions")
        };
        if target.functions.as_ref() != functions.as_slice() {
            target.functions = functions.clone().into();
        }
    }
    table.with_files(table_files)
}

fn structural_functions(file: &FileFacts) -> Vec<typefacts::SourceFunction> {
    let mut result = Vec::new();
    for function in &file.ast.functions {
        let bound_name = function.name.as_ref().or_else(|| {
            matches!(
                function.kind,
                solid_facts::ast::FunctionKind::Arrow | solid_facts::ast::FunctionKind::Expression
            )
            .then(|| {
                file.ast
                    .bindings
                    .iter()
                    .filter(|binding| {
                        binding
                            .initializer
                            .is_some_and(|initializer| initializer.contains(function.span))
                    })
                    .min_by_key(|binding| {
                        binding
                            .initializer
                            .map_or(u32::MAX, |span| span.end - span.start)
                    })
                    .and_then(|binding| binding.names.first())
            })
            .flatten()
        });
        let name = bound_name
            .map(|name| typefacts_location(file.path.as_str(), name.span))
            .or_else(|| {
                file.ast
                    .returns
                    .iter()
                    .chain(
                        file.ast
                            .functions
                            .iter()
                            .filter_map(|owner| owner.expression_return.as_ref()),
                    )
                    .any(|returned| {
                        returned
                            .argument
                            .into_iter()
                            .chain(returned.elements().iter().flatten().copied())
                            .chain(returned.properties().iter().map(|property| property.value))
                            .any(|value| file.ast.peel_ts_sugar_span(value) == function.span)
                    })
                    .then(|| typefacts_location(file.path.as_str(), function.span))
            });
        let Some(name) = name else {
            continue;
        };
        let exported = file.ast.exports.iter().any(|export| {
            export.span.contains(function.span)
                && !file.ast.functions.iter().any(|candidate| {
                    candidate.span != function.span
                        && export.span.contains(candidate.span)
                        && candidate.span.contains(function.span)
                })
        });
        result.push(typefacts::SourceFunction {
            name,
            body: typefacts::Location {
                path: file.path.to_string().into(),
                start_byte: u64::from(function.body.start),
                // tsrs reports a block body without the closing brace, while
                // expression bodies use their exact expression span.
                end_byte: u64::from(if function.expression_body {
                    function.body.end
                } else {
                    function.body.end.saturating_sub(1)
                }),
            },
            parameters: function
                .parameters
                .iter()
                .map(|parameter| typefacts_location(file.path.as_str(), parameter.pattern))
                .collect(),
            exported,
            r#async: function.r#async,
            arrow: function.kind == solid_facts::ast::FunctionKind::Arrow,
        });
    }
    result
}

fn prepare_ast_parallel(
    sources: Vec<SourceFile>,
    cache: &mut FactsCache,
) -> Result<Vec<(SourceFile, Arc<solid_facts::ast::AstFacts>)>, BackendError> {
    let mut misses = Vec::new();
    let mut prepared = vec![None; sources.len()];
    for (index, file) in sources.iter().enumerate() {
        let key = ast_cache_key(file);
        if let Some(ast) = cache.ast.get(&key) {
            prepared[index] = Some(ast.clone());
        } else {
            misses.push((index, key, file.path.as_str(), file.source.as_ref()));
        }
    }
    let parsed = analyze_in_chunks(&misses, "Oxc", |_, chunk| {
        chunk
            .iter()
            .map(|(index, key, path, source)| {
                Ok((
                    *index,
                    key.clone(),
                    solid_facts::ast::extract(*path, source)?,
                ))
            })
            .collect::<Result<Vec<_>, solid_facts::ast::AstFactsError>>()
    })?;
    for (index, key, ast) in parsed {
        let ast = Arc::new(ast);
        prepared[index] = Some(Arc::clone(&ast));
        cache.ast.insert(key, ast);
    }
    Ok(sources
        .into_iter()
        .zip(prepared)
        .map(|(file, ast)| (file, ast.expect("every source was parsed or cached")))
        .collect())
}

fn ast_cache_key(file: &SourceFile) -> String {
    format!(
        "{}\0{}",
        file.path,
        solid_facts::core::SourceHash::of(&file.source)
    )
}

fn compiler_cache_key(
    dialect: &'static Dialect,
    request: &AnalysisRequest,
) -> Result<String, BackendError> {
    // The dialect id keys the cache alongside the request: two dialects'
    // compilers can disagree about the same source, so their execution maps
    // must never answer for each other. It sits after the path because
    // `FactsCache::invalidate_path` matches on the `path\0` prefix.
    Ok(format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        request.path,
        request.source_hash,
        dialect.id,
        dialect.compiler_facts_identity,
        request.compiler_facts_protocol,
        serde_json::to_string(&request.compiler_options)?
    ))
}

#[cfg(test)]
mod tests;
