//! Build entry points and the staged incremental pipeline that
//! assembles a `Program`, plus the bounded-parallelism helpers.

use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use crate::cache::{BuildCaches, ReusePlan, build_typescript_indexes};
use crate::contracts::{ResolvedContractBinding, resolve_contract_imports};
use crate::identity::{SymbolId, SymbolName};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use crate::reachability::{ReachabilityInputs, reachability_stage};
use crate::source_discovery::{StageContext, discover_sources};
use crate::symbols::{add_solid_import_names, patch_typescript_indexes};
use crate::timings::{ReactiveIrStage, StageClock};
use crate::{
    ActionInvocation, AsyncRead, BuildError, BuildTimings, ContractExport,
    ContractGenerationObligation, LeafOwnerOperation, ObligationCounts, OwnerRequirement,
    PackageContract, PrimitiveCreation, Program, ReactiveRead, ReactiveSourceKind, ReactiveWrite,
    RuleOptions, Solid1xRuleOptions, StaticDefect, StaticViolation, location_order,
};
use crate::{
    cleanup, directives, owners, reactive_analysis, server_rules, static_api, static_rules,
};
use solid_dialect::Dialect;
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::Location;

/// The program under assembly: every table and obligation counter the
/// pipeline's stages fill, owned in one place so a stage's writes are part
/// of its signature rather than ambient mutation of a shared function body.
#[derive(Default)]
pub(crate) struct ProgramDraft {
    pub(crate) reads: Vec<ReactiveRead>,
    pub(crate) writes: Vec<ReactiveWrite>,
    pub(crate) action_invocations: Vec<ActionInvocation>,
    pub(crate) async_reads: Vec<AsyncRead>,
    pub(crate) static_violations: Vec<StaticViolation>,
    pub(crate) static_defects: Vec<StaticDefect>,
    pub(crate) leaf_operations: Vec<LeafOwnerOperation>,
    pub(crate) directive_creations: Vec<PrimitiveCreation>,
    pub(crate) missing_owners: Vec<OwnerRequirement>,
    pub(crate) contract_exports: Arc<BTreeMap<String, ContractExport>>,
    pub(crate) contract_generation_obligations: Vec<ContractGenerationObligation>,
    pub(crate) strict_read_obligations: usize,
    pub(crate) write_action_obligations: HashSet<(&'static str, String, u64, u64)>,
    /// One static diagnostic per (rule, file, offset) identity, across the
    /// stages that share an identity space.
    pub(crate) seen_diagnostics: HashSet<(&'static str, Arc<str>, u64)>,
}

impl ProgramDraft {
    /// Adds a version-neutral static defect once per rule, path, and offset.
    /// Several passes can discover the same fact through different semantic
    /// routes, so the draft owns deduplication for all of them.
    pub(crate) fn push_defect(&mut self, identity: &'static str, defect: StaticDefect) {
        if self.seen_diagnostics.insert((
            identity,
            defect.location.path.clone(),
            defect.location.start_byte,
        )) {
            self.static_defects.push(defect);
        }
    }

    /// Orders every table by location and assembles the final [`Program`].
    pub(crate) fn into_program(mut self, factory_instances: usize) -> Program {
        self.reads
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.writes
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.action_invocations
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.static_violations
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.static_defects
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.directive_creations
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.missing_owners
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.async_reads
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.contract_generation_obligations
            .sort_by(|left, right| location_order(&left.location, &right.location));
        Program {
            reads: self.reads,
            writes: self.writes,
            actions: self.action_invocations,
            leaf_operations: self.leaf_operations,
            static_violations: self.static_violations,
            static_defects: self.static_defects,
            directive_creations: self.directive_creations,
            missing_owners: self.missing_owners,
            async_reads: self.async_reads,
            contract_exports: self.contract_exports,
            contract_generation_obligations: self.contract_generation_obligations,
            obligation_counts: ObligationCounts {
                strict_reads: self.strict_read_obligations,
                writes_and_actions: self.write_action_obligations.len(),
                factory_instances,
            },
        }
    }
}

/// The shared read-only environment for the stages that run after source
/// discovery and reachability have settled: project facts, the dialect, and
/// the discovery-derived lookup tables. Stages borrow it immutably; their
/// outputs go to the [`ProgramDraft`].
pub(crate) struct AnalysisContext<'a> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) dialect: &'a dyn Dialect,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolName>,
    pub(crate) aliases: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) setters: &'a HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) uncertain_prop_sources: &'a HashSet<SymbolId>,
    pub(crate) props_reactivity: &'a crate::source_discovery::PropsReactivityIndex,
    pub(crate) semantic_lookup: &'a SemanticLookup<'a>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) source_owned_write: &'a HashMap<SymbolId, bool>,
    pub(crate) value_form_stores: &'a HashSet<SymbolId>,
    pub(crate) reachable_calls: &'a HashMap<Location, usize>,
    pub(crate) symbols_by_root: &'a HashMap<SymbolId, Vec<SymbolId>>,
    pub(crate) contracted: &'a HashMap<SymbolId, ResolvedContractBinding>,
    pub(crate) rule_options: &'a RuleOptions,
    pub(crate) solid1x_rule_options: &'a Solid1xRuleOptions,
}

