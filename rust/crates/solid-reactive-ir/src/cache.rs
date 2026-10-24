//! Cached cross-generation state for the incremental build:
//! per-file contributions, fingerprints, and reuse checks.

use crate::indexes::CachedAstFileIndex;
use crate::owners::{
    SettledGateDecisions, binding_returns_reactive_source, returned_arrow_function,
};
use crate::pipeline::available_analysis_workers;
use crate::{
    ActionInvocation, AsyncRead, CacheRetention, CallbackSummary, FunctionNode, OwnerRequirement,
    Program, ReactiveRead, ReactiveSourceKind, ReactiveWrite, RuleOptions, StaticDefect,
    UnresolvedCallbackObligation,
};

use crate::{HashMap, HashSet};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::base::reads::{ReadAnswers, ReadProof};
use crate::identity::{SymbolId, SymbolInterner, SymbolName};
use crate::indexes::EntitySymbols;
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
    pub(crate) runtime_configuration: crate::RuntimeConfigurationPremise,
    pub(crate) dialect: solid_dialect::Version,
    pub(crate) project_id: String,
    pub(crate) generation: u64,
    pub(crate) boundary_fingerprints: Vec<[u8; 32]>,
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
    boundary_fingerprints: Vec<[u8; 32]>,
    /// Rule and runtime options feed stages whose cached files and aggregates
    /// are keyed by sources alone (server rendering reaches the local-access
    /// aggregate, rule enablement the compatibility pass), so a different
    /// value is a different domain, as a different boundary is.
    rule_options: RuleOptions,
    /// The redirects every cached index and stage was built under. The
    /// snapshot delta cannot see them, so a change in either direction (none
    /// to some, some to none) is a different domain.
    runtime_symbol_redirects: std::collections::HashMap<String, String>,
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
    interprocedural_graph: HashMap<SourcePath, CachedInterproceduralGraph>,
    interprocedural_results: CachedInterproceduralResults,
    typescript_indexes: Option<CachedTypeScriptIndexes>,
    reachability: Option<CachedReachability>,
    late_stages: Option<CachedLateStages>,
    /// Shadow mode only: the previous build's base indexes, which the next
    /// build updates and compares with its refold ([`crate::base::retained`]).
    base_indexes: Option<crate::base::retained::RetainedBase>,
    /// L2: memo answers with their recorded reads ([`crate::base::memo`]),
    /// reused across builds until a read changes.
    memos: Option<crate::base::memo::build::MemoStore>,
    domain: Option<SourceDiscoveryDomain>,
}

#[derive(Default)]
pub(crate) struct BuildCaches<'a> {
    pub(crate) ast_indexes: Option<&'a mut HashMap<SourcePath, CachedAstFileIndex>>,
    pub(crate) source_discovery: Option<&'a mut HashMap<SourcePath, CachedSourceDiscovery>>,
    pub(crate) interprocedural_graph:
        Option<&'a mut HashMap<SourcePath, CachedInterproceduralGraph>>,
    pub(crate) interprocedural_results: Option<&'a mut CachedInterproceduralResults>,
    pub(crate) typescript_indexes: Option<&'a mut Option<CachedTypeScriptIndexes>>,
    pub(crate) reachability: Option<&'a mut Option<CachedReachability>>,
    pub(crate) late_stages: Option<&'a mut Option<CachedLateStages>>,
    /// Set in shadow mode: the retained base to check, and how to report a
    /// mismatch.
    pub(crate) base_check: Option<(
        &'a mut Option<crate::base::retained::RetainedBase>,
        crate::shadow::ShadowMode,
    )>,
    /// The retained memo store; `None` gives the build a store of its own.
    pub(crate) memos: Option<&'a crate::base::memo::build::MemoStore>,
}

