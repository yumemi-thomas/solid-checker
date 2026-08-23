//! Cached cross-generation state for the incremental build:
//! per-file contributions, fingerprints, and reuse checks.

use crate::indexes::CachedAstFileIndex;
use crate::owners::{
    CachedOwnerFile, SettledGateDecisions, binding_returns_reactive_source, returned_arrow_function,
};
use crate::pipeline::available_analysis_workers;
use crate::{
    ActionInvocation, AsyncRead, CacheRetention, ContractCallback, ContractExport,
    ContractGenerationObligation, FunctionNode, OwnerRequirement, Program, ReactiveRead,
    ReactiveSourceKind, ReactiveWrite, RuleOptions, StaticDefect,
};

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::identity::{SymbolId, SymbolInterner, SymbolName};
use crate::indexes::{CrossFileProofDigest, EntitySymbols};
use crate::interproc::{InterproceduralResult, SummaryNode, SummaryRead};
use crate::reachability::ReachabilityTopology;
use crate::symbols::{
    alias_roots_and_source_declarations, entity_symbols, source_discovery_symbol_semantics,
    symbol_alias_targets, symbol_names, symbols_by_root,
};
use sha2::{Digest, Sha256};
use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use solid_facts::core::{SourceHash, SourcePath, Span};
use typefacts::{Declaration, Location};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BuildIdentity {
    pub(crate) dialect: solid_dialect::Version,
    pub(crate) project_id: String,
    pub(crate) generation: u64,
    pub(crate) contracts: Vec<[u8; 32]>,
    /// Per-rule options change what the static pass emits, so two runs with
    /// different options never share a retained program.
    pub(crate) rule_options: RuleOptions,
}

pub(crate) struct RetainedBuild {
    pub(crate) identity: BuildIdentity,
    pub(crate) program: Arc<Program>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDiscoveryDomain {
    dialect: solid_dialect::Version,
    project_id: String,
    contracts: Vec<[u8; 32]>,
}

/// Owns every derived cache retained across Reactive IR generations.
///
/// The builder delegates domain invalidation, idle retention, and the narrow
/// mutable view consumed by one build to this module, so adding a cache family
/// has one policy owner rather than three reset lists to keep synchronized.
#[derive(Default)]
pub(crate) struct IncrementalCacheState {
    ast_indexes: HashMap<SourcePath, CachedAstFileIndex>,
    source_discovery: HashMap<SourcePath, CachedSourceDiscovery>,
    typed_accessors: HashMap<SourcePath, CachedTypedAccessors>,
    interprocedural_graph: HashMap<SourcePath, CachedInterproceduralGraph>,
    interprocedural_results: CachedInterproceduralResults,
    typescript_indexes: Option<CachedTypeScriptIndexes>,
    reachability: Option<CachedReachability>,
    late_stages: Option<CachedLateStages>,
    domain: Option<SourceDiscoveryDomain>,
}

#[derive(Default)]
pub(crate) struct BuildCaches<'a> {
    pub(crate) ast_indexes: Option<&'a mut HashMap<SourcePath, CachedAstFileIndex>>,
    pub(crate) source_discovery: Option<&'a mut HashMap<SourcePath, CachedSourceDiscovery>>,
    pub(crate) typed_accessors: Option<&'a mut HashMap<SourcePath, CachedTypedAccessors>>,
    pub(crate) interprocedural_graph:
        Option<&'a mut HashMap<SourcePath, CachedInterproceduralGraph>>,
    pub(crate) interprocedural_results: Option<&'a mut CachedInterproceduralResults>,
    pub(crate) typescript_indexes: Option<&'a mut Option<CachedTypeScriptIndexes>>,
    pub(crate) reachability: Option<&'a mut Option<CachedReachability>>,
    pub(crate) late_stages: Option<&'a mut Option<CachedLateStages>>,
}

impl IncrementalCacheState {
    /// Selects the source-discovery domain and clears derived state when it
    /// changes. Returns whether invalidation occurred.
    pub(crate) fn ensure_domain(
        &mut self,
        dialect: solid_dialect::Version,
        project_id: &str,
        contracts: &[[u8; 32]],
    ) -> bool {
        let next = SourceDiscoveryDomain {
            dialect,
            project_id: project_id.to_owned(),
            contracts: contracts.to_vec(),
        };
        if self.domain.as_ref() == Some(&next) {
            return false;
        }
        self.clear_derived();
        self.domain = Some(next);
        true
    }