pub fn build(facts: &ProjectFacts, dialect: &dyn Dialect) -> Result<Program, BuildError> {
    build_with_contracts(facts, dialect, &[])
}

pub fn build_with_contracts(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    contracts: &[PackageContract],
) -> Result<Program, BuildError> {
    build_with_contracts_measured(facts, dialect, contracts).map(|(program, _)| program)
}

pub fn build_with_contracts_measured(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    contracts: &[PackageContract],
) -> Result<(Program, BuildTimings), BuildError> {
    build_with_contracts_measured_incremental(
        facts,
        dialect,
        contracts,
        &RuleOptions::default(),
        BuildCaches::default(),
    )
}

/// The staged incremental pipeline. One stage per clock span, in order:
///
/// 1. Project indexes, the late-stage cache gate, TypeScript indexes,
///    symbol names, contract resolution, and the semantic lookup.
/// 2. Source discovery and reachability, in two parallel lanes.
/// 3. The static prepass ([`static_rules::static_prepass`]).
/// 4. Local accesses and the interprocedural fixed point, two lanes again.
/// 5. Returns-conditionally, the upstream-compat pass, and the leaf and
///    cleanup tables.
/// 6. Static API checks, directive discovery, and the owner fixed point.
/// 7. Final ordering and assembly ([`ProgramDraft::into_program`]).
///
/// Stages read the shared [`AnalysisContext`], write the [`ProgramDraft`],
/// and end their span on the [`StageClock`]; each late-stage cache sub-slot
/// is handed to exactly one stage. See
/// `docs/pipeline-orchestrator-redesign.md`.
pub(crate) fn build_with_contracts_measured_incremental(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    contracts: &[PackageContract],
    rule_options: &RuleOptions,
    caches: BuildCaches<'_>,
) -> Result<(Program, BuildTimings), BuildError> {
    let BuildCaches {
        ast_indexes: ast_indexes_cache,
        source_discovery: source_discovery_cache,
        typed_accessors: typed_accessor_cache,
        interprocedural_graph: interprocedural_graph_cache,
        interprocedural_results: interprocedural_result_cache,
        typescript_indexes: typescript_indexes_cache,
        reachability: mut reachability_cache,
        late_stages: mut late_stage_cache,
    } = caches;
    let emit_timings = std::env::var_os("SOLID_CHECKER_TIMINGS").is_some();
    let total_started = Instant::now();
    let mut clock = StageClock::new(emit_timings);
    let mut build_timings = BuildTimings::default();
    let substage_started = Instant::now();
    let owned_ast_indexes;
    let ast_indexes = if let Some(cache) = ast_indexes_cache {
        let current_paths = facts
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        cache.retain(|path, _| current_paths.contains(path.as_str()));
        for file in &facts.files {
            if cache
                .get(file.path.as_str())
                .is_some_and(|index| Arc::ptr_eq(&index.ast, &file.ast))
            {
                continue;
            }
            cache.insert(file.path.clone(), CachedAstFileIndex::new(file));
        }
        &*cache
    } else {
        owned_ast_indexes = facts
            .files
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        &owned_ast_indexes
    };
    let project_indexes = ProjectIndexes::new(facts, ast_indexes);
    build_timings.project_indexes = substage_started.elapsed();
    let typescript_unchanged = facts
        .typescript_changes
        .as_ref()
        .is_some_and(|changes| changes.unchanged);
    let owned_typescript_indexes;
    let typescript_indexes = if let Some(cache) = typescript_indexes_cache {
        let patch_timings = (!typescript_unchanged)
            .then(|| {
                cache.as_mut().and_then(|cached| {
                    facts.typescript_changes.as_ref().and_then(|changes| {
                        patch_typescript_indexes(
                            cached,
                            &facts.typescript,
                            &project_indexes.symbols_by_id,
                            dialect,
                            changes,
                        )
                    })
                })
            })
            .flatten();
        let indexes_patched = patch_timings.is_some();
        if let Some((alias_roots, entity_symbols)) = patch_timings {
            build_timings.alias_roots = alias_roots;
            build_timings.entity_symbols = entity_symbols;
            build_timings.alias_and_entity_indexes = alias_roots + entity_symbols;
        }
        if (!typescript_unchanged && !indexes_patched) || cache.is_none() {
            let substage_started = Instant::now();
            let (indexes, alias_roots, entity_symbols) =
                build_typescript_indexes(&facts.typescript, dialect, facts.files.len());
            build_timings.alias_roots = alias_roots;
            build_timings.entity_symbols = entity_symbols;
            build_timings.alias_and_entity_indexes = substage_started.elapsed();
            *cache = Some(indexes);
        } else {
            build_timings.typescript_indexes_reused = true;
        }
        cache.as_ref().expect("TypeScript indexes initialized")
    } else {
        let substage_started = Instant::now();
        let (indexes, alias_roots, entity_symbols) =
            build_typescript_indexes(&facts.typescript, dialect, facts.files.len());
        build_timings.alias_roots = alias_roots;
        build_timings.entity_symbols = entity_symbols;
        build_timings.alias_and_entity_indexes = substage_started.elapsed();
        owned_typescript_indexes = indexes;
        &owned_typescript_indexes
    };
    let aliases = &typescript_indexes.aliases;
    let source_declarations = &typescript_indexes.source_declarations;
    let entities = &typescript_indexes.entities;
    let substage_started = Instant::now();
    let mut symbol_names = typescript_indexes.symbol_names.clone();
    add_solid_import_names(facts, entities, dialect, &mut symbol_names);
    build_timings.symbol_name_indexes = substage_started.elapsed();
    let substage_started = Instant::now();
    let mut resolved_contracts =
        resolve_contract_imports(facts, contracts, entities, dialect, &rule_options.runtime);
    build_timings.contract_resolution = substage_started.elapsed();
    let missing_contract_exports = std::mem::take(&mut resolved_contracts.missing_exports);
    let semantic_lookup = SemanticLookup::new(
        facts,
        ast_indexes,
        entities,
        &symbol_names,
        dialect,
        &resolved_contracts,
        rule_options.runtime.program_is_closed(),
    );
    let semantic_lookup = &semantic_lookup;
    let reuse = ReusePlan::prepare(
        facts,
        late_stage_cache.as_deref_mut(),
        semantic_lookup.cross_file_proof_digest(),
    );
    // Source discovery does not inspect missing exports, and the static prepass
    // owns them after the two independent index passes complete.
    let mut draft = ProgramDraft {
        static_defects: missing_contract_exports,
        ..ProgramDraft::default()
    };
    let mut owned_reachable_calls = None;
    // Source discovery and reachability are independent lanes over the same
    // settled indexes. Both are prepared once here so the two arms below
    // differ only in how they are scheduled.
    let source_context = StageContext {
        facts,
        project_indexes: &project_indexes,
        typescript_indexes,
        entities,
        source_declarations,
        symbol_names: &symbol_names,
        semantic_lookup,
        resolved_contracts: &resolved_contracts,
        contracts,
        runtime: &rule_options.runtime,
    };
    let discover = move || {
        let mut timings = BuildTimings::default();
        let sources = discover_sources(
            &source_context,
            source_discovery_cache,
            reuse.typescript_unchanged,
            &mut timings,
            emit_timings,
        );
        (sources, timings)
    };
    let reachability_inputs = ReachabilityInputs {
        facts,
        indexes: &project_indexes,
        entities,
        symbol_names: &symbol_names,
        lookup: semantic_lookup,
        typescript_unchanged: reuse.typescript_unchanged,
        typescript_delta: typescript_indexes.source_discovery_delta.as_ref(),
    };
    let source_discovery = if available_analysis_workers() <= 1 {
        // No worker is available, so the lanes run one after the other. A
        // wasm32-wasip1 reactor build has no threads at all and `Scope::spawn`
        // panics there, so this arm is a correctness requirement.
        //
        // Reachability closes the stage span before discovery starts:
        // `discover_sources` owns its own stage clock, and counting its time
        // inside this span too would make the stage sum exceed the real total.
        owned_reachable_calls = reachability_stage(
            reachability_inputs,
            reachability_cache.as_deref_mut(),
            &mut build_timings,
        );
        clock.finish(&mut build_timings, ReactiveIrStage::IndexesAndReachability);
        let (source_discovery, discovery_timings) = discover();
        build_timings.absorb_source_discovery(&discovery_timings);
        source_discovery
    } else {
        std::thread::scope(|scope| {
            let shared_worker_limit = analysis_worker_limit_for_lanes(2);
            let source_discovery_handle = scope.spawn(move || {
                let _worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
                discover()
            });
            let reachability_worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
            owned_reachable_calls = reachability_stage(
                reachability_inputs,
                reachability_cache.as_deref_mut(),
                &mut build_timings,
            );
            drop(reachability_worker_limit);
            clock.finish(&mut build_timings, ReactiveIrStage::IndexesAndReachability);
            let (source_discovery, discovery_timings) = source_discovery_handle
                .join()
                .expect("parallel source discovery worker panicked");
            build_timings.absorb_source_discovery(&discovery_timings);
            source_discovery
        })
    };
    let reachable_calls = if let Some(cache) = reachability_cache {
        &cache.as_ref().expect("reachability initialized").calls
    } else {
        owned_reachable_calls
            .as_ref()
            .expect("owned reachability initialized")
    };
    // discover_sources owns its own stage clock, and in the sequential arm it
    // also ran after the stage above closed; restart this function's clock so
    // the static-prepass stage measures only the prepass loops.
    clock.restart();

    let analysis = AnalysisContext {
        facts,
        dialect,
        entities,
        symbol_names: &symbol_names,
        aliases,
        accessors: &source_discovery.accessors,
        setters: &source_discovery.setters,
        actions: &source_discovery.actions,
        prop_sources: &source_discovery.prop_sources,
        uncertain_prop_sources: &source_discovery.uncertain_prop_sources,
        props_reactivity: &source_discovery.props_reactivity,
        semantic_lookup,
        source_kinds: &source_discovery.source_kinds,
        source_primitives: &source_discovery.source_primitives,
        source_owned_write: &source_discovery.source_owned_write,
        value_form_stores: &source_discovery.value_form_stores,
        reachable_calls,
        symbols_by_root: &typescript_indexes.symbols_by_root,
        contracted: &resolved_contracts.by_symbol,
        rule_options,
        solid1x_rule_options: &rule_options.solid1x,
    };
    static_rules::static_prepass(&analysis, &mut draft);
    clock.finish(&mut build_timings, ReactiveIrStage::StaticPrepass);
    let factory_instances = reactive_analysis::collect_project(
        reactive_analysis::ProjectInputs {
            ctx: &analysis,
            source: &source_discovery,
            project_indexes: &project_indexes,
            source_declarations,
            typescript_delta: typescript_indexes.source_discovery_delta.as_ref(),
        },
        reactive_analysis::IncrementalCaches {
            typed_accessors: typed_accessor_cache,
            interprocedural_graph: interprocedural_graph_cache,
            interprocedural_results: interprocedural_result_cache,
            late_stages: late_stage_cache.as_deref_mut().and_then(Option::as_mut),
        },
        reuse,
        &mut draft,
        &mut build_timings,
        &mut clock,
    );
    cleanup::collect_project(&analysis, &mut draft);
    clock.finish(&mut build_timings, ReactiveIrStage::LeafAndCleanup);
    static_api::check_project(&analysis, &mut draft);
    server_rules::check_project(&analysis, &mut draft);
    clock.finish(&mut build_timings, ReactiveIrStage::StaticApi);
    directives::discover_directive_creations(&analysis, &mut draft);
    clock.finish(&mut build_timings, ReactiveIrStage::Directives);
    owners::collect_project(
        &analysis,
        &project_indexes,
        &source_discovery.retained_source_paths,
        late_stage_cache.and_then(Option::as_mut),
        reuse.late_stages_reusable,
        &mut draft,
        &mut build_timings,
    );
    clock.finish(&mut build_timings, ReactiveIrStage::OwnerFixedPoint);
    let program = draft.into_program(factory_instances);
    clock.finish(&mut build_timings, ReactiveIrStage::FinalOrdering);
    build_timings.total = total_started.elapsed();
    Ok((program, build_timings))
}