impl IncrementalCacheState {
    /// Selects the source-discovery domain and clears derived state when it
    /// changes. Returns whether invalidation occurred.
    pub(crate) fn ensure_domain(
        &mut self,
        dialect: solid_dialect::Version,
        project_id: &str,
        boundary_fingerprints: &[[u8; 32]],
        rule_options: &RuleOptions,
        runtime_symbol_redirects: &std::collections::HashMap<String, String>,
    ) -> bool {
        let next = SourceDiscoveryDomain {
            dialect,
            project_id: project_id.to_owned(),
            boundary_fingerprints: boundary_fingerprints.to_vec(),
            rule_options: rule_options.clone(),
            runtime_symbol_redirects: runtime_symbol_redirects.clone(),
        };
        if self.domain.as_ref() == Some(&next) {
            return false;
        }
        self.clear_derived();
        self.domain = Some(next);
        true
    }

    /// The caches one build uses. With shadow mode on, the build also checks
    /// the retained base indexes ([`crate::base::retained`]).
    pub(crate) fn for_build(&mut self, shadow: crate::shadow::ShadowMode) -> BuildCaches<'_> {
        BuildCaches {
            base_check: (shadow != crate::shadow::ShadowMode::Off)
                .then_some((&mut self.base_indexes, shadow)),
            ast_indexes: Some(&mut self.ast_indexes),
            source_discovery: Some(&mut self.source_discovery),
            interprocedural_graph: Some(&mut self.interprocedural_graph),
            interprocedural_results: Some(&mut self.interprocedural_results),
            typescript_indexes: Some(&mut self.typescript_indexes),
            reachability: Some(&mut self.reachability),
            late_stages: Some(&mut self.late_stages),
            memos: Some(
                self.memos
                    .get_or_insert_with(crate::base::memo::build::MemoStore::from_env),
            ),
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
        // The interprocedural graph and result chunks stay (ADR
        // 03XX-edit-residual): rebuilding them cost about 95 ms of every
        // app-game edit for about 50 MB, as do reachability's (ADR 0397).
        self.base_indexes = None;
        if retention == CacheRetention::Compact {
            self.interprocedural_graph.clear();
            self.interprocedural_results = CachedInterproceduralResults::default();
            self.reachability = None;
            // Memo answers are small next to what they save; only the
            // compact policy, which keeps no index at all, drops them.
            self.memos = None;
            // The TypeScript indexes are patched from an exact Type Facts
            // delta (instant-edits P4) for a few milliseconds where a rebuild
            // costs tens: they stay under the balanced policy.
            self.typescript_indexes = None;
            self.ast_indexes.clear();
            self.source_discovery.clear();
            self.late_stages = None;
        }
    }