    pub(crate) fn for_build(&mut self) -> BuildCaches<'_> {
        BuildCaches {
            ast_indexes: Some(&mut self.ast_indexes),
            source_discovery: Some(&mut self.source_discovery),
            typed_accessors: Some(&mut self.typed_accessors),
            interprocedural_graph: Some(&mut self.interprocedural_graph),
            interprocedural_results: Some(&mut self.interprocedural_results),
            typescript_indexes: Some(&mut self.typescript_indexes),
            reachability: Some(&mut self.reachability),
            late_stages: Some(&mut self.late_stages),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.clear_derived();
        self.domain = None;
    }

    pub(crate) fn retain_for_idle(&mut self, retention: CacheRetention) {
        if retention == CacheRetention::Performance {
            return;
        }
        self.interprocedural_graph.clear();
        self.interprocedural_results = CachedInterproceduralResults::default();
        self.typescript_indexes = None;
        self.reachability = None;
        if retention == CacheRetention::Compact {
            self.ast_indexes.clear();
            self.source_discovery.clear();
            self.typed_accessors.clear();
            self.late_stages = None;
        }
    }

    fn clear_derived(&mut self) {
        self.ast_indexes.clear();
        self.source_discovery.clear();
        self.typed_accessors.clear();
        self.interprocedural_graph.clear();
        self.interprocedural_results = CachedInterproceduralResults::default();
        self.typescript_indexes = None;
        self.reachability = None;
        self.late_stages = None;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceDiscoveryIdentity {
    pub(crate) source_hash: SourceHash,
    pub(crate) symbols: Vec<SymbolId>,
}

#[derive(Clone, Default)]
pub(crate) struct SourceDiscoveryContribution {
    pub(crate) accessors: Vec<(SymbolId, (SymbolId, Location))>,
    pub(crate) accessor_origins: Vec<(SymbolId, (SymbolId, SymbolId, Location))>,
    pub(crate) setters: Vec<(SymbolId, (SymbolId, Location, bool, ReactiveSourceKind))>,
    pub(crate) actions: Vec<(SymbolId, (SymbolId, Location))>,
    pub(crate) source_kinds: Vec<(SymbolId, ReactiveSourceKind)>,
    pub(crate) source_primitives: Vec<(SymbolId, SymbolName)>,
    pub(crate) source_phases: Vec<(SymbolId, u8)>,
    pub(crate) returned_source_symbols: Vec<SymbolId>,
    pub(crate) summary_source_symbols: Vec<SymbolId>,
    pub(crate) source_owned_write: Vec<(SymbolId, bool)>,
    pub(crate) async_sources: Vec<SymbolId>,
    pub(crate) source_async_options: Vec<(SymbolId, crate::source_discovery::AsyncSourceOptions)>,
    /// Store bindings whose initializer is provably the value form
    /// (`createStore(value)` / `createOptimisticStore(value)`): no compute
    /// node exists, so the store is not a valid `refresh()` target.
    pub(crate) value_form_stores: Vec<SymbolId>,
    pub(crate) contracted_accessor_symbols: Vec<SymbolId>,
}

pub(crate) struct CachedSourceDiscovery {
    pub(crate) identity: SourceDiscoveryIdentity,
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) contribution: SourceDiscoveryContribution,
}

#[derive(Clone)]
pub(crate) struct TypedAccessorContribution {
    pub(crate) owner: Span,
    pub(crate) read: SummaryRead,
}

pub(crate) struct CachedTypedAccessors {
    pub(crate) contributions: Vec<TypedAccessorContribution>,
}

#[derive(Clone)]
pub(crate) enum InterproceduralGraphTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone, Default)]
pub(crate) struct InterproceduralGraphContribution {
    pub(crate) direct_reads: Vec<(Span, SummaryRead)>,
    pub(crate) edges: Vec<(Span, InterproceduralGraphTarget)>,
    /// A composite TypeScript call can be dispatched to several exact local
    /// implementations. These edges stay deferred until their summaries can
    /// be compared; adding all of them eagerly would union incompatible
    /// behavior and make an ambiguous call look certified.
    pub(crate) dispatches: Vec<(Span, Vec<SymbolId>)>,
    pub(crate) invoked_parameters: Vec<(Span, usize)>,
    /// A member invoked on a parameter: `(owner, parameter index, property)`
    /// for `function invoke(reader) { reader.read() }`. The implementation is
    /// not a property of the owner -- each call site supplies it -- so this
    /// records the obligation and leaves resolution to the site.
    pub(crate) invoked_parameter_members: Vec<(Span, usize, String)>,
    pub(crate) callbacks: Vec<(Span, ContractCallback)>,
    pub(crate) callback_forwardings: Vec<(
        Span,
        InterproceduralGraphTarget,
        usize,
        usize,
        Option<String>,
    )>,
    pub(crate) contract_generation_obligations: Vec<(Span, ContractGenerationObligation)>,
    pub(crate) contract_consumer_obligations: Vec<StaticDefect>,
    pub(crate) returned_bindings: Vec<(SymbolId, SymbolId)>,
    pub(crate) factory_calls: Vec<(Span, SymbolId)>,
}

