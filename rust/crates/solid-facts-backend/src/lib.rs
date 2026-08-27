//! Rust-led orchestration of Oxc AST facts, Solid execution facts, and
//! TypeScript-Go semantic facts.

#[cfg(not(any(feature = "dialect-v1", feature = "dialect-v2")))]
compile_error!("solid-facts-backend requires at least one dialect feature");

mod cache;
mod contract_document;
mod contract_document_v2;
mod contract_interface;
mod demand_plan;
mod diagnostics;
pub mod dialect;
mod wire;

pub use cache::{CacheStats, FactsCache};
pub use contract_document::encode as encode_package_contract;
pub use contract_interface::{
    ArtifactResolutionFailure, ArtifactResolver, BundledEvidenceStore, ContractFailure,
    EvidenceKey, EvidenceStore, EvidenceStoreFailure, HostResolutionAdapter, ImportRequest,
    LocalEvidenceStore, ResolutionAuthority, ResolutionTraceStep, ResolvedFile, ResolvedImport,
    StandaloneResolutionAdapter, load_accepted_contract,
};
#[cfg(feature = "dialect-v2")]
pub use diagnostics::bundled_solid_js_contract;
pub use diagnostics::{
    DiagnosticAnalysis, DiagnosticSession, DiagnosticTimings, Metrics, PackageContractStatus,
    PackageSummary, RequestedRuleEnablement, Snapshot, SnapshotEvidence, SnapshotFinding,
    SnapshotFix, SnapshotTextEdit, SourceLocation, analysis_metrics, analyze_project,
    analyze_project_measured, analyze_project_measured_with,
    analyze_project_measured_with_enablement, discovered_contract_paths,
    discovered_rule_options_path, imported_package_roots, load_package_contracts,
    load_package_contracts_with, package_contract_statuses, package_contract_statuses_with,
    read_package_contract, semantic_demand_options_for_enablement, snapshot, source_location,
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
    /// Package generation only: retain exact parameter value domains for the
    /// hash-bound runtime probe construction sidecar.
    pub contract_probe_parameters: bool,
}

impl SemanticDemandOptions {
    pub const NONE: Self = Self {
        array_map_receiver_types: false,
        contract_probe_parameters: false,
    };

    pub const PREFERENCES: Self = Self {
        array_map_receiver_types: true,
        contract_probe_parameters: false,
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
    #[error("package contract error: {0}")]
    Contract(String),
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
            CompilerProviderError::Native(message) => Self::NativeCompiler(message),
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

    /// Compiler-selected call facts for exact call-expression demands.
    ///
    /// Like [`Self::module_graph`], this is a read of the live TypeScript
    /// generation and does not alter the retained analysis demand set. Package
    /// proof callers opt into the bounded callable tree and implementation
    /// census; ordinary project analysis pays neither serialization nor
    /// retention cost.
    pub fn invocations(
        &mut self,
        demands: &[typefacts::InvocationDemand],
    ) -> Result<typefacts::InvocationAnswer, BackendError> {
        Ok(self.session.invocations(demands)?)
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
/// in-process. TypeScript-Go is the only process boundary.
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
}

/// The importing files whose module specifiers must be attested before a
/// package contract may be bound to any of them.
///
/// A contract is applied by installed identity, which needs the compiler's own
/// resolution for the specifier
/// ([`solid_reactive_ir::PackageContract::for_import`]). Asking for that
/// resolution is an explicit operation on the Type Facts session, and its
/// import half is proportional to the files asked about — so it is asked only
/// of the files that could carry a contract-bound specifier at all: the ones
/// with at least one bare specifier. A relative or `node:` specifier can never
/// name a package.
///
/// The scope deliberately does **not** consult contract discovery, though that
/// would narrow it further. The attestation is computed once per program
/// generation and a retained session reuses it across checks, while contracts
/// are re-discovered on every check; a scope keyed on today's contracts would
/// answer for a contract that appeared afterwards by *silently omitting* its
/// files, which is name-only binding restored by accident. `export … from`
/// specifiers count for the same reason contract resolution binds them.
///
/// An empty answer means no specifier in this program could name a package, and
/// the caller then asks for nothing.
#[must_use]
pub fn contract_identity_scope(facts: &ProjectFacts) -> Vec<String> {
    facts
        .files
        .iter()
        .filter(|file| {
            file.ast
                .imports
                .iter()
                .map(|import| import.module.as_str())
                .chain(
                    file.ast
                        .exports
                        .iter()
                        .filter_map(|export| export.module.as_deref()),
                )
                .any(|specifier| {
                    !specifier.starts_with('.')
                        && !specifier.starts_with('/')
                        && !specifier.starts_with("node:")
                })
        })
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
/// is the fact a contract is bound by.
///
/// A requested file the program does not hold is reported by the producer
/// rather than dropped, and it is recorded here as *not covered*: a contract
/// then refuses every specifier in it. Presenting an unanswered file as "this
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
            displaced.push((change.path.clone(), self.sources.get(&change.path).cloned()));
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
        );
        result.map(|(facts, timings)| {
            self.last_build_timings = timings;
            facts
        })
    }