    fn clear_derived(&mut self) {
        self.ast_indexes.clear();
        self.source_discovery.clear();
        self.interprocedural_graph.clear();
        self.interprocedural_results = CachedInterproceduralResults::default();
        self.typescript_indexes = None;
        self.reachability = None;
        self.late_stages = None;
        self.base_indexes = None;
        self.memos = None;
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
}

pub(crate) struct CachedSourceDiscovery {
    pub(crate) identity: SourceDiscoveryIdentity,
    /// What the file's discovery read of the rest of the program
    /// ([`crate::base::reads`]): a returned-source answer, a component
    /// identity, a member target.
    pub(crate) proof: ReadProof,
    pub(crate) contribution: SourceDiscoveryContribution,
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
    /// ADR 0276: the subset of [`Self::edges`] whose call is a completion site
    /// of its owner, so the callee runs on every normal completion of it.
    pub(crate) guaranteed_edges: Vec<(Span, InterproceduralGraphTarget)>,
    /// A composite TypeScript call can be dispatched to several exact local
    /// implementations. These edges stay deferred until their summaries can
    /// be compared; adding all of them eagerly would union incompatible
    /// behavior and make an ambiguous call look certified.
    pub(crate) dispatches: Vec<(Span, Vec<SymbolId>)>,
    pub(crate) invoked_parameters: Vec<(Span, usize)>,
    /// `(owner, parameter index)` for a parameter the owner calls *itself*,
    /// directly, in its own body -- `function f(cb) { cb() }` -- which is the
    /// one `callbacks` row the implementation census can confirm site for
    /// site (ADR 0100). Recorded beside the row rather than in it: the wire
    /// spells this and a primitive's inline position with the same word.
    pub(crate) direct_callback_parameters: Vec<(Span, usize)>,
    /// ADR 0183: `(owner, parameter)` for a parameter written directly as an
    /// eager owned-computation slot (`createMemo(fn)`) by a call that covers
    /// every normal completion of the owner's body. A proposal input only.
    pub(crate) guaranteed_callback_parameters: Vec<(Span, usize)>,
    /// `(owner, protocol, parameter index)` for a property read (`Get`) or a
    /// coercion (`Coerce`) of a parameter's own value written directly in the
    /// owner's body (item A of ways-to-improve § 3.3): the proposal input the
    /// generator describes as a non-call `callbacks` item. Never evidence.
    pub(crate) direct_protocol_parameters:
        Vec<(Span, crate::summary_semantics::InvokeProtocol, usize)>,
    /// `(owner, parameter index, member path)` for a call of a literal-keyed
    /// member of a parameter's own unwritten binding written directly in the
    /// owner's body -- `handler[0](…)` (item B of ways-to-improve § 3.3): the
    /// proposal input beside the member-path `inline` row the pass writes.
    /// Never evidence.
    pub(crate) direct_member_callback_parameters: Vec<(Span, usize, Vec<String>)>,
    /// `(owner, parameter index)` for a parameter whose caller-supplied value
    /// this function neither invokes nor observes inertly — it stores it, hands
    /// it on, or returns it. See
    /// `interproc::push_unaccounted_parameter_escapes`.
    pub(crate) escaped_parameters: Vec<(Span, usize)>,
    /// A member invoked on a parameter, with its owner and execution context,
    /// for `function invoke(reader) { reader.read() }`. The path is the
    /// whole chain from the parameter -- `reader.source.read()` records
    /// `["source", "read"]`, not `["read"]` -- and is empty when the
    /// parameter's own value is read, or when no segment could be named
    /// exactly. The implementation is not a property of the owner -- each call
    /// site supplies it -- so this records the obligation and leaves
    /// resolution to the site.
    pub(crate) invoked_parameter_members: Vec<(Span, crate::interproc::ParameterMemberInvocation)>,
    pub(crate) callbacks: Vec<(Span, CallbackSummary)>,
    pub(crate) callback_forwardings: Vec<(
        Span,
        InterproceduralGraphTarget,
        usize,
        usize,
        crate::interproc::ForwardedAmbientExecution,
    )>,
    pub(crate) unresolved_callback_obligations: Vec<(Span, UnresolvedCallbackObligation)>,
    pub(crate) returned_bindings: Vec<(SymbolId, SymbolId)>,
    pub(crate) factory_calls: Vec<(Span, SymbolId)>,
}

pub(crate) struct CachedInterproceduralGraph {
    pub(crate) nodes: Vec<SummaryNode>,
    pub(crate) contribution: InterproceduralGraphContribution,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    /// What the file's nodes and contribution read of the lookup
    /// ([`crate::base::reads`]).
    pub(crate) proof: ReadProof,
    /// The accessor entries the contribution embeds, by symbol
    /// (`interproc::transport_reads`): another file's discovery can move one.
    pub(crate) accessor_reads: crate::interproc::TransportReads,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum InterproceduralResultDependency {
    Symbol(SymbolId),
    InlineFunction(String, Span),
    /// ADR 0280: a result decided a callee's branches from this call's
    /// arguments, so it depends on the callee's source text itself.
    FunctionSource(String, Span),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InterproceduralResultDependencyState {
    Missing,
    Function {
        name: Option<String>,
        summary: Vec<SummaryRead>,
        invoked_parameters: Vec<usize>,
        invoked_parameter_members: Vec<crate::interproc::ParameterMemberInvocation>,
    },
    Returned(Vec<SummaryRead>),
    Source(solid_facts::core::SourceHash),
}

pub(crate) struct CachedInterproceduralResultFile {
    pub(crate) dependencies: HashSet<InterproceduralResultDependency>,
    pub(crate) reads: Vec<ReactiveRead>,
    pub(crate) dispatch_obligations: Vec<StaticDefect>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    /// What the file's result reads read of the lookup
    /// ([`crate::base::reads`]).
    pub(crate) proof: ReadProof,
    /// What the stage's shared inputs (the accessor resolver, call
    /// transport, the graph) and source discovery's project passes read in
    /// the build that computed the file: one proof for every file computed
    /// in that build, checked once per build.
    pub(crate) stage_proof: Arc<ReadProof>,
    /// What the file's result reads read of the stage's transport tables
    /// (`interproc::transport_reads`).
    pub(crate) transport: crate::interproc::TransportReads,
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
// Threads only when `parallel`; the sequential arm is the `else` branch (wasm32-wasip1).
#[allow(clippy::disallowed_methods)]
pub(crate) fn build_typescript_indexes(
    table: &solid_facts::TypeScriptTable,
    dialect: &dyn Dialect,
    project_files: usize,
    runtime_redirects: &std::collections::HashMap<String, String>,
) -> (CachedTypeScriptIndexes, Duration, Duration) {
    let parallel =
        project_files >= PARALLEL_INDEX_FILE_THRESHOLD && available_analysis_workers() > 1;
    let interner = SymbolInterner::from_table(table);
    let aliases_started = Instant::now();
    let (aliases, source_declarations) =
        alias_roots_and_source_declarations(table, &interner, runtime_redirects);
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
    /// What the file's roots and edges read of the rest of the program (a
    /// component identity, a returned callback's use): [`crate::base::reads`].
    pub(crate) proof: ReadProof,
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
    /// The text, for the syntax a stage parses again
    /// ([`same_reparsed_syntax`]); shared with the facts.
    pub(crate) source: Arc<str>,
}

/// Whether two texts of one file answer the same where the interprocedural
/// stage parses the source again: the accessor transport's `this` uses (a
/// literal receiver's escape census) and array literal slots, which the
/// compared AST facts do not hold. A same-shape edit that adds `this` to a
/// method must not reuse the stage. Parsed only for a file whose text moved.
fn same_reparsed_syntax(path: &str, previous: &str, current: &str) -> bool {
    let path = std::path::Path::new(path);
    solid_facts::ast::receiver_and_array_syntax(path, previous)
        == solid_facts::ast::receiver_and_array_syntax(path, current)
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
    /// What the aggregate read of the rest of the program: every file's
    /// reads, and source discovery's project passes'.
    pub(crate) reads: crate::base::reads::Reads,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalAccessSymbolState {
    pub(crate) accessor: Option<(SymbolId, Location)>,
    pub(crate) typed_source: Option<crate::source_discovery::TypedSource>,
    pub(crate) accessor_origin: Option<(SymbolId, SymbolId, Location)>,
    pub(crate) setter: Option<(SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) action: Option<(SymbolId, Location)>,
    pub(crate) source_primitive: Option<SymbolId>,
    pub(crate) async_source: bool,
    /// The declared async/hydration options with the project-level rendering
    /// proof folded into `ssr_client_bare` / `server_rendering_unresolved`, so
    /// a fixed import elsewhere invalidates every file reading this source.
    pub(crate) async_options: crate::source_discovery::AsyncSourceOptions,
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
    /// What the file's accesses read of the rest of the program (execution
    /// and component classification can depend on uses in another file),
    /// with what source discovery's project passes read:
    /// [`crate::base::reads`].
    pub(crate) proof: ReadProof,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) dependencies: HashSet<SymbolId>,
    pub(crate) call_multiplicities: Vec<(Location, Option<usize>)>,
    pub(crate) contribution: LocalAccessResult,
}

/// The source-discovery tables a file's local accesses read by symbol
/// (`LocalAccessSymbolState`), as the last build that checked the files saw
/// them: the symbols whose entries differ now are exactly the ones whose
/// state may have moved (ADR 0397), whichever file's discovery or
/// project pass moved them.
#[derive(Clone)]
pub(crate) struct LocalAccessSources {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) typed_sources: HashMap<SymbolId, crate::source_discovery::TypedSource>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) async_sources: HashSet<SymbolId>,
    pub(crate) source_async_options: HashMap<SymbolId, crate::source_discovery::AsyncSourceOptions>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) server_rendering: crate::source_discovery::ServerRenderingPremise,
}

impl LocalAccessSources {
    /// The symbols whose entry in some table differs between `self` and
    /// `current`; every async source when the rendering premise moved.
    pub(crate) fn changed_symbols(&self, current: &Self) -> HashSet<SymbolId> {
        fn map<V: PartialEq>(
            previous: &HashMap<SymbolId, V>,
            current: &HashMap<SymbolId, V>,
            changed: &mut HashSet<SymbolId>,
        ) {
            for (symbol, value) in current {
                if previous.get(symbol) != Some(value) {
                    changed.insert(symbol.clone());
                }
            }
            for symbol in previous.keys() {
                if !current.contains_key(symbol) {
                    changed.insert(symbol.clone());
                }
            }
        }
        let mut changed = HashSet::default();
        map(&self.accessors, &current.accessors, &mut changed);
        map(&self.typed_sources, &current.typed_sources, &mut changed);
        map(
            &self.accessor_origins,
            &current.accessor_origins,
            &mut changed,
        );
        map(&self.setters, &current.setters, &mut changed);
        map(&self.actions, &current.actions, &mut changed);
        map(
            &self.source_primitives,
            &current.source_primitives,
            &mut changed,
        );
        map(
            &self.source_async_options,
            &current.source_async_options,
            &mut changed,
        );
        map(&self.source_kinds, &current.source_kinds, &mut changed);
        changed.extend(
            self.async_sources
                .symmetric_difference(&current.async_sources)
                .cloned(),
        );
        if self.server_rendering != current.server_rendering {
            changed.extend(self.async_sources.iter().cloned());
            changed.extend(current.async_sources.iter().cloned());
        }
        changed
    }
}

#[derive(Default)]
pub(crate) struct CachedLocalAccesses {
    pub(crate) aggregate: Option<LocalAccessResult>,
    /// The tables as of the last build that checked the files.
    pub(crate) sources: Option<LocalAccessSources>,
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
    /// What the retained aggregates (local accesses, the interprocedural
    /// result, missing owners, the compatibility reference map) read of the
    /// rest of the program, set once a build computed all of them
    /// ([`Self::reads`]); `None` until then.
    pub(crate) proof: Option<ReadProof>,
    /// This build's reads of the aggregates, gathered while they compute.
    pub(crate) reads: crate::base::reads::Reads,
    /// The proof's reads a pass not yet run in this build must decide
    /// ([`ReusePlan::settle`]).
    pub(crate) undecided: Vec<crate::base::reads::MemoRead>,
    pub(crate) local_accesses: CachedLocalAccesses,
    pub(crate) interprocedural: Option<InterproceduralResult>,
    pub(crate) missing_owners: Option<Vec<OwnerRequirement>>,
    /// The call-site gate decisions the owner fixed point resolved for
    /// call-site-gated leaf owners, cached beside `missing_owners` under the
    /// same reuse condition: the leaf-operation table is rebuilt every run
    /// and must be re-gated even when the graph itself is reused.
    pub(crate) settled_gates: Option<SettledGateDecisions>,
    pub(crate) owners: crate::owners::OwnerState,
    /// The upstream-compat surface's location-keyed reference map. Retained
    /// under the same condition as `interprocedural`, because both are
    /// functions of the TypeScript table and the proven source set.
    pub(crate) compat_reference_locations: Option<SourceReferenceLocations>,
    /// The chunked passes' per-file outputs, each with its own proof
    /// ([`crate::late_chunks`]).
    pub(crate) chunks: crate::late_chunks::LateChunks,
}

/// Whether two analyses of one path's source hold the same AST facts, every
/// table compared, the source hash aside: what the whole-stage reuse of
/// reachability and the late stages asks of a file whose bytes moved. The
/// stages read many more tables than calls, bindings and functions (the
/// role machine reads assignments, logical and conditional expressions,
/// object properties, classes, through another file too), so a table left
/// out of the comparison was a same-length edit the reuse could not see
/// (ADR 0408).
pub(crate) fn same_reachability_ast(
    previous: &solid_facts::ast::AstFacts,
    current: &solid_facts::ast::AstFacts,
) -> bool {
    if previous.schema != current.schema || previous.source.path != current.source.path {
        return false;
    }
    if previous.source.hash == current.source.hash {
        return previous == current;
    }
    let mut rehashed = previous.clone();
    rehashed.source.hash.clone_from(&current.source.hash);
    rehashed == *current
}

pub(crate) fn same_compiler_semantics(
    previous: &solid_facts::compiler::ExecutionMap,
    current: &solid_facts::compiler::ExecutionMap,
) -> bool {
    previous.compiler_facts_protocol == current.compiler_facts_protocol
        && same_compiler_model_semantics(&previous.semantic_model, &current.semantic_model)
        && previous.tracked_regions == current.tracked_regions
        && previous.untracked_regions == current.untracked_regions
        && previous.discarded_regions == current.discarded_regions
        && previous.ownership_regions == current.ownership_regions
        && previous.callback_roles == current.callback_roles
        && previous.jsx_operations == current.jsx_operations
}

fn same_compiler_model_semantics(
    previous: &solid_facts::compiler::CompilerSemanticModel,
    current: &solid_facts::compiler::CompilerSemanticModel,
) -> bool {
    let same_producer_semantics = match (&previous.producer, &current.producer) {
        (None, None) => true,
        (Some(previous), Some(current)) => {
            previous.dialect == current.dialect
                && previous.trace_version == current.trace_version
                && previous.package_version == current.package_version
                && previous.upstream_revision == current.upstream_revision
                && previous.implementation_revision == current.implementation_revision
                && previous.identity_complete == current.identity_complete
        }
        _ => false,
    };
    same_producer_semantics
        && previous.refusal == current.refusal
        && previous.source_operations_complete == current.source_operations_complete
        && previous.generated_operations_complete == current.generated_operations_complete
        && previous.operations == current.operations
        && previous.generated_operations == current.generated_operations
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
                                    == late_stage_source_fingerprint(file)
                                && same_reparsed_syntax(
                                    file.path.as_str(),
                                    &previous.source,
                                    &file.source,
                                ))
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
                    source: Arc::clone(&file.source),
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
                source: Arc::clone(&file.source),
            },
        );
    }
}

