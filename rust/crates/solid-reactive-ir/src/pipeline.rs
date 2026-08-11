//! Build entry points and the staged incremental pipeline that
//! assembles a `Program`, plus the bounded-parallelism helpers.

use crate::*;

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::cleanup::{cleanup_returns_for_file, leaf_owner_operations_for_file};
use crate::contracts::{ResolvedContractBinding, resolve_contract_imports};
use crate::identity::SymbolId;
use crate::indexes::{CachedAstFileIndex, ProjectIndexes, SemanticLookup};
use crate::interproc::{InterproceduralContext, InterproceduralTimings};
use crate::reachability::{
    ReachabilityInputs, ReachabilityState, reachable_call_multiplicity,
    reachable_call_multiplicity_incremental,
};
use crate::static_api::StaticApiContext;
use crate::symbols::{add_solid_import_names, patch_typescript_indexes, references_for_sources};
use solid_dialect::Dialect;
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::Location;

/// Times the pipeline's stages and emits the `SOLID_CHECKER_TIMINGS` stage
/// lines. Each build lane owns one clock; a stage ends with [`finish`],
/// which records into the selected [`BuildTimings`] field, emits the stage
/// line, and starts timing the next stage.
///
/// [`stage_line`] is the single place the emitted shape is produced — the
/// names and JSON form are read by the performance tooling and must not
/// drift.
///
/// [`finish`]: StageClock::finish
/// [`stage_line`]: StageClock::stage_line
pub(crate) struct StageClock {
    started: Instant,
    emit: bool,
}

impl StageClock {
    pub(crate) fn new(emit: bool) -> Self {
        Self {
            started: Instant::now(),
            emit,
        }
    }

    pub(crate) fn finish(
        &mut self,
        timings: &mut BuildTimings,
        field: fn(&mut BuildTimings) -> &mut Duration,
        name: &str,
    ) {
        let elapsed = self.started.elapsed();
        *field(timings) = elapsed;
        self.record(name, elapsed);
        self.started = Instant::now();
    }

    /// Emits a stage line for a duration measured outside the clock.
    pub(crate) fn record(&self, name: &str, elapsed: Duration) {
        if self.emit {
            eprintln!("{}", Self::stage_line(name, elapsed));
        }
    }

    /// The time since the current stage began, without ending it.
    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Starts the next stage without recording the current one, for
    /// boundaries whose time was already accounted through [`Self::record`].
    pub(crate) fn restart(&mut self) {
        self.started = Instant::now();
    }