    fn restore_overlay(&mut self, displaced: Vec<(String, Option<SourceFile>)>) {
        for (path, previous) in displaced {
            self.cache.invalidate_path(&path);
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
        let (facts, timings) = build_project_native_cached_measured_with_demands(
            self.dialect,
            self.project_id.clone(),
            self.generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            self.semantic_demand_options,
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
        )?;
        self.last_build_timings = timings;
        let facts = Arc::new(self.attested(facts)?);
        self.last_facts = Some(Arc::clone(&facts));
        Ok(facts)
    }

    /// Attaches this program's attested import identities to the facts it
    /// produced, so a package contract reaching them is bound to the install
    /// each specifier resolves to rather than to its name.
    ///
    /// Every path that produces `ProjectFacts` from this session passes through
    /// here. A retained session reuses one generation's facts across many
    /// checks, so the attestation is computed once per generation with it — and
    /// it must not be conditional on anything that can change between checks
    /// (see [`contract_identity_scope`]).
    fn attested(&mut self, mut facts: ProjectFacts) -> Result<ProjectFacts, BackendError> {
        let scope = contract_identity_scope(&facts);
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
                let execution = if solid_facts::ast::is_json_module_path(&file.path) {
                    inert_execution_map(request.source_hash.clone())
                } else {
                    compiler.analyze(&request)?
                };
                execution.validate(&file.source)?;
                Ok((
                    start + offset,
                    FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?,
                ))
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

pub fn build_project_native_cached(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_cached_measured(
        dialect, project_id, generation, sources, typescript, cache,
    )
    .map(|(facts, _)| facts)
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
    )
}

pub fn build_project_native_cached_cancellable(
    dialect: &'static Dialect,
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_cached_measured_inner(
        dialect,
        project_id,
        generation,
        sources,
        typescript,
        cache,
        Some(cancelled),
        None,
        SemanticDemandOptions::NONE,
    )
    .map(|(facts, _)| facts)
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
        let facts = FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?;
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
                let execution = if solid_facts::ast::is_json_module_path(&file.path) {
                    inert_execution_map(request.source_hash.clone())
                } else {
                    compiler.analyze(&request)?
                };
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
            let execution = Arc::new(if solid_facts::ast::is_json_module_path(&file.path) {
                inert_execution_map(request.source_hash.clone())
            } else {
                compiler.analyze(&request)?
            });
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

fn structural_accessor_spans(dialect: &'static Dialect, file: &FileFacts) -> HashSet<Span> {
    let vocabulary = dialect.vocabulary;
    let mut named_imports = HashMap::<&str, solid_dialect::Primitive>::new();
    let mut namespace_imports = HashMap::<&str, &str>::new();
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
                        named_imports.insert(local, primitive);
                    }
                }
                solid_facts::ast::ImportKind::Namespace => {
                    if let Some(local) = file.source_text(binding.local.span) {
                        namespace_imports.insert(local, &import.module);
                    }
                }
                _ => {}
            }
        }
    }
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
        let primitive = if let Some(primitive) = named_imports.get(static_callee) {
            Some(*primitive)
        } else if let Some((namespace, property)) = static_callee.split_once('.')
            && let Some(module) = namespace_imports.get(namespace)
        {
            let name = property.rsplit('.').next().unwrap_or(property);
            vocabulary
                .namespace_import_primitives(module)
                .contains(&name)
                .then(|| vocabulary.primitive(name))
                .flatten()
        } else {
            None
        };
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
                    .any(|returned| {
                        returned.value == solid_facts::ast::ReturnValueKind::Function
                            && returned.span == function.span
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
                // TS-Go reports a block body without the closing brace, while
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
mod tests {
    use super::*;
    use solid_facts::compiler::COMPILER_FACTS_PROTOCOL;
    use solid_facts::core::SourceHash;
    use typefacts::SourceDigest;

    /// The wire schema the in-memory stub claims; the real value comes from
    /// the producer's handshake, which no stub performs.
    const STUB_SCHEMA: u64 = 2;

    struct Compiler;
    impl CompilerFactsProvider for Compiler {
        fn analyze(
            &mut self,
            request: &AnalysisRequest,
        ) -> Result<ExecutionMap, CompilerProviderError> {
            Ok(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: request.source_hash.clone(),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            })
        }
    }

    struct CountingCompiler(usize);
    impl CompilerFactsProvider for CountingCompiler {
        fn analyze(
            &mut self,
            request: &AnalysisRequest,
        ) -> Result<ExecutionMap, CompilerProviderError> {
            self.0 += 1;
            Compiler.analyze(request)
        }
    }

    /// Stands in for a retained session: it answers with a table stamped for
    /// the generation it is on, and advances the way a real session does when
    /// the caller moves to the next one.
    struct Types {
        source: SourceDigest,
        project_id: String,
        generation: u64,
    }
    impl TypeFactsProvider for Types {
        fn semantic_grouped(
            &mut self,
            _groups: &[SemanticDemandGroup<'_>],
        ) -> Result<TypeScriptTable, BackendError> {
            let generation = self.generation;
            self.generation += 1;
            Ok(TypeScriptTable::from_parts(
                STUB_SCHEMA,
                generation,
                self.project_id.clone(),
                vec![self.source.clone()],
                vec![],
                vec![],
                vec![],
            ))
        }
    }

    fn test_file_facts(path: &str, source: &str) -> FileFacts {
        let ast = solid_facts::ast::extract(path, source).unwrap();
        FileFacts::new(
            Generation::new(1).unwrap(),
            source,
            ast,
            ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            },
        )
        .unwrap()
    }

    #[test]
    fn retained_demands_and_indexed_hydration_match_fresh_results() {
        let files = vec![
            test_file_facts(
                "src/b.tsx",
                "export const B = () => <div>{createSignal(1)}</div>;",
            ),
            test_file_facts(
                "src/a.ts",
                "export function A(value: number) { return value; }",
            ),
        ];
        let dialect = dialect::default_dialect();
        let fresh_demands = semantic_demands(dialect, &files, SemanticDemandOptions::NONE).unwrap();
        let mut cache = FactsCache::default();
        let retained_demands =
            semantic_demands_cached(dialect, &files, &mut cache, SemanticDemandOptions::NONE)
                .unwrap();
        assert_eq!(retained_demands, fresh_demands);

        let fresh_table = TypeScriptTable::from_parts(
            STUB_SCHEMA,
            1,
            "project",
            vec![],
            vec![],
            vec![],
            files
                .iter()
                .rev()
                .map(|file| typefacts::FileFact {
                    path: file.path.to_string().into(),
                    calls: vec![].into(),
                    bindings: vec![].into(),
                    functions: vec![].into(),
                    async_functions: vec![].into(),
                })
                .collect(),
        );
        let retained_table = fresh_table.clone();
        let fresh_table = hydrate_structural_file_facts(fresh_table, &files);
        let retained_table =
            hydrate_structural_file_facts_cached(retained_table, &files, &mut cache);
        assert_eq!(
            retained_table.files().collect::<Vec<_>>(),
            fresh_table.files().collect::<Vec<_>>()
        );
    }

    #[test]
    fn semantic_demand_plan_is_complete_for_downstream_consumers() {
        let file = test_file_facts(
            "src/component.tsx",
            // Every export shape the `kind` decision can be asked about, not
            // just the declaration form: a specifier list, a class
            // declaration, an anonymous default class (whose recorded span is
            // the class node, there being no name), and a destructuring
            // declarator. Each must carry both signature facts, because
            // `export_kind_proof` reads absence at one of these spans as a
            // refusal.
            "const value = createMemo(async () => 1); const effectArgs: [() => number] = [() => 1]; createEffect(...effectArgs); const local = 1; const o: { a: number } = { a: 1 }; export { local }; export class K {} export const { a } = o; export default class {} export function Card(props: { title: string }) { const key = 'title'; const copy = { ...props }; return <div>{props[key]}{copy.title}{value()}</div>; }",
        );
        let demands = semantic_demands(
            dialect::default_dialect(),
            std::slice::from_ref(&file),
            SemanticDemandOptions::NONE,
        )
        .unwrap();

        for member in &file.ast.members {
            let location = typefacts_location(file.path.as_str(), member.object);
            assert!(
                demands
                    .iter()
                    .any(|demand| demand.symbol && demand.location == location),
                "member object {location:?} must retain symbol provenance"
            );
        }
        for spread in &file.ast.spreads {
            let location = typefacts_location(file.path.as_str(), spread.argument);
            assert!(
                demands
                    .iter()
                    .any(|demand| demand.symbol && demand.location == location),
                "spread argument {location:?} must retain symbol provenance"
            );
        }
        for call in &file.ast.calls {
            let location = typefacts_location(file.path.as_str(), call.callee);
            let demand = demands
                .iter()
                .find(|demand| demand.location == location && demand.query_location.is_some())
                .expect("every call callee needs a symbol/type query");
            assert!(demand.symbol);
            assert_eq!(demand.type_descriptor, call.arguments.is_empty());
            let property = callee_property_location(&file.source, &location);
            if property != location {
                assert!(demands.iter().any(|demand| {
                    demand.location == property && demand.symbol && demand.query_location.is_none()
                }));
            }
            for argument in call.arguments.iter().filter(|argument| {
                argument.value == solid_facts::ast::ArgumentValueKind::Identifier
            }) {
                let argument_location = typefacts_location(file.path.as_str(), argument.span);
                assert!(
                    demands.iter().any(|demand| {
                        demand.location == argument_location
                            && demand.symbol
                            && demand.type_descriptor
                            && demand.callability
                    }),
                    "identifier call arguments need compiler callability for runtime semantics"
                );
            }
            for argument in call.arguments.iter().filter(|argument| !argument.spread) {
                let argument_location = typefacts_location(file.path.as_str(), argument.span);
                assert!(demands.iter().any(|demand| {
                    demand.location == argument_location && demand.primitive_value_domain
                }));
            }
            for argument in call.arguments.iter().filter(|argument| argument.spread) {
                let argument_location = typefacts_location(
                    file.path.as_str(),
                    file.ast
                        .spreads
                        .iter()
                        .find(|spread| spread.span == argument.span)
                        .map_or(argument.span, |spread| spread.argument),
                );
                assert!(
                    demands.iter().any(|demand| demand.symbol
                        && demand.tuple_shape
                        && demand.location == argument_location),
                    "call spread {argument_location:?} must retain symbol provenance and exact tuple shape"
                );
            }
        }
        for import in &file.ast.imports {
            for binding in &import.bindings {
                let location = typefacts_location(file.path.as_str(), binding.local.span);
                assert!(demands.iter().any(|demand| {
                    demand.location == location && demand.reference_space && demand.runtime_identity
                }));
            }
        }
        for export in &file.ast.exports {
            for item in export.specifiers.iter().chain(&export.declarations) {
                let location = typefacts_location(file.path.as_str(), item.local.span);
                // Both signature facts, at every export specifier and every
                // exported declaration name. `export_kind_proof` treats an
                // absent fact at one of these spans as a *refusal* rather than
                // as a keep-the-summary no-op, which is only honest because
                // this invariant holds: absence there is the producer finding
                // no node to classify, never the plan declining to ask.
                assert!(demands.iter().any(|demand| {
                    demand.location == location
                        && demand.callability
                        && demand.constructability
                        && demand.runtime_identity
                }));
            }
        }
        assert!(demands.iter().any(|demand| {
            demand.r#async && demand.location.path.as_ref() == file.path.as_str()
        }));
        assert!(
            demands.windows(2).all(|pair| pair[0] != pair[1]),
            "the transport plan must not contain duplicate queries"
        );
        assert!(
            demands
                .windows(2)
                .all(|pair| pair[0].location != pair[1].location),
            "each syntax location must combine all requested compiler facts"
        );
        let mut reversed = vec![
            file.clone(),
            test_file_facts("src/a.ts", "export const a = 1;"),
        ];
        let planned = semantic_demands(
            dialect::default_dialect(),
            &reversed,
            SemanticDemandOptions::NONE,
        )
        .unwrap();
        reversed.reverse();
        assert_eq!(
            planned,
            semantic_demands(
                dialect::default_dialect(),
                &reversed,
                SemanticDemandOptions::NONE,
            )
            .unwrap(),
            "query order must not depend on source traversal order"
        );
    }

    #[test]
    fn array_shape_demands_are_limited_to_compiler_proven_jsx_maps() {
        let mut file = test_file_facts(
            "src/lists.tsx",
            "const rendered = <ul>{items().map(item => item)}</ul>; const computed = items().map(item => item);",
        );
        let map_calls = file
            .ast
            .calls
            .iter()
            .filter(|call| {
                file.ast.members.iter().any(|member| {
                    member.span == call.callee && file.source_text(member.property) == Some("map")
                })
            })
            .map(|call| call.span)
            .collect::<Vec<_>>();
        assert_eq!(map_calls.len(), 2);
        Arc::make_mut(&mut file.compiler).jsx_operations.push(
            solid_facts::compiler::JsxOperation {
                span: map_calls[0],
                kind: "jsx-expression".into(),
            },
        );

        let default_demands = semantic_demands(
            dialect::default_dialect(),
            &[file.clone()],
            SemanticDemandOptions::NONE,
        )
        .unwrap();
        assert!(
            default_demands.iter().all(|demand| !demand.array_shape),
            "the low-level NONE option must not request map receiver shapes"
        );

        let demands = semantic_demands(
            dialect::default_dialect(),
            &[file.clone()],
            SemanticDemandOptions::PREFERENCES,
        )
        .unwrap();
        for (index, call_span) in map_calls.into_iter().enumerate() {
            let member = file
                .ast
                .members
                .iter()
                .find(|member| {
                    member.span
                        == file
                            .ast
                            .calls
                            .iter()
                            .find(|call| call.span == call_span)
                            .unwrap()
                            .callee
                })
                .unwrap();
            let location = typefacts_location(file.path.as_str(), member.object);
            let array_shape = demands
                .iter()
                .find(|demand| demand.location == location)
                .is_some_and(|demand| demand.array_shape);
            assert_eq!(
                array_shape,
                index == 0,
                "only the compiler-proven JSX child map should request an array-shape query"
            );
        }
    }

    #[test]
    fn structural_accessors_follow_the_selected_vocabulary_and_export_modules() {
        let file = test_file_facts(
            "src/sources.ts",
            r#"
                import { createResource, createProjection } from "solid-js";
                import { createStore } from "solid-js/store";
                const [resource] = createResource(fetcher);
                const projection = createProjection(() => state);
                const [store] = createStore({ count: 0 });
            "#,
        );
        let names = |selected| {
            structural_accessor_spans(selected, &file)
                .into_iter()
                .filter_map(|span| file.source_text(span).map(str::to_owned))
                .collect::<HashSet<_>>()
        };

        assert_eq!(
            names(
                dialect::by_version(solid_dialect::Version::V1)
                    .expect("default build includes solid-v1"),
            ),
            HashSet::from(["resource".to_owned(), "store".to_owned()])
        );
        assert_eq!(
            names(
                dialect::by_version(solid_dialect::Version::V2)
                    .expect("default build includes solid-v2"),
            ),
            HashSet::from(["projection".to_owned()])
        );
    }

    #[test]
    fn returned_member_calls_combine_symbol_and_resolved_call_demands() {
        let file = test_file_facts(
            "src/cleanup.ts",
            "export function subscribe(source: { on(): () => void }) { return source.on(); }",
        );
        let returned = file
            .ast
            .returns
            .iter()
            .find_map(|returned| returned.callee)
            .expect("returned call");
        let location = typefacts_location(file.path.as_str(), returned);
        let demands = semantic_demands(
            dialect::default_dialect(),
            std::slice::from_ref(&file),
            SemanticDemandOptions::NONE,
        )
        .unwrap();
        let matching = demands
            .iter()
            .filter(|demand| demand.location == location)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1);
        assert!(matching[0].symbol);
        assert!(matching[0].resolved_call);
        assert!(matching[0].query_location.is_some());
    }

    #[test]
    fn joins_all_three_fact_sources() {
        let source = "export const answer = 42;";
        let mut compiler = Compiler;
        let mut types = Types {
            project_id: "project".into(),
            generation: 1,
            source: SourceDigest {
                path: "src/a.ts".into(),
                sha256: typefacts::SourceHash::of(source),
            },
        };
        let project = build_project(
            dialect::default_dialect(),
            "project",
            1,
            vec![SourceFile {
                path: "src/a.ts".into(),
                source: source.into(),
                compiler_options: CompilerOptions::default(),
            }],
            &mut compiler,
            &mut types,
        )
        .unwrap();
        assert_eq!(project.files.len(), 1);
        assert_eq!(
            project.typescript.sources().collect::<Vec<_>>(),
            vec![&types.source]
        );
    }

    /// How many analyses the stub dialect's compiler has performed; proves
    /// the native pipeline ran the selected dialect's compiler rather than
    /// the default one.
    static STUB_COMPILER_RUNS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    struct RecordingStubCompiler;
    impl CompilerFactsProvider for RecordingStubCompiler {
        fn analyze(
            &mut self,
            request: &AnalysisRequest,
        ) -> Result<ExecutionMap, CompilerProviderError> {
            STUB_COMPILER_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Compiler.analyze(request)
        }
    }

    const STUB_RULE: solid_reactive_ir::RuleMetadata = solid_reactive_ir::RuleMetadata {
        code: "ST0001",
        name: "stub-dialect-ran",
        severity: "warning",
        uncertifiable: false,
        default_enabled: true,
        presets: &[],
    };

    const STUB_CONTRACT_RULE: solid_reactive_ir::RuleMetadata = solid_reactive_ir::RuleMetadata {
        code: "ST9001",
        name: "stub-contract-missing",
        severity: "error",
        uncertifiable: true,
        default_enabled: true,
        presets: &[],
    };

    fn stub_solve(
        _program: &solid_reactive_ir::Program,
    ) -> (
        Vec<solid_reactive_ir::Finding>,
        solid_reactive_ir::SolveTimings,
    ) {
        (
            vec![solid_reactive_ir::Finding::new(
                STUB_RULE,
                "the stub dialect's catalog examined this program".into(),
                typefacts::Location {
                    path: "src/a.ts".into(),
                    start_byte: 0,
                    end_byte: 0,
                },
            )],
            solid_reactive_ir::SolveTimings::default(),
        )
    }

    fn stub_package_contract_finding(
        issue: &solid_reactive_ir::PackageContractIssue,
    ) -> solid_reactive_ir::Finding {
        solid_reactive_ir::Finding::new(
            STUB_CONTRACT_RULE,
            "the stub dialect's package contract is unavailable".into(),
            issue.location.clone(),
        )
    }

    static STUB_DIALECT: dialect::Dialect = dialect::Dialect {
        id: "stub-dialect",
        compiler_facts_identity: "stub-compiler:trace0:test",
        vocabulary: &solid_dialect::Solid2,
        rule_count: 1,
        compiler: || Box::new(RecordingStubCompiler),
        solve_measured: stub_solve,
        docs_url: |rule| format!("stub://docs/{rule}"),
        has_rule: |_| false,
        rule_metadata: |rule| (rule == STUB_RULE.name).then_some(STUB_RULE),
        semantic_demands: dialect::SemanticDemandCapabilities::NONE,
        catalog_capabilities: solid_reactive_ir::CatalogCapabilities::SOLID_2,
        package_contract_finding: stub_package_contract_finding,
        bundled_packages: &[],
        bundled_contract: |_| Ok(None),
    };

    /// An alternate dialect flows end to end through the native pipeline:
    /// its compiler produces the execution maps, its catalog produces the
    /// findings, and its cache keys never collide with another dialect's.
    #[test]
    fn alternate_dialect_flows_through_native_pipeline() {
        let source = "export const answer = 42;";
        let mut types = Types {
            project_id: "project/tsconfig.json".into(),
            generation: 1,
            source: SourceDigest {
                path: "src/a.ts".into(),
                sha256: typefacts::SourceHash::of(source),
            },
        };
        let sources = vec![SourceFile {
            path: "src/a.ts".into(),
            source: source.into(),
            compiler_options: CompilerOptions::default(),
        }];
        let facts = build_project_native(
            &STUB_DIALECT,
            "project/tsconfig.json",
            1,
            sources.clone(),
            &mut types,
        )
        .unwrap();
        assert!(
            STUB_COMPILER_RUNS.load(std::sync::atomic::Ordering::SeqCst) > 0,
            "the stub dialect's compiler must produce the execution maps"
        );
        let analysis = analyze_project(
            &STUB_DIALECT,
            std::path::Path::new("project/tsconfig.json"),
            &sources,
            &facts,
            &[],
        )
        .unwrap();
        let identities = analysis
            .snapshot
            .findings
            .iter()
            .map(|finding| (finding.id.as_str(), finding.rule.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            identities,
            vec![("ST0001", "stub-dialect-ran")],
            "the stub dialect's catalog must supply the findings"
        );

        let request = AnalysisRequest::new(
            "src/a.ts",
            Arc::<str>::from(source),
            CompilerOptions::default(),
        );
        let stub_key = compiler_cache_key(&STUB_DIALECT, &request).unwrap();
        assert!(stub_key.contains(STUB_DIALECT.compiler_facts_identity));
        assert!(stub_key.contains(&format!("\0{}\0", COMPILER_FACTS_PROTOCOL)));
        assert_ne!(
            stub_key,
            compiler_cache_key(dialect::default_dialect(), &request).unwrap(),
            "an execution map cached by one dialect must never answer for another"
        );
    }

    /// Regression for the real defect in `@solidjs/start@2.0.3`'s
    /// `dist/shared/dev-toolbar/index.jsx`, which imports its own
    /// `package.json`: the TypeScript program legitimately hands a `.json`
    /// module to the native build alongside ordinary JS/TS sources, and the
    /// build must certify it inert -- not crash, and not invoke a JS/JSX
    /// compiler that has no grammar for JSON -- rather than merely tolerate
    /// it.
    /// Stands in for a retained session the same way [`Types`] does, but
    /// over an arbitrary source set: this regression needs digests for both
    /// the JS entry and the JSON module it imports, and [`Types`] only ever
    /// carries one.
    struct MultiSourceTypes {
        project_id: String,
        generation: u64,
        sources: Vec<SourceDigest>,
    }
    impl TypeFactsProvider for MultiSourceTypes {
        fn semantic_grouped(
            &mut self,
            _groups: &[SemanticDemandGroup<'_>],
        ) -> Result<TypeScriptTable, BackendError> {
            let generation = self.generation;
            self.generation += 1;
            Ok(TypeScriptTable::from_parts(
                STUB_SCHEMA,
                generation,
                self.project_id.clone(),
                self.sources.clone(),
                vec![],
                vec![],
                vec![],
            ))
        }
    }

    #[test]
    fn a_json_import_source_is_certified_inert_without_invoking_the_compiler() {
        let index_source =
            "import pkg from \"./package.json\";\nexport const version = pkg.version;\n";
        let json_source = r#"{"name": "demo", "version": "1.0.0"}"#;
        let mut types = MultiSourceTypes {
            project_id: "project/tsconfig.json".into(),
            generation: 1,
            sources: vec![
                SourceDigest {
                    path: "index.mjs".into(),
                    sha256: typefacts::SourceHash::of(index_source),
                },
                SourceDigest {
                    path: "package.json".into(),
                    sha256: typefacts::SourceHash::of(json_source),
                },
            ],
        };
        let sources = vec![
            SourceFile {
                path: "index.mjs".into(),
                source: index_source.into(),
                compiler_options: CompilerOptions::default(),
            },
            SourceFile {
                path: "package.json".into(),
                source: json_source.into(),
                compiler_options: CompilerOptions::default(),
            },
        ];
        // `build_project` takes its `CompilerFactsProvider` directly rather
        // than the dialect constructing one internally, so a local counter
        // proves how many files it ran on without racing another test's
        // shared dialect-compiler counter.
        let mut compiler = CountingCompiler(0);
        let facts = build_project(
            dialect::default_dialect(),
            "project/tsconfig.json",
            1,
            sources,
            &mut compiler,
            &mut types,
        )
        .unwrap();

        let json_file = facts
            .files
            .iter()
            .find(|file| file.path.as_str() == "package.json")
            .expect("the JSON module must still be enrolled as an analyzed file");
        assert!(json_file.ast.calls.is_empty());
        assert!(json_file.ast.bindings.is_empty());
        assert!(json_file.ast.functions.is_empty());
        assert!(json_file.ast.imports.is_empty());
        assert!(json_file.ast.exports.is_empty());
        assert!(json_file.ast.members.is_empty());
        assert!(json_file.compiler.tracked_regions.is_empty());
        assert!(json_file.compiler.untracked_regions.is_empty());
        // Every execution-fact array, enumerated: the claim is that a JSON
        // module has *no* execution facts, so a new array left out of this list
        // would let the next one arrive populated with this test still green.
        assert!(json_file.compiler.discarded_regions.is_empty());
        assert!(json_file.compiler.ownership_regions.is_empty());
        assert!(json_file.compiler.callback_roles.is_empty());
        assert!(json_file.compiler.jsx_operations.is_empty());

        // Only `index.mjs` needed the dialect's compiler; the JSON module's
        // inertness is a proof from its module kind, not a question the JS/JSX
        // compiler was asked and happened to answer favorably.
        assert_eq!(
            compiler.0, 1,
            "the dialect compiler must run for index.mjs only, never for the JSON module"
        );
    }

    /// The JSON fix is an extension/module-kind rule, not a blanket "skip
    /// anything unparseable": a source reached through a genuinely
    /// unsupported extension must still fail the native build closed, exactly
    /// as it did before the JSON module kind got its own path.
    #[test]
    fn a_non_json_unsupported_extension_still_fails_the_native_build() {
        let source = "binary";
        let mut types = Types {
            project_id: "project/tsconfig.json".into(),
            generation: 1,
            source: SourceDigest {
                path: "asset.wasm".into(),
                sha256: typefacts::SourceHash::of(source),
            },
        };
        let sources = vec![SourceFile {
            path: "asset.wasm".into(),
            source: source.into(),
            compiler_options: CompilerOptions::default(),
        }];
        let error = build_project_native(
            &STUB_DIALECT,
            "project/tsconfig.json",
            1,
            sources,
            &mut types,
        )
        .expect_err("a non-JSON unsupported extension must still fail closed, not be skipped");
        assert!(
            matches!(
                error,
                BackendError::Ast(solid_facts::ast::AstFactsError::SourceType { .. })
            ),
            "unexpected error variant: {error:?}"
        );
    }

    #[test]
    fn reuses_ast_and_compiler_facts_by_source_identity() {
        let source = "export const answer = 42;";
        let input = SourceFile {
            path: "src/a.ts".into(),
            source: source.into(),
            compiler_options: CompilerOptions::default(),
        };
        let mut compiler = CountingCompiler(0);
        let mut types = Types {
            project_id: "project".into(),
            generation: 1,
            source: SourceDigest {
                path: "src/a.ts".into(),
                sha256: typefacts::SourceHash::of(source),
            },
        };
        let mut cache = FactsCache::default();
        let projects = [1, 2].map(|generation| {
            build_project_cached(
                dialect::default_dialect(),
                "project",
                generation,
                vec![input.clone()],
                &mut compiler,
                &mut types,
                &mut cache,
            )
            .unwrap()
        });
        assert_eq!(compiler.0, 1);
        assert!(Arc::ptr_eq(
            &projects[0].files[0].ast,
            &projects[1].files[0].ast
        ));
        assert!(Arc::ptr_eq(
            &projects[0].files[0].compiler,
            &projects[1].files[0].compiler
        ));
        assert_eq!(
            cache.stats(),
            CacheStats {
                ast_entries: 1,
                compiler_entries: 1
            }
        );
    }
}