/// Whether the TypeScript table is the previous generation's, so that every
/// index and stage keyed on it may be reused.
///
/// Runtime redirects are generation-request facts outside the retained
/// TypeScript snapshot: the delta cannot see them, and the index rebuild
/// already honours them (a non-empty map forces it). Every consumer of this
/// decision therefore shares it, rather than each re-deriving it from the
/// delta; a build with redirects is never "unchanged", and so neither is the
/// build after it.
pub(crate) fn typescript_unchanged(facts: &ProjectFacts) -> bool {
    facts
        .typescript_changes
        .as_ref()
        .is_some_and(|changes| changes.unchanged)
        && facts.runtime_symbol_redirects.is_empty()
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
    /// The late aggregates' proof holds so far, but for memo reads that need
    /// a derived table a pass publishes later in this build (the resolver's):
    /// [`Self::settle`] decides once it has run. Until then nothing is
    /// reusable and the cache is untouched.
    pub(crate) late_stages_pending: bool,
}

impl ReusePlan {
    pub(crate) fn prepare(
        facts: &ProjectFacts,
        late_stages: Option<&mut Option<CachedLateStages>>,
        answers: &dyn ReadAnswers,
    ) -> Self {
        let typescript_unchanged = typescript_unchanged(facts);
        let mut undecided = Vec::new();
        let verdict = if typescript_unchanged {
            late_stages
                .as_deref()
                .and_then(Option::as_ref)
                .filter(|cache| late_stage_inputs_match(cache, facts))
                .and_then(|cache| cache.proof.as_ref())
                .map(|proof| proof.verdict(answers))
        } else {
            None
        };
        let late_stages_reusable = match verdict {
            Some(crate::base::reads::Verdict::Holds) => true,
            Some(crate::base::reads::Verdict::Undecided(reads)) => {
                undecided = reads;
                false
            }
            Some(crate::base::reads::Verdict::Moved(miss)) => {
                if std::env::var_os("SOLID_CHECKER_IR_READ_MISSES")
                    .is_some_and(|value| value != "0")
                {
                    eprintln!("{}", serde_json::json!({ "irReadMiss": miss }));
                }
                false
            }
            None => false,
        };
        let late_stages_pending = !undecided.is_empty();
        if let Some(slot) = late_stages {
            if late_stages_pending {
                if let Some(retained) = slot.as_mut() {
                    retained.undecided = undecided;
                }
            } else {
                settle_late_stages(slot, facts, late_stages_reusable);
            }
        }
        Self {
            typescript_unchanged,
            late_stages_reusable,
            late_stages_pending,
        }
    }