pub(crate) struct CachedInterproceduralGraph {
    pub(crate) nodes: Vec<SummaryNode>,
    pub(crate) contribution: InterproceduralGraphContribution,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum InterproceduralResultDependency {
    Symbol(SymbolId),
    InlineFunction(String, Span),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InterproceduralResultDependencyState {
    Missing,
    Function {
        name: Option<String>,
        summary: Vec<SummaryRead>,
        invoked_parameters: Vec<usize>,
        invoked_parameter_members: Vec<(usize, String)>,
    },
    Returned(Vec<SummaryRead>),
    Inline(Vec<SummaryRead>),
}

pub(crate) struct CachedInterproceduralResultFile {
    pub(crate) dependencies: HashSet<InterproceduralResultDependency>,
    pub(crate) reads: Vec<ReactiveRead>,
    pub(crate) dispatch_obligations: Vec<StaticDefect>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CachedReactiveSource {
    pub(crate) symbol: SymbolId,
    pub(crate) display: SymbolId,
    pub(crate) declaration: Location,
    pub(crate) phase: u8,
}

#[derive(Default)]
pub(crate) struct CachedInterproceduralResults {
    pub(crate) dependency_states:
        HashMap<InterproceduralResultDependency, InterproceduralResultDependencyState>,
    pub(crate) dependency_users: HashMap<InterproceduralResultDependency, usize>,
    pub(crate) files: HashMap<SourcePath, CachedInterproceduralResultFile>,
    pub(crate) reactive_sources: Option<Arc<Vec<CachedReactiveSource>>>,
    pub(crate) contract_exports: CachedContractExports,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ContractNodeKey {
    pub(crate) path: String,
    pub(crate) ordinal: usize,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(crate) struct ContractExportFragment {
    pub(crate) direct: Vec<(String, ContractExport)>,
    pub(crate) syntax: Vec<(String, ContractExport, bool)>,
    pub(crate) dependencies: HashSet<ContractNodeKey>,
}

#[derive(Default)]
pub(crate) struct CachedContractExports {
    pub(crate) nodes: HashMap<ContractNodeKey, ContractExport>,
    pub(crate) files: HashMap<String, ContractExportFragment>,
    pub(crate) aggregate: Option<Arc<BTreeMap<String, ContractExport>>>,
}

pub(crate) type SourceDiscoverySymbolFingerprint = [u8; 32];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceDiscoveryDeclarationSemantics {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) runtime: bool,
}

pub(crate) fn source_discovery_declaration_semantic(
    declaration: &Declaration,
) -> SourceDiscoveryDeclarationSemantics {
    SourceDiscoveryDeclarationSemantics {
        name: declaration.name.to_string(),
        kind: declaration.kind.to_string(),
        runtime: !declaration.location.path.ends_with(".d.ts"),
    }
}

pub(crate) fn source_discovery_symbol_fingerprint(
    alias_target: &str,
    declarations: &[Declaration],
) -> SourceDiscoverySymbolFingerprint {
    fn field(hasher: &mut Sha256, value: &str) {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    field(&mut hasher, alias_target);
    hasher.update(
        u64::try_from(declarations.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for declaration in declarations {
        field(&mut hasher, declaration.name.as_ref());
        field(&mut hasher, declaration.kind.as_ref());
        hasher.update([u8::from(!declaration.location.path.ends_with(".d.ts"))]);
    }
    hasher.finalize().into()
}

pub(crate) struct SourceDiscoveryTypeScriptDelta {
    pub(crate) entity_paths: HashSet<String>,
    pub(crate) file_paths: HashSet<String>,
    pub(crate) semantic_symbol_ids: HashSet<SymbolId>,
}

pub(crate) struct CachedTypeScriptIndexes {
    pub(crate) interner: SymbolInterner,
    pub(crate) symbol_alias_targets: HashMap<SymbolId, SymbolId>,
    pub(crate) aliases: HashMap<SymbolId, SymbolId>,
    pub(crate) symbols_by_root: HashMap<SymbolId, Vec<SymbolId>>,
    pub(crate) source_declarations: HashMap<SymbolId, Declaration>,
    pub(crate) entities: EntitySymbols,
    pub(crate) symbol_names: HashMap<SymbolId, SymbolName>,
    pub(crate) source_discovery_symbol_semantics:
        HashMap<SymbolId, SourceDiscoverySymbolFingerprint>,
    pub(crate) source_discovery_delta: Option<SourceDiscoveryTypeScriptDelta>,
}

/// Below this many project files the five index lanes are cheaper to run in
/// order than to hand to worker threads.
pub(crate) const PARALLEL_INDEX_FILE_THRESHOLD: usize = 256;

/// Builds every TypeScript-derived index for one generation.
///
/// `project_files` is the project's file count; the fan-out decision lives
/// here rather than at the callers so there is one place that also asks
/// whether the host has workers at all. A wasm32-wasip1 reactor build has no
/// thread support, and `Scope::spawn` panics there instead of degrading to an
/// inline call, so the sequential arm is a correctness requirement and not
/// only a small-project optimization.
pub(crate) fn build_typescript_indexes(
    table: &solid_facts::TypeScriptTable,
    dialect: &dyn Dialect,
    project_files: usize,
) -> (CachedTypeScriptIndexes, Duration, Duration) {
    let parallel =
        project_files >= PARALLEL_INDEX_FILE_THRESHOLD && available_analysis_workers() > 1;
    let interner = SymbolInterner::from_table(table);
    let aliases_started = Instant::now();
    let (aliases, source_declarations) = alias_roots_and_source_declarations(table, &interner);
    let aliases_elapsed = aliases_started.elapsed();
    let (
        (entities, entities_elapsed),
        symbol_names,
        symbol_alias_targets,
        symbols_by_root,
        source_discovery_symbol_semantics,
    ) = if parallel {
        std::thread::scope(|scope| {
            let entities = scope.spawn(|| {
                let started = Instant::now();
                let entities = entity_symbols(table, &aliases, &interner);
                (entities, started.elapsed())
            });
            let names = scope.spawn(|| symbol_names(table, &aliases, &interner, dialect));
            let targets = scope.spawn(|| symbol_alias_targets(table, &interner));
            let roots = scope.spawn(|| symbols_by_root(table, &aliases, &interner));
            let semantics = scope.spawn(|| source_discovery_symbol_semantics(table, &interner));
            (
                entities.join().expect("entity index worker panicked"),
                names.join().expect("symbol name index worker panicked"),
                targets.join().expect("symbol target index worker panicked"),
                roots.join().expect("symbol root index worker panicked"),
                semantics
                    .join()
                    .expect("source discovery semantics worker panicked"),
            )
        })
    } else {
        let entities_started = Instant::now();
        let entities = entity_symbols(table, &aliases, &interner);
        let entities_elapsed = entities_started.elapsed();
        (
            (entities, entities_elapsed),
            symbol_names(table, &aliases, &interner, dialect),
            symbol_alias_targets(table, &interner),
            symbols_by_root(table, &aliases, &interner),
            source_discovery_symbol_semantics(table, &interner),
        )
    };
    (
        CachedTypeScriptIndexes {
            interner,
            symbol_alias_targets,
            aliases,
            symbols_by_root,
            source_declarations,
            entities,
            symbol_names,
            source_discovery_symbol_semantics,
            source_discovery_delta: None,
        },
        aliases_elapsed,
        entities_elapsed,
    )
}

pub(crate) struct CachedReachability {
    pub(crate) inputs: HashMap<String, (SourceHash, Arc<solid_facts::ast::AstFacts>)>,
    pub(crate) files: HashMap<SourcePath, CachedReachabilityFile>,
    pub(crate) calls: HashMap<Location, usize>,
    pub(crate) multiplicity_by_path: HashMap<String, Vec<usize>>,
    pub(crate) function_symbols: HashSet<SymbolId>,
}

#[derive(Clone)]
pub(crate) enum ReachabilityTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone)]
pub(crate) struct ReachabilityEdge {
    pub(crate) owner: Option<Span>,
    pub(crate) target: ReachabilityTarget,
}

pub(crate) struct CachedReachabilityFile {
    pub(crate) identity: SourceDiscoveryIdentity,
    /// See [`SemanticLookup::cross_file_proof_digest`]. Reachability roots and
    /// edges can depend on component or callback uses in another file.
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) functions: Vec<FunctionNode>,
    pub(crate) roots: Vec<ReachabilityTarget>,
    pub(crate) edges: Vec<ReachabilityEdge>,
    pub(crate) callback_edges: Vec<(Option<Span>, Vec<ReachabilityTarget>)>,
    pub(crate) call_owners: Vec<Option<Span>>,
    pub(crate) call_owner_indices: Vec<Option<usize>>,
    pub(crate) topology: ReachabilityTopology,
}

pub(crate) type LateStageSourceFingerprint = [u8; 32];

#[derive(Clone)]
pub(crate) struct LateStageFileInput {
    pub(crate) source_hash: SourceHash,
    pub(crate) ast: Arc<solid_facts::ast::AstFacts>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) source_fingerprint: LateStageSourceFingerprint,
}

#[derive(Clone, Default)]
pub(crate) struct LocalAccessResult {
    pub(crate) reads: Vec<Arc<ReactiveRead>>,
    pub(crate) writes: Vec<Arc<ReactiveWrite>>,
    pub(crate) action_invocations: Vec<Arc<ActionInvocation>>,
    pub(crate) async_reads: Vec<Arc<AsyncRead>>,
    pub(crate) strict_read_obligations: usize,
    pub(crate) write_action_obligations: HashSet<(&'static str, String, u64, u64)>,
    pub(crate) dispatch_obligations: Vec<StaticDefect>,
}

pub(crate) struct LocalAccessBuild {
    pub(crate) result: LocalAccessResult,
    pub(crate) reused: bool,
    pub(crate) reused_files: u64,
    pub(crate) recomputed_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalAccessSymbolState {
    pub(crate) accessor: Option<(SymbolId, Location)>,
    pub(crate) accessor_origin: Option<(SymbolId, SymbolId, Location)>,
    pub(crate) setter: Option<(SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) action: Option<(SymbolId, Location)>,
    pub(crate) source_primitive: Option<SymbolId>,
    pub(crate) async_source: bool,
    /// The declared async/hydration options with the project-level rendering
    /// proof folded into `ssr_client_bare` / `server_rendering_unresolved`, so
    /// a fixed import elsewhere invalidates every file reading this source.
    pub(crate) async_options: crate::source_discovery::AsyncSourceOptions,
    pub(crate) contract_reads: Option<Vec<(String, String, Location, String)>>,
    pub(crate) contract_parameter_reads: Option<Vec<(usize, String, String, Location)>>,
    pub(crate) source_kind: Option<ReactiveSourceKind>,
    pub(crate) prop_source: Option<(
        SymbolId,
        Location,
        Option<crate::source_discovery::PropsReactivity>,
        bool,
    )>,
    pub(crate) source_declaration: Option<Declaration>,
    pub(crate) symbol_name: Option<SymbolId>,
}

pub(crate) struct CachedLocalAccessFile {
    pub(crate) source_hash: SourceHash,
    /// See [`SemanticLookup::cross_file_proof_digest`]. Execution and component
    /// classification can depend on callback or JSX uses in another file.
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) dependencies: HashSet<SymbolId>,
    pub(crate) call_multiplicities: Vec<(Location, Option<usize>)>,
    pub(crate) contribution: LocalAccessResult,
}

#[derive(Default)]
pub(crate) struct CachedLocalAccesses {
    pub(crate) aggregate: Option<LocalAccessResult>,
    pub(crate) files: HashMap<SourcePath, CachedLocalAccessFile>,
    pub(crate) dependency_states: HashMap<SymbolId, LocalAccessSymbolState>,
    /// Prop-source identity plus caller classification per props symbol.
    /// Classification is cross-file — a call site in one file decides
    /// findings in another — so a change here re-enters the symbol into the
    /// changed-dependency set even when both files' sources are untouched.
    pub(crate) prop_sources: HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            Option<crate::source_discovery::PropsReactivity>,
            bool,
        ),
    >,
}

/// The proven-source symbol at each exact TypeScript reference location:
/// `path -> byte range -> source symbol`.
pub(crate) type SourceReferenceLocations = HashMap<String, HashMap<(u64, u64), SymbolId>>;

pub(crate) struct CachedLateStages {
    pub(crate) inputs: HashMap<SourcePath, LateStageFileInput>,
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) local_accesses: CachedLocalAccesses,
    pub(crate) interprocedural: Option<InterproceduralResult>,
    pub(crate) missing_owners: Option<Vec<OwnerRequirement>>,
    /// The call-site gate decisions the owner fixed point resolved for
    /// call-site-gated leaf owners, cached beside `missing_owners` under the
    /// same reuse condition: the leaf-operation table is rebuilt every run
    /// and must be re-gated even when the graph itself is reused.
    pub(crate) settled_gates: Option<SettledGateDecisions>,
    pub(crate) owner_files: HashMap<SourcePath, CachedOwnerFile>,
    /// The upstream-compat surface's location-keyed reference map. Retained
    /// under the same condition as `interprocedural`, because both are
    /// functions of the TypeScript table and the proven source set.
    pub(crate) compat_reference_locations: Option<SourceReferenceLocations>,
}

pub(crate) fn same_reachability_ast(
    previous: &solid_facts::ast::AstFacts,
    current: &solid_facts::ast::AstFacts,
) -> bool {
    previous.schema == current.schema
        && previous.source.path == current.source.path
        && previous.calls == current.calls
        && previous.bindings == current.bindings
        && previous.functions == current.functions
        && previous.imports == current.imports
        && previous.exports == current.exports
        && previous.identifiers == current.identifiers
        && previous.awaits == current.awaits
        && previous.unconditional_awaits == current.unconditional_awaits
        && previous.returns == current.returns
        && previous.jsx_elements == current.jsx_elements
        && previous.members == current.members
        && previous.spreads == current.spreads
        && previous.conditional_tests == current.conditional_tests
}

pub(crate) fn same_compiler_semantics(
    previous: &solid_facts::compiler::ExecutionMap,
    current: &solid_facts::compiler::ExecutionMap,
) -> bool {
    previous.compiler_facts_protocol == current.compiler_facts_protocol
        && previous.tracked_regions == current.tracked_regions
        && previous.untracked_regions == current.untracked_regions
        && previous.ownership_regions == current.ownership_regions
        && previous.callback_roles == current.callback_roles
        && previous.jsx_operations == current.jsx_operations
}

pub(crate) fn late_stage_source_fingerprint(
    file: &solid_facts::FileFacts,
) -> LateStageSourceFingerprint {
    fn optional_bool(hasher: &mut Sha256, value: Option<bool>) {
        hasher.update([match value {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        }]);
    }

    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(file.ast.calls.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for call in &file.ast.calls {
        let callee = usize::try_from(call.callee.start)
            .ok()
            .zip(usize::try_from(call.callee.end).ok())
            .and_then(|(start, end)| file.source.get(start..end));
        optional_bool(&mut hasher, callee.map(|_| false));
        if let Some(callee) = callee {
            hasher.update(
                u64::try_from(callee.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(callee.as_bytes());
        }
    }
    hasher.update(
        u64::try_from(file.ast.bindings.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for binding in &file.ast.bindings {
        let shape = binding.call_initializer.and_then(|initializer| {
            file.ast
                .call_at(initializer)
                .map(|call| binding_returns_reactive_source(binding, call))
        });
        optional_bool(&mut hasher, shape);
    }
    let returned_functions = file.ast.returns.iter().filter_map(|returned| {
        (returned.value == solid_facts::ast::ReturnValueKind::Function)
            .then_some(returned.argument)
            .flatten()
    });
    for argument in returned_functions {
        hasher.update([u8::from(returned_arrow_function(&file.ast, argument))]);
    }
    hasher.finalize().into()
}

pub(crate) fn late_stage_inputs_match(cache: &CachedLateStages, facts: &ProjectFacts) -> bool {
    cache.inputs.len() == facts.files.len()
        && facts.files.iter().all(|file| {
            cache
                .inputs
                .get(file.path.as_str())
                .is_some_and(|previous| {
                    (Arc::ptr_eq(&previous.compiler, &file.compiler)
                        || same_compiler_semantics(&previous.compiler, &file.compiler))
                        && (previous.source_hash == file.source_hash
                            || same_reachability_ast(&previous.ast, &file.ast)
                                && previous.source_fingerprint
                                    == late_stage_source_fingerprint(file))
                })
        })
}

pub(crate) fn current_late_stage_inputs(
    facts: &ProjectFacts,
) -> HashMap<SourcePath, LateStageFileInput> {
    facts
        .files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                LateStageFileInput {
                    source_hash: file.source_hash.clone(),
                    ast: file.ast.clone(),
                    compiler: file.compiler.clone(),
                    source_fingerprint: late_stage_source_fingerprint(file),
                },
            )
        })
        .collect()
}

pub(crate) fn refresh_late_stage_inputs(
    inputs: &mut HashMap<SourcePath, LateStageFileInput>,
    facts: &ProjectFacts,
) {
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    inputs.retain(|path, _| current_paths.contains(path.as_str()));
    for file in &facts.files {
        let unchanged = inputs.get(file.path.as_str()).is_some_and(|input| {
            input.source_hash == file.source_hash
                && (Arc::ptr_eq(&input.compiler, &file.compiler)
                    || same_compiler_semantics(&input.compiler, &file.compiler))
        });
        if unchanged {
            continue;
        }
        inputs.insert(
            file.path.clone(),
            LateStageFileInput {
                source_hash: file.source_hash.clone(),
                ast: file.ast.clone(),
                compiler: file.compiler.clone(),
                source_fingerprint: late_stage_source_fingerprint(file),
            },
        );
    }
}

/// One build's cross-generation reuse decisions.
///
/// The decision and the late-stage cache transition happen together, before
/// any stage borrows an individual cache slot. Stages consume this value
/// instead of re-deriving whether the TypeScript table or aggregate inputs
/// are reusable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReusePlan {
    pub(crate) typescript_unchanged: bool,
    pub(crate) late_stages_reusable: bool,
}

impl ReusePlan {
    pub(crate) fn prepare(
        facts: &ProjectFacts,
        late_stages: Option<&mut Option<CachedLateStages>>,
        cross_file_proofs: Option<CrossFileProofDigest>,
    ) -> Self {
        let typescript_unchanged = facts
            .typescript_changes
            .as_ref()
            .is_some_and(|changes| changes.unchanged);
        let late_stages_reusable = typescript_unchanged
            && late_stages
                .as_deref()
                .and_then(Option::as_ref)
                .is_some_and(|cache| {
                    cache.cross_file_proofs == cross_file_proofs
                        && late_stage_inputs_match(cache, facts)
                });

        if let Some(slot) = late_stages {
            if late_stages_reusable {
                if let Some(retained) = slot.as_mut() {
                    for file in &facts.files {
                        if let Some(input) = retained.inputs.get_mut(file.path.as_str()) {
                            input.source_hash.clone_from(&file.source_hash);
                        }
                    }
                }
            } else if let Some(retained) = slot.as_mut() {
                refresh_late_stage_inputs(&mut retained.inputs, facts);
                retained.cross_file_proofs = cross_file_proofs;
                retained.local_accesses.aggregate = None;
                retained.interprocedural = None;
                retained.missing_owners = None;
                retained.settled_gates = None;
                retained.compat_reference_locations = None;
            } else {
                *slot = Some(CachedLateStages {
                    inputs: current_late_stage_inputs(facts),
                    cross_file_proofs,
                    local_accesses: CachedLocalAccesses::default(),
                    interprocedural: None,
                    missing_owners: None,
                    settled_gates: None,
                    owner_files: HashMap::new(),
                    compat_reference_locations: None,
                });
            }
        }

        Self {
            typescript_unchanged,
            late_stages_reusable,
        }
    }
}
