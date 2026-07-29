//! Rust-led orchestration of Oxc AST facts, Solid execution facts, and
//! TypeScript-Go semantic facts.

mod cache;
mod contract_document;
mod demand_plan;
mod diagnostics;

pub use cache::{CacheStats, FactsCache};
pub use contract_document::encode as encode_package_contract;
pub use diagnostics::{
    DiagnosticAnalysis, DiagnosticSession, DiagnosticTimings, Metrics, PackageContractStatus,
    PackageSummary, Snapshot, SnapshotEvidence, SnapshotFinding, SnapshotFix, SnapshotTextEdit,
    SourceLocation, analysis_metrics, analyze_project, analyze_project_measured,
    analyze_project_measured_with, bundled_solid_js_contract, discovered_contract_paths,
    imported_package_roots, load_package_contracts, load_package_contracts_with,
    package_contract_statuses, package_contract_statuses_with, read_package_contract, snapshot,
    source_location,
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

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use solid_compiler_facts::{AnalysisRequest, CompilerOptions, ExecutionMap};
use solid_facts::{FileFacts, ProjectFacts, TypeScriptChanges, TypeScriptTable};
use solid_facts_core::{Generation, Span};
use thiserror::Error;
use typefacts::v3::EntityDemand;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub source: Arc<str>,
    #[serde(default)]
    pub compiler_options: CompilerOptions,
}

pub trait CompilerFactsProvider {
    fn analyze(&mut self, request: &AnalysisRequest) -> Result<ExecutionMap, BackendError>;
}

pub struct SemanticDemandGroup<'a> {
    pub path: &'a str,
    pub demands: &'a [EntityDemand],
    pub shared_demands: Option<&'a Arc<[EntityDemand]>>,
}