    /// Decides a pending reuse ([`Self::late_stages_pending`]) once the pass
    /// that publishes what its undecided reads need has run in this build.
    pub(crate) fn settle(
        &mut self,
        facts: &ProjectFacts,
        late_stages: Option<&mut Option<CachedLateStages>>,
        answers: &dyn ReadAnswers,
    ) {
        if !self.late_stages_pending {
            return;
        }
        self.late_stages_pending = false;
        let Some(slot) = late_stages else {
            return;
        };
        let reusable = slot.as_mut().is_some_and(|cache| {
            let undecided = std::mem::take(&mut cache.undecided);
            cache
                .proof
                .as_ref()
                .is_some_and(|proof| proof.recheck(&undecided, answers))
        });
        self.late_stages_reusable = reusable;
        settle_late_stages(slot, facts, reusable);
    }
}

/// The late-stage cache transition of a decided reuse.
fn settle_late_stages(slot: &mut Option<CachedLateStages>, facts: &ProjectFacts, reusable: bool) {
    if reusable {
        if let Some(retained) = slot.as_mut() {
            for file in &facts.files {
                if let Some(input) = retained.inputs.get_mut(file.path.as_str()) {
                    input.source_hash.clone_from(&file.source_hash);
                    input.source = Arc::clone(&file.source);
                }
            }
        }
    } else if let Some(retained) = slot.as_mut() {
        refresh_late_stage_inputs(&mut retained.inputs, facts);
        retained.proof = None;
        retained.reads = crate::base::reads::Reads::default();
        retained.undecided = Vec::new();
        retained.local_accesses.aggregate = None;
        retained.interprocedural = None;
        retained.missing_owners = None;
        retained.settled_gates = None;
        retained.compat_reference_locations = None;
    } else {
        *slot = Some(CachedLateStages {
            inputs: current_late_stage_inputs(facts),
            proof: None,
            reads: crate::base::reads::Reads::default(),
            undecided: Vec::new(),
            local_accesses: CachedLocalAccesses::default(),
            interprocedural: None,
            missing_owners: None,
            settled_gates: None,
            owners: crate::owners::OwnerState::default(),
            compat_reference_locations: None,
            chunks: crate::late_chunks::LateChunks::default(),
        });
    }
}