pub(crate) fn parallel_file_results<R, F>(files: &[FileFacts], analyze: F) -> Vec<R>
where
    R: Send,
    F: Fn(&FileFacts) -> R + Sync,
{
    parallel_slice_results(files, analyze)
}

thread_local! {
    static ANALYSIS_WORKER_LIMIT: Cell<usize> = const { Cell::new(usize::MAX) };
}

pub(crate) struct AnalysisWorkerLimit {
    pub(crate) previous: usize,
}

impl AnalysisWorkerLimit {
    pub(crate) fn enter(limit: usize) -> Self {
        let previous = ANALYSIS_WORKER_LIMIT.replace(limit.max(1));
        Self { previous }
    }
}

impl Drop for AnalysisWorkerLimit {
    fn drop(&mut self) {
        ANALYSIS_WORKER_LIMIT.set(self.previous);
    }
}

pub(crate) fn available_analysis_workers() -> usize {
    let available = std::thread::available_parallelism().map_or(1, usize::from);
    ANALYSIS_WORKER_LIMIT.with(|limit| available.min(limit.get()))
}

pub(crate) fn analysis_worker_limit_for_lanes(lanes: usize) -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .div_ceil(lanes.max(1))
}

pub(crate) fn parallel_slice_results<T, R, F>(items: &[T], analyze: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let workers = available_analysis_workers().min(items.len());
    if workers <= 1 || items.len() < 256 {
        return items.iter().map(analyze).collect();
    }
    let chunk_size = items.len().div_ceil(workers);
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || chunk.iter().map(analyze).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked")
            })
            .collect()
    })
}

pub(crate) fn parallel_file_chunk_results<R, F>(files: &[FileFacts], analyze: F) -> Vec<R>
where
    R: Send,
    F: Fn(&[FileFacts]) -> R + Sync,
{
    let workers = available_analysis_workers().min(files.len());
    if workers <= 1 || files.len() < 256 {
        return vec![analyze(files)];
    }
    let chunk_size = files.len().div_ceil(workers);
    std::thread::scope(|scope| {
        files
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || analyze(chunk))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked")
            })
            .collect()
    })
}