/// The checker's view of a TypeFacts producer.
///
/// The retained session — process lifecycle, framing, handshake, request
/// correlation, retained demands and delta application — belongs to the
/// `typefacts` crate. What is left here is the one question the analysis
/// pipeline asks: given this generation's demands, what is the fact table?
pub trait TypeFactsProvider {
    /// Analyses the current generation from demands already grouped by path.
    ///
    /// A group equal to the retained state is neither cloned nor transmitted,
    /// so an unchanged generation costs a lookup rather than a round trip.
    fn semantic_grouped(
        &mut self,
        groups: &[SemanticDemandGroup<'_>],
    ) -> Result<TypeScriptTable, BackendError>;

    /// Analyses from a flat demand list, grouping it first. A convenience for
    /// callers that do not already keep demands grouped.
    fn semantic(&mut self, demands: Vec<EntityDemand>) -> Result<TypeScriptTable, BackendError> {
        let grouped = group_demands(&demands);
        let groups = grouped
            .iter()
            .filter(|run| !run.is_empty())
            .map(|run| SemanticDemandGroup {
                path: run[0].location.path.as_ref(),
                demands: run,
                shared_demands: None,
            })
            .collect::<Vec<_>>();
        self.semantic_grouped(&groups)
    }

    fn take_last_exchange_timings(&mut self) -> Option<TypeFactsExchangeTimings> {
        None
    }

    fn take_last_table_changes(&mut self) -> Option<TypeScriptChanges> {
        None
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
    Ast(#[from] solid_ast_facts::AstFactsError),
    #[error("compiler facts error: {0}")]
    Compiler(#[from] solid_compiler_facts::CompilerFactsError),
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

#[derive(Default)]
pub struct NativeCompilerFacts;

impl CompilerFactsProvider for NativeCompilerFacts {
    fn analyze(&mut self, request: &AnalysisRequest) -> Result<ExecutionMap, BackendError> {
        use dom_expressions_compiler::{CompileOptions, Generate, Wrapper, compile};

        let requested = &request.compiler_options;
        // `None` leaves the compiler's own default wrapper in place; an
        // explicitly empty name is how the checker asks for no wrapper at all.
        let effect_wrapper = match requested.effect_wrapper.as_deref() {
            None => Wrapper::Default,
            Some("") => Wrapper::Disabled,
            Some(name) => Wrapper::Name(name.to_owned()),
        };
        let generate = match requested.generate.as_str() {
            "dom" => Generate::Dom,
            other => {
                return Err(BackendError::NativeCompiler(format!(
                    "semantic tracing supports DOM output only, not `{other}`"
                )));
            }
        };
        let options = CompileOptions {
            filename: Some(request.path.clone()),
            module_name: requested.module_name.clone(),
            generate,
            hydratable: requested.hydratable,
            dev: requested.dev,
            effect_wrapper,
            wrap_conditionals: requested.wrap_conditionals.unwrap_or(true),
            static_marker: requested
                .static_marker
                .clone()
                .unwrap_or_else(|| CompileOptions::default().static_marker),
            built_ins: requested.built_ins.clone(),
            semantic_trace: true,
            ..CompileOptions::default()
        };
        let output = compile(&request.source, &options)
            .map_err(|error| BackendError::NativeCompiler(format!("{}: {error}", request.path)))?;
        let trace = output
            .semantic_trace
            .ok_or(BackendError::MissingExecutionMap)?;
        execution_map_from_trace(&trace, &request.source)
    }
}

/// Projects the compiler's semantic trace onto the checker's execution map.
///
/// The trace is total: every censused JSX site carries a terminal decision, so
/// each one lands in exactly one of the tracked, untracked, or callback
/// categories, and `ExecutionMap::uncovered_jsx_expressions` is empty by
/// construction rather than by luck.
fn execution_map_from_trace(
    trace: &dom_expressions_compiler::SemanticTrace,
    source: &str,
) -> Result<ExecutionMap, BackendError> {
    use dom_expressions_compiler::{
        CallbackDecision, ExecutionSiteKind, TerminalDecision, ValueDecision,
    };
    use solid_compiler_facts::{
        COMPILER_FACTS_PROTOCOL, CallbackRole, CallbackRoleKind, ExecutionRegion, JsxOperation,
        RegionReason,
    };

    let mut map = ExecutionMap {
        compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
        source_hash: solid_facts_core::SourceHash::of(source),
        tracked_regions: Vec::new(),
        untracked_regions: Vec::new(),
        // The compiler has never emitted ownership regions; ownership is
        // derived from the AST and type facts instead.
        ownership_regions: Vec::new(),
        callback_roles: Vec::new(),
        jsx_operations: Vec::new(),
    };

    // Sites arrive ordered by (span, kind), so appending in iteration order
    // keeps every category in the canonical span order `validate` requires.
    for site in &trace.sites {
        let span = Span::new(site.span.start, site.span.end);
        let kind = match site.kind {
            ExecutionSiteKind::JsxChild => "jsx-expression",
            ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                "dynamic-attribute"
            }
            ExecutionSiteKind::ComponentProperty
            | ExecutionSiteKind::ComponentSpread
            | ExecutionSiteKind::ComponentChild => "component-property",
            ExecutionSiteKind::EventHandler => "event-listener",
            ExecutionSiteKind::Ref => "directive-apply",
            ExecutionSiteKind::ControlFlowRender => "control-flow-render",
        };
        map.jsx_operations.push(JsxOperation {
            span,
            kind: kind.into(),
        });

        match site.decision {
            TerminalDecision::Value(ValueDecision::ReactiveRerun) => {
                let reason = match site.kind {
                    ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                        RegionReason::JsxAttribute
                    }
                    _ => RegionReason::JsxChild,
                };
                map.tracked_regions.push(ExecutionRegion { span, reason });
            }
            // `CallerContext` is the dynamic component prop: the expression is
            // handed to the child as a getter and re-evaluated in the child's
            // tracking context. It is deferred, not untracked — treating it as
            // an untracked region would report every `when={count()}` as a
            // stale read.
            TerminalDecision::Value(ValueDecision::CallerContext) => {
                map.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            // A component child is handed to the component and invoked from
            // the component's own render, not from here — a deferred callback
            // even when the value itself is built once.
            TerminalDecision::Value(ValueDecision::EagerOnce)
                if site.kind == ExecutionSiteKind::ComponentChild =>
            {
                map.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            // `EagerOnce` and `Elided` settle at render and never re-run.
            TerminalDecision::Value(ValueDecision::EagerOnce | ValueDecision::Elided) => {
                let reason = match site.kind {
                    ExecutionSiteKind::NativeAttribute | ExecutionSiteKind::NativeSpread => {
                        RegionReason::JsxAttribute
                    }
                    ExecutionSiteKind::ComponentProperty
                    | ExecutionSiteKind::ComponentSpread
                    | ExecutionSiteKind::ComponentChild => RegionReason::ComponentGetter,
                    _ => RegionReason::JsxChild,
                };
                map.untracked_regions.push(ExecutionRegion { span, reason });
            }
            TerminalDecision::Callback(decision) => {
                let role = match decision {
                    CallbackDecision::LaterEvent => CallbackRoleKind::EventHandler,
                    CallbackDecision::RefApply => CallbackRoleKind::DirectiveApply,
                    // A render callback runs at render time under no tracking
                    // scope of its own.
                    CallbackDecision::LaterRender => CallbackRoleKind::Render,
                };
                map.callback_roles.push(CallbackRole { span, role });
            }
        }
    }

    map.validate(source)?;
    Ok(map)
}

#[derive(Clone, Debug)]
pub struct SourceChange {
    pub path: String,
    pub version: u64,
    pub source: Option<String>,
    pub compiler_options: CompilerOptions,
}

const TYPEFACTS_RECOVERY_ATTEMPTS: u32 = 3;

/// Splits a demand list into per-path runs.
///
/// Demands arrive sorted by location, so equal paths are already adjacent and
/// grouping is a single scan.
fn group_demands(demands: &[EntityDemand]) -> Vec<Vec<EntityDemand>> {
    let mut runs: Vec<Vec<EntityDemand>> = Vec::new();
    for demand in demands {
        match runs.last_mut() {
            Some(run) if run[0].location.path == demand.location.path => run.push(demand.clone()),
            _ => runs.push(vec![demand.clone()]),
        }
    }
    runs
}

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
    project_id: String,
    generation: u64,
    sources: HashMap<String, SourceFile>,
    cache: FactsCache,
    last_facts: Option<Arc<ProjectFacts>>,
    typescript: TypeFactsSession,
    known_paths: HashSet<String>,
    last_build_timings: NativeBuildTimings,
}

impl NativeIncrementalSession {
    pub fn open(
        project_id: String,
        sources: Vec<SourceFile>,
        typescript: TypeFactsSession,
    ) -> Result<Self, BackendError> {
        Ok(Self::from_sources(project_id, sources, typescript))
    }

    /// Opens the project and returns the session together with its configured
    /// sources, so callers can seed their own bookkeeping. `TypeFactsSession`
    /// has already issued the `open`, so callers must not open again.
    pub fn open_pipelined(
        project_id: String,
        mut typescript: TypeFactsSession,
    ) -> Result<(Self, Vec<SourceFile>), BackendError> {
        let sources = typescript.configured_sources()?;
        let session = Self::from_sources(project_id, sources.clone(), typescript);
        Ok((session, sources))
    }

    fn from_sources(
        project_id: String,
        sources: Vec<SourceFile>,
        typescript: TypeFactsSession,
    ) -> Self {
        Self {
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
        }
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
                    let facts = Arc::new(facts);
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
            self.project_id.clone(),
            next_generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            cancelled,
            retained,
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
        let (facts, timings) = build_project_native_cached_measured(
            self.project_id.clone(),
            self.generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
        )?;
        self.last_build_timings = timings;
        let facts = Arc::new(facts);
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
            self.project_id.clone(),
            self.generation,
            sources,
            &mut self.typescript,
            &mut self.cache,
            Some(cancelled),
            None,
        )?;
        self.last_build_timings = timings;
        let facts = Arc::new(facts);
        self.last_facts = Some(Arc::clone(&facts));
        Ok(facts)
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
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    compiler: &mut (impl CompilerFactsProvider + ?Sized),
    typescript: &mut impl TypeFactsProvider,
) -> Result<ProjectFacts, BackendError> {
    build_project_cached(
        project_id,
        generation,
        sources,
        compiler,
        typescript,
        &mut FactsCache::default(),
    )
}

pub fn build_project_native(
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_measured(project_id, generation, sources, typescript)
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

#[derive(Clone, Copy, Debug, Default)]
pub struct TypeFactsExchangeTimings {
    pub roundtrip: Duration,
    pub request_send: Duration,
    pub request_bytes: u64,
    pub response_decode: Duration,
    pub response_bytes: u64,
    pub server_request_decode: Duration,
    pub server_analyze: Duration,
    pub server_async: Duration,
    pub server_demand: Duration,
    pub server_assembly: Duration,
    pub server_sort: Duration,
    pub server_close_symbols: Duration,
    pub server_materialized: bool,
    pub server_retained_files: u64,
    pub server_recomputed_files: u64,
    pub server_non_durable_files: u64,
}

impl TypeFactsExchangeTimings {
    #[must_use]
    pub fn encode_and_transport(self) -> Duration {
        self.roundtrip.saturating_sub(
            self.request_send
                .saturating_add(self.response_decode)
                .saturating_add(self.server_request_decode)
                .saturating_add(self.server_analyze),
        )
    }
}

pub fn build_project_native_measured(
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    let project_id = project_id.into();
    let generation = Generation::new(generation).map_err(|_| BackendError::Generation)?;
    let source_files_recomputed = u64::try_from(sources.len()).unwrap_or(u64::MAX);
    let analysis_started = Instant::now();
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(sources.len().max(1));
    let chunk_size = sources.len().div_ceil(workers);
    let analyzed = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (chunk_index, chunk) in sources.chunks(chunk_size.max(1)).enumerate() {
            handles.push(scope.spawn(move || {
                let mut compiler = NativeCompilerFacts;
                chunk
                    .iter()
                    .enumerate()
                    .map(|(offset, file)| {
                        let ast = solid_ast_facts::extract(&file.path, &file.source)?;
                        let request = AnalysisRequest::new(
                            &file.path,
                            Arc::clone(&file.source),
                            file.compiler_options.clone(),
                        );
                        let execution = compiler.analyze(&request)?;
                        execution.validate(&file.source)?;
                        Ok((
                            chunk_index * chunk_size + offset,
                            FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, BackendError>>()
            }));
        }
        let mut analyzed = Vec::with_capacity(sources.len());
        for handle in handles {
            analyzed.extend(handle.join().expect("native facts worker panicked")?);
        }
        Ok::<_, BackendError>(analyzed)
    })?;
    let mut analyzed = analyzed;
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
    let demands = semantic_demands(&files)?;
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
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_cached_measured(project_id, generation, sources, typescript, cache)
        .map(|(facts, _)| facts)
}

pub fn build_project_native_cached_measured(
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
) -> Result<(ProjectFacts, NativeBuildTimings), BackendError> {
    build_project_native_cached_measured_inner(
        project_id, generation, sources, typescript, cache, None, None,
    )
}

pub fn build_project_native_cached_cancellable(
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<ProjectFacts, BackendError> {
    build_project_native_cached_measured_inner(
        project_id,
        generation,
        sources,
        typescript,
        cache,
        Some(cancelled),
        None,
    )
    .map(|(facts, _)| facts)
}

struct RetainedFileFacts<'a> {
    previous: &'a ProjectFacts,
    changed_paths: &'a HashSet<String>,
}

fn build_project_native_cached_measured_inner(
    project_id: impl Into<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typescript: &mut impl TypeFactsProvider,
    cache: &mut FactsCache,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
    retained: Option<RetainedFileFacts<'_>>,
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
    let executions = prepare_native_compiler_parallel(&prepared, cache)?;
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
    let demand_groups = semantic_demand_groups_cached(&files, cache)?;
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
    prepared: &[(SourceFile, Arc<solid_ast_facts::AstFacts>)],
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
        let key = compiler_cache_key(&request)?;
        if let Some(execution) = cache.compiler.get(&key) {
            executions[index] = Some(execution.clone());
        } else {
            misses.push((index, key, file));
        }
    }
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(misses.len().max(1));
    let chunk_size = misses.len().div_ceil(workers);
    let analyzed = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in misses.chunks(chunk_size.max(1)) {
            handles.push(scope.spawn(move || {
                let mut compiler = NativeCompilerFacts;
                chunk
                    .iter()
                    .map(|(index, key, file)| {
                        let request = AnalysisRequest::new(
                            &file.path,
                            Arc::clone(&file.source),
                            file.compiler_options.clone(),
                        );
                        let execution = compiler.analyze(&request)?;
                        execution.validate(&file.source)?;
                        Ok((*index, key.clone(), execution))
                    })
                    .collect::<Result<Vec<_>, BackendError>>()
            }));
        }
        let mut analyzed = Vec::with_capacity(misses.len());
        for handle in handles {
            analyzed.extend(handle.join().expect("native compiler worker panicked")?);
        }
        Ok::<_, BackendError>(analyzed)
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
        let compiler_key = compiler_cache_key(&request)?;
        let execution = if let Some(cached) = cache.compiler.get(&compiler_key) {
            cached.clone()
        } else {
            let execution = Arc::new(compiler.analyze(&request)?);
            cache.compiler.insert(compiler_key, Arc::clone(&execution));
            execution
        };
        execution.validate(&file.source)?;
        let facts = FileFacts::new(generation, Arc::clone(&file.source), ast, execution)?;
        seeds.extend(facts.compiler_seed_locations()?);
        seeds.extend(facts.structural_seed_locations());
        files.push(facts);
    }
    let table = typescript.semantic(semantic_demands_cached(&files, cache)?)?;
    let table = hydrate_structural_file_facts_cached(table, &files, cache);
    ProjectFacts::join(generation, project_id, files, table).map_err(Into::into)
}

fn semantic_demands(files: &[FileFacts]) -> Result<Vec<typefacts::v3::EntityDemand>, BackendError> {
    demand_plan::plan(files)
}

fn structural_accessor_spans(file: &FileFacts) -> HashSet<Span> {
    let mut named_imports = HashMap::<&str, &str>::new();
    let mut namespace_imports = HashSet::<&str>::new();
    for import in &file.ast.imports {
        if !import.module.starts_with("solid-js") {
            continue;
        }
        for binding in &import.bindings {
            match binding.kind {
                solid_ast_facts::ImportKind::Named => {
                    let Some(local) = file.source_text(binding.local.span) else {
                        continue;
                    };
                    named_imports.insert(local, binding.imported.as_deref().unwrap_or(local));
                }
                solid_ast_facts::ImportKind::Namespace => {
                    if let Some(local) = file.source_text(binding.local.span) {
                        namespace_imports.insert(local);
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
        let primitive = if let Some(imported) = named_imports.get(static_callee) {
            Some(*imported)
        } else if let Some((namespace, property)) = static_callee.split_once('.')
            && namespace_imports.contains(namespace)
        {
            Some(property.rsplit('.').next().unwrap_or(property))
        } else {
            None
        };
        if !matches!(
            primitive,
            Some(
                "createSignal"
                    | "createMemo"
                    | "createStore"
                    | "createProjection"
                    | "createOptimistic"
                    | "createOptimisticStore"
            )
        ) {
            continue;
        }
        let source = if binding.shape == solid_ast_facts::BindingShape::Array {
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
    files: &[FileFacts],
    cache: &mut FactsCache,
) -> Result<Vec<typefacts::v3::EntityDemand>, BackendError> {
    let mut demands = Vec::new();
    let mut ordered_files = files.iter().collect::<Vec<_>>();
    ordered_files.sort_by(|left, right| left.path.cmp(&right.path));
    for file in ordered_files {
        let key = format!("{}\0{}", file.path, file.source_hash);
        let per_file = if let Some(cached) = cache.semantic_demands.get(&key) {
            cached
        } else {
            let generated: Arc<[typefacts::v3::EntityDemand]> =
                semantic_demands(std::slice::from_ref(file))?.into();
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
    files: &'a [FileFacts],
    cache: &'a mut FactsCache,
) -> Result<Vec<SemanticDemandGroup<'a>>, BackendError> {
    let mut ordered_files = files.iter().collect::<Vec<_>>();
    ordered_files.sort_by(|left, right| left.path.cmp(&right.path));
    let keys = ordered_files
        .iter()
        .map(|file| format!("{}\0{}", file.path, file.source_hash))
        .collect::<Vec<_>>();
    for (file, key) in ordered_files.iter().zip(&keys) {
        if !cache.semantic_demands.contains_key(key) {
            cache.semantic_demands.insert(
                key.clone(),
                semantic_demands(std::slice::from_ref(*file))?.into(),
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

fn typefacts_location(path: &str, span: solid_facts_core::Span) -> typefacts::Location {
    typefacts::Location {
        path: path.into(),
        start_byte: u64::from(span.start),
        end_byte: u64::from(span.end),
    }
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
                solid_ast_facts::FunctionKind::Arrow | solid_ast_facts::FunctionKind::Expression
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
                        returned.value == solid_ast_facts::ReturnValueKind::Function
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
            arrow: function.kind == solid_ast_facts::FunctionKind::Arrow,
        });
    }
    result
}

fn prepare_ast_parallel(
    sources: Vec<SourceFile>,
    cache: &mut FactsCache,
) -> Result<Vec<(SourceFile, Arc<solid_ast_facts::AstFacts>)>, BackendError> {
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
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(misses.len().max(1));
    let chunk_size = misses.len().div_ceil(workers);
    let parsed = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in misses.chunks(chunk_size.max(1)) {
            handles.push(scope.spawn(move || {
                chunk
                    .iter()
                    .map(|(index, key, path, source)| {
                        Ok((
                            *index,
                            key.clone(),
                            solid_ast_facts::extract(*path, source)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, solid_ast_facts::AstFactsError>>()
            }));
        }
        let mut parsed = Vec::new();
        for handle in handles {
            parsed.extend(handle.join().expect("Oxc worker panicked")?);
        }
        Ok::<_, solid_ast_facts::AstFactsError>(parsed)
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
        solid_facts_core::SourceHash::of(&file.source)
    )
}

fn compiler_cache_key(request: &AnalysisRequest) -> Result<String, BackendError> {
    Ok(format!(
        "{}\0{}\0{}",
        request.path,
        request.source_hash,
        serde_json::to_string(&request.compiler_options)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_compiler_facts::COMPILER_FACTS_PROTOCOL;
    use solid_facts_core::SourceHash;
    use typefacts::SourceDigest;

    /// The wire schema the in-memory stub claims; the real value comes from
    /// the producer's handshake, which no stub performs.
    const STUB_SCHEMA: u64 = 2;

    struct Compiler;
    impl CompilerFactsProvider for Compiler {
        fn analyze(&mut self, request: &AnalysisRequest) -> Result<ExecutionMap, BackendError> {
            Ok(ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: request.source_hash.clone(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            })
        }
    }

    struct CountingCompiler(usize);
    impl CompilerFactsProvider for CountingCompiler {
        fn analyze(&mut self, request: &AnalysisRequest) -> Result<ExecutionMap, BackendError> {
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
        let ast = solid_ast_facts::extract(path, source).unwrap();
        FileFacts::new(
            Generation::new(1).unwrap(),
            source,
            ast,
            ExecutionMap {
                compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                tracked_regions: vec![],
                untracked_regions: vec![],
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
        let fresh_demands = semantic_demands(&files).unwrap();
        let mut cache = FactsCache::default();
        let retained_demands = semantic_demands_cached(&files, &mut cache).unwrap();
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
            "const value = createMemo(async () => 1); export function Card(props: { title: string }) { const key = 'title'; const copy = { ...props }; return <div>{props[key]}{copy.title}{value()}</div>; }",
        );
        let demands = semantic_demands(std::slice::from_ref(&file)).unwrap();

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
            for argument in call
                .arguments
                .iter()
                .filter(|argument| argument.value == solid_ast_facts::ArgumentValueKind::Identifier)
            {
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
                assert!(demands.iter().any(|demand| {
                    demand.location == location && demand.callability && demand.runtime_identity
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
        let planned = semantic_demands(&reversed).unwrap();
        reversed.reverse();
        assert_eq!(
            planned,
            semantic_demands(&reversed).unwrap(),
            "query order must not depend on source traversal order"
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
        let demands = semantic_demands(std::slice::from_ref(&file)).unwrap();
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