    pub(crate) fn stage_line(name: &str, elapsed: Duration) -> String {
        format!(
            "{{\"reactiveIrStage\":\"{}\",\"elapsedNs\":{}}}",
            name,
            elapsed.as_nanos()
        )
    }
}

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
    pub(crate) leaf_operations: Vec<LeafOwnerOperation>,
    pub(crate) invalid_cleanup_returns: Vec<InvalidCleanupReturn>,
    pub(crate) unresolved_cleanup_returns: Vec<UnresolvedCleanupReturn>,
    pub(crate) directive_creations: Vec<PrimitiveCreation>,
    pub(crate) missing_owners: Vec<OwnerRequirement>,
    pub(crate) contract_exports: Arc<BTreeMap<String, ContractExport>>,
    pub(crate) contract_generation_obligations: Vec<ContractGenerationObligation>,
    pub(crate) strict_read_obligations: usize,
    pub(crate) write_action_obligations: HashSet<(&'static str, String, u64, u64)>,
    /// One static violation per (rule, file, offset) identity, across the
    /// stages that share an identity space.
    pub(crate) seen_static: HashSet<(&'static str, Arc<str>, u64)>,
}

impl ProgramDraft {
    /// Orders every table by location and assembles the final [`Program`].
    pub(crate) fn into_program(mut self, factory_instances: usize) -> Program {
        self.reads.sort_by(|left, right| {
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
        self.writes
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.action_invocations
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.invalid_cleanup_returns
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.unresolved_cleanup_returns
            .sort_by(|left, right| location_order(&left.location, &right.location));
        self.static_violations
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
            invalid_cleanup_returns: self.invalid_cleanup_returns,
            unresolved_cleanup_returns: self.unresolved_cleanup_returns,
            static_violations: self.static_violations,
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
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) semantic_lookup: &'a SemanticLookup<'a>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_owned_write: &'a HashMap<SymbolId, bool>,
    pub(crate) reachable_calls: &'a HashMap<Location, usize>,
    pub(crate) symbols_by_root: &'a HashMap<SymbolId, Vec<SymbolId>>,
    pub(crate) contracted: &'a HashMap<SymbolId, ResolvedContractBinding>,
    pub(crate) rule_options: &'a RuleOptions,
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
    let late_stages_reusable = typescript_unchanged
        && late_stage_cache
            .as_deref()
            .and_then(Option::as_ref)
            .is_some_and(|cache| late_stage_inputs_match(cache, facts));
    if let Some(cache) = late_stage_cache.as_deref_mut() {
        if late_stages_reusable {
            if let Some(retained) = cache.as_mut() {
                for file in &facts.files {
                    if let Some(input) = retained.inputs.get_mut(file.path.as_str()) {
                        input.source_hash.clone_from(&file.source_hash);
                    }
                }
            }
        } else if let Some(retained) = cache.as_mut() {
            refresh_late_stage_inputs(&mut retained.inputs, facts);
            retained.local_accesses.aggregate = None;
            retained.interprocedural = None;
            retained.missing_owners = None;
            retained.compat_reference_locations = None;
        } else {
            *cache = Some(CachedLateStages {
                inputs: current_late_stage_inputs(facts),
                local_accesses: CachedLocalAccesses::default(),
                interprocedural: None,
                missing_owners: None,
                owner_files: HashMap::new(),
                compat_reference_locations: None,
            });
        }
    }
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
                build_typescript_indexes(&facts.typescript, dialect, facts.files.len() >= 256);
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
            build_typescript_indexes(&facts.typescript, dialect, facts.files.len() >= 256);
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
    let mut resolved_contracts = resolve_contract_imports(facts, contracts, entities, dialect);
    build_timings.contract_resolution = substage_started.elapsed();
    let semantic_lookup = SemanticLookup::new(facts, ast_indexes, entities, &symbol_names, dialect);
    let semantic_lookup = &semantic_lookup;
    // Source discovery does not inspect missing exports, and the static prepass
    // owns them after the two independent index passes complete.
    let mut draft = ProgramDraft {
        static_violations: std::mem::take(&mut resolved_contracts.missing_exports),
        ..ProgramDraft::default()
    };
    let mut owned_reachable_calls = None;
    let source_discovery = std::thread::scope(|scope| {
        let shared_worker_limit = analysis_worker_limit_for_lanes(2);
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
        };
        let source_discovery_handle = scope.spawn(move || {
            let _worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
            let mut timings = BuildTimings::default();
            let sources = discover_sources(
                &source_context,
                source_discovery_cache,
                typescript_unchanged,
                &mut timings,
                emit_timings,
            );
            (sources, timings)
        });
        let reachability_worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
        if let Some(cache) = reachability_cache.as_deref_mut() {
            let can_reuse = typescript_unchanged
                && cache.as_ref().is_some_and(|cached| {
                    cached.inputs.len() == facts.files.len()
                        && facts.files.iter().all(|file| {
                            cached.inputs.get(file.path.as_str()).is_some_and(
                                |(source_hash, ast)| {
                                    source_hash == &file.source_hash
                                        || same_reachability_ast(ast, &file.ast)
                                },
                            )
                        })
                });
            if can_reuse {
                let cached = cache.as_mut().expect("checked retained reachability");
                for file in &facts.files {
                    if let Some((source_hash, _)) = cached.inputs.get_mut(file.path.as_str()) {
                        source_hash.clone_from(&file.source_hash);
                    }
                    if let Some(retained_file) = cached.files.get_mut(file.path.as_str()) {
                        retained_file
                            .identity
                            .source_hash
                            .clone_from(&file.source_hash);
                    }
                }
                build_timings.reachability_reused = true;
            } else {
                let substage_started = Instant::now();
                let cached = cache.get_or_insert_with(|| CachedReachability {
                    inputs: HashMap::new(),
                    files: HashMap::new(),
                    calls: HashMap::new(),
                    multiplicity_by_path: HashMap::new(),
                    function_symbols: HashSet::new(),
                });
                let (reused_files, recomputed_files) = reachable_call_multiplicity_incremental(
                    ReachabilityInputs {
                        facts,
                        indexes: &project_indexes,
                        entities,
                        symbol_names: &symbol_names,
                        lookup: semantic_lookup,
                        typescript_unchanged,
                        typescript_delta: typescript_indexes.source_discovery_delta.as_ref(),
                    },
                    ReachabilityState {
                        files: &mut cached.files,
                        multiplicity_by_path: &mut cached.multiplicity_by_path,
                        calls: &mut cached.calls,
                        function_symbols: &mut cached.function_symbols,
                    },
                );
                build_timings.reachability = substage_started.elapsed();
                build_timings.reachability_reused_files = reused_files;
                build_timings.reachability_recomputed_files = recomputed_files;
                cached.inputs = facts
                    .files
                    .iter()
                    .map(|file| {
                        (
                            file.path.to_string(),
                            (file.source_hash.clone(), file.ast.clone()),
                        )
                    })
                    .collect();
            }
        } else {
            let substage_started = Instant::now();
            owned_reachable_calls = Some(reachable_call_multiplicity(
                facts,
                &project_indexes,
                entities,
                &symbol_names,
                semantic_lookup,
            ));
            build_timings.reachability = substage_started.elapsed();
        }
        drop(reachability_worker_limit);
        clock.finish(
            &mut build_timings,
            |timings| &mut timings.indexes_and_reachability,
            "indexes-and-reachability",
        );
        let (source_discovery, discovery_timings) = source_discovery_handle
            .join()
            .expect("parallel source discovery worker panicked");
        build_timings.absorb_source_discovery(&discovery_timings);
        source_discovery
    });
    let reachable_calls = if let Some(cache) = reachability_cache {
        &cache.as_ref().expect("reachability initialized").calls
    } else {
        owned_reachable_calls
            .as_ref()
            .expect("owned reachability initialized")
    };
    let SourceDiscovery {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        contract_reads,
        contract_callbacks,
        contract_returns,
        contracted_accessor_symbols,
        prop_sources,
        bundled_returns,
        retained_source_paths,
        changed_source_symbols,
    } = source_discovery;
    // discover_sources owns its own stage clock; restart this function's so
    // the static-prepass stage measures only the prepass loops.
    clock.restart();

    let analysis = AnalysisContext {
        facts,
        dialect,
        entities,
        symbol_names: &symbol_names,
        aliases,
        accessors: &accessors,
        prop_sources: &prop_sources,
        semantic_lookup,
        source_kinds: &source_kinds,
        source_owned_write: &source_owned_write,
        reachable_calls,
        symbols_by_root: &typescript_indexes.symbols_by_root,
        contracted: &resolved_contracts.by_symbol,
        rule_options,
    };
    static_rules::static_prepass(&analysis, &mut draft);
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.static_prepass,
        "static-prepass",
    );
    let local_access_context = LocalAccessContext {
        facts,
        lookup: semantic_lookup,
        entities,
        symbol_names: &symbol_names,
        reachable_calls,
        accessors: &accessors,
        accessor_origins: &accessor_origins,
        setters: &setters,
        actions: &actions,
        source_primitives: &source_primitives,
        async_sources: &async_sources,
        source_declarations,
        contract_reads: &contract_reads,
        contract_returns: &contract_returns,
        bundled_returns: &bundled_returns,
        source_kinds: &source_kinds,
        prop_sources: &prop_sources,
    };
    let cached_interprocedural = late_stages_reusable
        .then(|| {
            late_stage_cache
                .as_deref()
                .and_then(Option::as_ref)
                .and_then(|cache| cache.interprocedural.as_ref())
                .cloned()
        })
        .flatten();
    // Only the interprocedural pass consumes the ordered per-symbol reference
    // lists, so a warm interprocedural cache means nobody asks for them. The
    // upstream-compat surface needs the same references, but keyed by location
    // rather than by symbol, and it gets that map from its own cached
    // projection below.
    let references_by_source = if cached_interprocedural.is_some() {
        HashMap::new()
    } else {
        references_for_sources(
            &facts.typescript,
            &typescript_indexes.symbols_by_root,
            accessors.keys(),
        )
    };
    let local_access_cache = late_stage_cache
        .as_deref_mut()
        .and_then(Option::as_mut)
        .map(|cache| &mut cache.local_accesses);
    let overlap_late_stages = cached_interprocedural.is_none() && facts.files.len() >= 256;
    let interprocedural_context = InterproceduralContext {
        facts,
        project_indexes: &project_indexes,
        accessors: &accessors,
        contracted_accessor_symbols: &contracted_accessor_symbols,
        returned_source_symbols: &returned_source_symbols,
        summary_source_symbols: &summary_source_symbols,
        source_phases: &source_phases,
        source_kinds: &source_kinds,
        contract_reads: &contract_reads,
        contract_callbacks: &contract_callbacks,
        contract_returns: &contract_returns,
        bundled_returns: &bundled_returns,
        source_primitives: &source_primitives,
        entities,
        references_by_source: &references_by_source,
        symbol_names: &symbol_names,
        changed_semantic_symbols: typescript_indexes
            .source_discovery_delta
            .as_ref()
            .map(|delta| &delta.semantic_symbol_ids),
        retained_source_paths: &retained_source_paths,
        lookup: semantic_lookup,
    };
    let run_local_access = || {
        local_access_context.build(
            local_access_cache,
            LocalAccessReuse {
                aggregate_reusable: late_stages_reusable,
                typescript_unchanged,
                source_discovery_delta: typescript_indexes.source_discovery_delta.as_ref(),
                changed_source_symbols: &changed_source_symbols,
                retained_source_paths: &retained_source_paths,
                global_async_context_unchanged: late_stages_reusable,
            },
        )
    };
    let (local_access, interprocedural, local_access_elapsed, interprocedural_elapsed, reused) =
        std::thread::scope(|scope| {
            if let Some(mut cached) = cached_interprocedural {
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                cached.timings = InterproceduralTimings::default();
                return (local_access, cached, local_elapsed, Duration::ZERO, true);
            }
            if overlap_late_stages {
                let shared_worker_limit = analysis_worker_limit_for_lanes(2);
                let interprocedural = scope.spawn(move || {
                    let _worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
                    let started = Instant::now();
                    let result = interprocedural_context.build(
                        typed_accessor_cache,
                        interprocedural_graph_cache,
                        interprocedural_result_cache,
                    );
                    (result, started.elapsed())
                });
                let local_worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                drop(local_worker_limit);
                let (interprocedural, interprocedural_elapsed) = interprocedural
                    .join()
                    .expect("parallel interprocedural analysis worker panicked");
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    interprocedural_elapsed,
                    false,
                )
            } else {
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                let interprocedural_started = Instant::now();
                let interprocedural = interprocedural_context.build(
                    typed_accessor_cache,
                    interprocedural_graph_cache,
                    interprocedural_result_cache,
                );
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    interprocedural_started.elapsed(),
                    false,
                )
            }
        });
    build_timings.local_reads_and_writes = local_access_elapsed;
    build_timings.interprocedural_summaries = interprocedural_elapsed;
    build_timings.interprocedural_reused = reused;
    let local_and_interprocedural_elapsed = clock.elapsed();
    build_timings.local_and_interprocedural = local_and_interprocedural_elapsed;
    clock.record("local-reads-and-writes", local_access_elapsed);
    clock.record("interprocedural-summaries", interprocedural_elapsed);
    clock.record(
        "local-and-interprocedural",
        local_and_interprocedural_elapsed,
    );
    clock.restart();
    if !reused && let Some(cache) = late_stage_cache.as_deref_mut().and_then(Option::as_mut) {
        cache.interprocedural = Some(interprocedural.clone());
    }
    build_timings.local_accesses_reused = local_access.reused;
    build_timings.local_access_reused_files = local_access.reused_files;
    build_timings.local_access_recomputed_files = local_access.recomputed_files;
    let LocalAccessResult {
        reads,
        writes,
        action_invocations,
        async_reads,
        strict_read_obligations,
        write_action_obligations,
    } = local_access.result;
    draft.reads = reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect::<Vec<_>>();
    draft.writes = writes
        .into_iter()
        .map(|write| (*write).clone())
        .collect::<Vec<_>>();
    draft.action_invocations = action_invocations
        .into_iter()
        .map(|action| (*action).clone())
        .collect::<Vec<_>>();
    draft.async_reads = async_reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect::<Vec<_>>();
    draft.strict_read_obligations = strict_read_obligations;
    draft.write_action_obligations = write_action_obligations;
    build_timings.absorb_interprocedural(&interprocedural.timings);
    draft.strict_read_obligations += interprocedural.reads.len();
    draft.reads.extend(interprocedural.reads.iter().cloned());
    static_rules::component_returns_conditionally(&analysis, &mut draft);
    draft.contract_exports = interprocedural.exports.clone();
    draft.contract_generation_obligations =
        interprocedural.contract_generation_obligations.to_vec();
    upstream_compat::check_project(
        &analysis,
        late_stage_cache
            .as_deref_mut()
            .and_then(Option::as_mut)
            .map(|cache| &mut cache.compat_reference_locations),
        late_stages_reusable,
        &mut draft,
    );
    draft.leaf_operations.extend(
        parallel_file_results(&facts.files, |file| {
            leaf_owner_operations_for_file(file, &symbol_names, semantic_lookup)
        })
        .into_iter()
        .flatten(),
    );
    for (invalid, unresolved) in parallel_file_results(&facts.files, |file| {
        cleanup_returns_for_file(semantic_lookup, file, &symbol_names)
    }) {
        draft.invalid_cleanup_returns.extend(invalid);
        draft.unresolved_cleanup_returns.extend(unresolved);
    }
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.leaf_and_cleanup,
        "leaf-and-cleanup",
    );
    let static_api = StaticApiContext {
        lookup: analysis.semantic_lookup,
        entities: analysis.entities,
        symbol_names: analysis.symbol_names,
        source_kinds: analysis.source_kinds,
        source_owned_write: analysis.source_owned_write,
        accessors: analysis.accessors,
        reachable_calls: analysis.reachable_calls,
    };
    for result in parallel_file_results(&facts.files, |file| static_api.check_file(file)) {
        draft.static_violations.extend(result.violations);
        draft.writes.extend(result.writes);
        draft
            .write_action_obligations
            .extend(result.write_action_obligations);
    }
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.static_api,
        "static-api",
    );
    directives::discover_directive_creations(&analysis, &mut draft);
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.directives,
        "directives",
    );
    let cached_missing_owners = late_stages_reusable
        .then(|| {
            late_stage_cache
                .as_deref()
                .and_then(Option::as_ref)
                .and_then(|cache| cache.missing_owners.as_ref())
                .cloned()
        })
        .flatten();
    if let Some(cached) = cached_missing_owners {
        draft.missing_owners = cached;
        build_timings.owner_fixed_point_reused = true;
        build_timings.owner_reused_files = u64::try_from(facts.files.len()).unwrap_or(u64::MAX);
    } else {
        if let Some(cache) = late_stage_cache.and_then(Option::as_mut) {
            let (requirements, timings) = find_missing_owners_incremental(
                facts,
                semantic_lookup,
                &project_indexes,
                &symbol_names,
                &retained_source_paths,
                &mut cache.owner_files,
            );
            draft.missing_owners.extend(requirements);
            build_timings.absorb_owner(&timings);
            cache.missing_owners = Some(draft.missing_owners.clone());
        } else {
            draft.missing_owners.extend(find_missing_owners(
                facts,
                semantic_lookup,
                &project_indexes,
                &symbol_names,
            ));
            build_timings.owner_recomputed_files =
                u64::try_from(facts.files.len()).unwrap_or(u64::MAX);
        }
    }
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.owner_fixed_point,
        "owner-fixed-point",
    );
    let program = draft.into_program(interprocedural.factory_instances);
    clock.finish(
        &mut build_timings,
        |timings| &mut timings.final_ordering,
        "final-ordering",
    );
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
