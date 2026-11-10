//! Build entry points and the staged incremental pipeline that
//! assembles a `Program`, plus the bounded-parallelism helpers.

use crate::{HashMap, HashSet};
use std::{cell::Cell, sync::Arc, time::Instant};

use crate::boundary_defects::discover_boundary_defects;
use crate::cache::{BuildCaches, build_typescript_indexes};
use crate::hosts::ProjectHostIndex;
use crate::identity::{SymbolId, SymbolName};
use crate::indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use crate::package_boundary::PackageBoundary;
use crate::reachability::{ReachabilityInputs, reachability_stage};
use crate::source_discovery::{StageContext, discover_sources};
use crate::symbols::{add_solid_import_names, patch_typescript_indexes};
use crate::timings::{ReactiveIrStage, StageClock};
use crate::{
    ActionInvocation, AsyncRead, BuildError, BuildTimings, LeafOwnerOperation, ObligationCounts,
    OwnerRequirement, PrimitiveCreation, Program, ReactiveRead, ReactiveSourceKind, ReactiveWrite,
    RuleOptions, StaticDefect, StaticViolation, location_order,
};
use crate::{
    cleanup, directives, owners, reactive_analysis, server_rules, static_api, static_rules,
};
use sha2::{Digest as _, Sha256};
use solid_dialect::Dialect;
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts};
use typefacts::Location;

/// A filed row's exact span key ([`ProgramDraft::push_defect`]).
pub(crate) type SpanKey = (&'static str, Arc<str>, u64, u64);

/// The program under assembly: every table and obligation counter the
/// pipeline's stages fill, owned in one place so a stage's writes are part
/// of its signature rather than ambient mutation of a shared function body.
#[derive(Default)]
pub(crate) struct ProgramDraft {
    pub(crate) source_call_targets:
        Arc<HashMap<Location, crate::interproc::accessor_provenance::Callable>>,
    pub(crate) apply_compute_reads: HashSet<Location>,
    pub(crate) development_feedback: Vec<crate::development_feedback::DevelopmentFile>,
    pub(crate) reads: Vec<ReactiveRead>,
    pub(crate) writes: Vec<ReactiveWrite>,
    pub(crate) action_invocations: Vec<ActionInvocation>,
    pub(crate) async_reads: Vec<AsyncRead>,
    pub(crate) static_violations: Vec<StaticViolation>,
    pub(crate) static_defects: Vec<StaticDefect>,
    pub(crate) leaf_operations: Vec<LeafOwnerOperation>,
    pub(crate) directive_creations: Vec<PrimitiveCreation>,
    pub(crate) undecided_component_regions: Vec<crate::UndecidedRegion>,
    pub(crate) event_dispatch_sites: Vec<crate::EventDispatchSite>,
    pub(crate) missing_owners: Vec<OwnerRequirement>,
    /// ADR 0949: the owner graph's dense view, kept for the package entry table
    /// (built over the final rows, `owners::package_entry_table`).
    pub(crate) owner_view: Option<crate::owners::OwnerView>,
    /// ADR 0936: the silences the producers named for sites they gave no row,
    /// filed beside the rows of the same results.
    pub(crate) site_ends: crate::site_accounting::SiteEnds,
    /// ADR 0883: JSX SC6002 places on an apply's stack, and whether its proof
    /// answers for every run of the site.
    pub(crate) jsx_phase_sites: Vec<(Location, bool)>,
    pub(crate) strict_read_obligations: usize,
    pub(crate) write_action_obligations: HashSet<(&'static str, String, u64, u64)>,
    /// One static diagnostic per (rule, file, offset) identity, across the
    /// stages that share an identity space.
    pub(crate) seen_diagnostics: HashSet<(&'static str, Arc<str>, u64)>,
    /// ADR 0860: obligations no other producer's row may yield to: pushed
    /// after every producer has run, so a row at the same start keeps its
    /// place.
    pub(crate) deferred_defects: Vec<StaticDefect>,
    /// ADR 0874: the exact spans filed, so an uncertain row at a use nested
    /// in a withheld call's start (`t.slice(0, i).trim()` in `t.slice`)
    /// keeps its row.
    seen_spans: HashSet<(&'static str, Arc<str>, u64, u64)>,
    /// ADR 0909: while a speculative walk files rows, each `seen_spans`
    /// answer that decided whether a row was kept, with the answer.
    span_log: Option<Vec<(SpanKey, bool)>>,
    /// ADR 0874: where a call the impostor census withholds starts, by path.
    /// Filled once, before any stage; shared with every speculative draft.
    pub(crate) withheld_starts: Arc<HashSet<(Arc<str>, u64)>>,
    /// ADR 0874: each withheld call's callee span, by path. A row there is
    /// about the call's target, never a Get of the value on its chain.
    pub(crate) withheld_callees: HashSet<(Arc<str>, u64, u64)>,
    /// ADR 0840: per file (in `facts.files` order), the positions of the
    /// calls the impostor census withholds, asked once per call.
    pub(crate) withheld_calls: Vec<Vec<usize>>,
}

impl ProgramDraft {
    /// Adds a version-neutral static defect once per finding family, path,
    /// and offset. Several passes can discover the same fact through
    /// different semantic routes, so the draft owns deduplication for all of
    /// them.
    ///
    /// The identity is derived from the defect kind's family rather than
    /// passed in, so a caller cannot file a defect under another finding
    /// kind's identity and silently suppress it.
    /// [`Self::push_defect`], where a proven row replaces an obligation at the
    /// same site (ADR 0871): another walk of the function proved what this one
    /// could not.
    /// ADR 0909: an empty draft for a speculative walk: it files rows as
    /// this draft would after taking its rows and seen starts, minus the
    /// spans this one has seen, whose answers it logs ([`Self::spans_agree`]).
    pub(crate) fn speculative(&self) -> Self {
        Self {
            withheld_starts: Arc::clone(&self.withheld_starts),
            span_log: Some(Vec::new()),
            ..Self::default()
        }
    }

    /// Whether a speculative walk's span answers are this draft's: each span
    /// it found unseen is unseen here too (one it had seen, it had filed
    /// itself, so it is seen here as well once its spans are added).
    pub(crate) fn spans_agree(&self, log: &[(SpanKey, bool)]) -> bool {
        log.iter()
            .all(|(key, unseen)| !*unseen || !self.seen_spans.contains(key))
    }

    /// A speculative walk's spans and span log, taken out of its draft.
    pub(crate) fn take_spans(&mut self) -> (HashSet<SpanKey>, Vec<(SpanKey, bool)>) {
        (
            std::mem::take(&mut self.seen_spans),
            self.span_log.take().unwrap_or_default(),
        )
    }

    /// Adds the spans a walk filed, as filing them here would have.
    pub(crate) fn add_seen_spans(&mut self, spans: HashSet<SpanKey>) {
        self.seen_spans.extend(spans);
    }

    pub(crate) fn push_defect_or_strengthen(&mut self, defect: StaticDefect) {
        if !defect.uncertain
            && let Some(row) = self.static_defects.iter_mut().find(|row| {
                row.uncertain
                    && row.kind.dedup_identity() == defect.kind.dedup_identity()
                    && row.location.path == defect.location.path
                    && row.location.start_byte == defect.location.start_byte
            })
        {
            *row = defect;
            return;
        }
        self.push_defect(defect);
    }

    pub(crate) fn push_defect(&mut self, defect: StaticDefect) {
        // ADR 0874: a withheld call's row (`props.items.map`) shares no slot
        // with a row at a use it starts at (`props`).
        let identity = if defect
            .analysis_context
            .split(':')
            .next()
            .is_some_and(|head| head == crate::blockers::STANDARD_MEMBER_IMPOSTOR)
        {
            crate::blockers::STANDARD_MEMBER_IMPOSTOR
        } else {
            defect.kind.dedup_identity()
        };
        let first_at_start = self.seen_diagnostics.insert((
            identity,
            defect.location.path.clone(),
            defect.location.start_byte,
        ));
        let first_at_span = self.seen_spans.insert((
            identity,
            defect.location.path.clone(),
            defect.location.start_byte,
            defect.location.end_byte,
        ));
        // A violation keeps one row per start. At a withheld call's start,
        // an uncertain row names its own use (the call's row stands beside
        // the use it starts), so only its exact span deduplicates it.
        let at_withheld = !first_at_start
            && defect.uncertain
            && self
                .withheld_starts
                .contains(&(defect.location.path.clone(), defect.location.start_byte));
        if at_withheld && let Some(log) = &mut self.span_log {
            log.push((
                (
                    identity,
                    defect.location.path.clone(),
                    defect.location.start_byte,
                    defect.location.end_byte,
                ),
                first_at_span,
            ));
        }
        if first_at_start || (at_withheld && first_at_span) {
            self.static_defects.push(defect);
        }
    }

    /// Removes source-derived diagnostics whose primary operation the
    /// compiler proved it deleted.
    ///
    /// These tables are intentionally filtered together at the last common
    /// pipeline seam. Their producers range from API-shape checks through the
    /// upstream-compat pass and directive discovery, and none should have to
    /// duplicate compiler-region policy. A discarded region is stronger than
    /// a merely untracked one: there is no call, handler, mutation, directive
    /// application, or callback obligation in the emitted program to report.
    pub(crate) fn discard_deleted_static_diagnostics(&mut self, facts: &ProjectFacts) {
        let files = crate::base::reads::file_slots(facts)
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect::<HashMap<_, _>>();
        let retained = |location: &Location| {
            let (Ok(start), Ok(end)) = (
                u32::try_from(location.start_byte),
                u32::try_from(location.end_byte),
            ) else {
                return true;
            };
            !files.get(location.path.as_ref()).is_some_and(|file| {
                crate::execution_role::discarded_region_contains(file, Span::new(start, end))
            })
        };

        // ADR 0933: a row the compiler's discard removes leaves a typed silence.
        macro_rules! discard {
            ($table:ident, $name:literal) => {
                let stage =
                    crate::proof_log::RowStage::begin($name, &self.$table, |row| &row.location);
                self.$table.retain(|row| retained(&row.location));
                if let Some(stage) = stage {
                    stage.end(
                        "CompilerDiscardedRegion",
                        &[crate::proof_log::PremiseKind::CompilerDiscardedRegion],
                        &self.$table,
                        |row| &row.location,
                    );
                }
            };
        }
        discard!(static_defects, "static_defects");
        discard!(static_violations, "static_violations");
        discard!(directive_creations, "directive_creations");
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
        Program {
            apply_compute_reads: self.apply_compute_reads,
            runtime_configuration: crate::RuntimeConfigurationPremise::Assumed,
            development_feedback: self.development_feedback,
            execution_paths: crate::ExecutionPaths::default(),
            reads: self.reads,
            writes: self.writes,
            actions: self.action_invocations,
            leaf_operations: self.leaf_operations,
            static_violations: self.static_violations,
            static_defects: self.static_defects,
            directive_creations: self.directive_creations,
            missing_owners: self.missing_owners,
            async_reads: self.async_reads,
            undecided_component_regions: self.undecided_component_regions,
            event_dispatch_sites: self.event_dispatch_sites,
            unreached_sites: Vec::new(),
            early_mention_sites: Vec::new(),
            blocked_by: Vec::new(),
            hazards: Vec::new(),
            package_entries: Vec::new(),
            package_references: Vec::new(),
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
    /// ADR 0294: typed candidates, never proven sources.
    pub(crate) typed_sources: &'a HashMap<SymbolId, crate::source_discovery::TypedSource>,
    pub(crate) setters: &'a HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    >,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) uncertain_prop_sources: &'a HashSet<SymbolId>,
    pub(crate) props_reactivity: &'a crate::source_discovery::PropsReactivityIndex,
    pub(crate) semantic_lookup: &'a SemanticLookup<'a>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) async_sources: &'a HashSet<SymbolId>,
    pub(crate) source_owned_write: &'a HashMap<SymbolId, crate::OwnedWriteOption>,
    pub(crate) value_form_stores: &'a HashSet<SymbolId>,
    pub(crate) reachable_calls: &'a HashMap<Location, u64>,
    pub(crate) symbols_by_root: &'a HashMap<SymbolId, Vec<SymbolId>>,
    pub(crate) rule_options: &'a RuleOptions,
    /// The files whose source and TypeFacts rows are the last build's
    /// (source discovery's): a per-file chunk of such a file may be kept
    /// ([`crate::late_chunks`]).
    pub(crate) unchanged_paths: &'a HashSet<String>,
    /// Where each reference of an accessor source lies
    /// ([`crate::symbols::source_reference_locations`]): built once per
    /// build for both compatibility contexts (the census's member writes and
    /// the compatibility pass).
    pub(crate) source_reference_locations:
        std::sync::OnceLock<std::sync::Arc<crate::cache::SourceReferenceLocations>>,
}

/// What one analysis is given about the code it cannot read: the package
/// boundary, every import the analysis cannot answer from source, each with
/// the cause recorded at the decision that found it.
///
/// `hosts` is the inferred execution host. The IR pipeline fingerprints it
/// and never reads it.
#[derive(Clone, Copy, Debug)]
pub struct AnalysisInputs<'a> {
    pub boundary: &'a PackageBoundary,
    pub hosts: Option<&'a ProjectHostIndex>,
}

impl<'a> AnalysisInputs<'a> {
    /// No recorded boundary and no host: every caller that has not been given
    /// one.
    #[must_use]
    pub fn empty() -> Self {
        static EMPTY: std::sync::OnceLock<PackageBoundary> = std::sync::OnceLock::new();
        Self {
            boundary: EMPTY.get_or_init(PackageBoundary::default),
            hosts: None,
        }
    }

    /// These inputs with an inferred host.
    #[must_use]
    pub fn with_hosts(self, hosts: &'a ProjectHostIndex) -> Self {
        Self {
            hosts: Some(hosts),
            ..self
        }
    }

    /// The cache identity of the inputs. An empty boundary and an absent host
    /// add nothing; any recorded cause, explanation included, and every byte
    /// of a host is part of it.
    #[must_use]
    pub fn cache_fingerprints(&self) -> Vec<[u8; 32]> {
        let mut fingerprints = Vec::new();
        if !self.boundary.is_empty() {
            fingerprints.push(self.boundary.digest());
        }
        if let Some(hosts) = self.hosts {
            let mut hash = Sha256::new();
            hash.update(b"project-host-index-v1");
            // Deterministic BTree collections; include config, integration,
            // source inventory, roots and the final conservative join.
            hash.update(serde_json::to_vec(hosts).expect("host index is serializable"));
            fingerprints.push(hash.finalize().into());
        }
        fingerprints
    }
}

pub fn build(facts: &ProjectFacts, dialect: &dyn Dialect) -> Result<Program, BuildError> {
    build_with_analysis_inputs_measured(
        facts,
        dialect,
        AnalysisInputs::empty(),
        &RuleOptions::default(),
    )
    .map(|(program, _)| program)
}

/// The staged incremental pipeline. One stage per clock span, in order:
///
/// 1. Project indexes, the late-stage cache gate, TypeScript indexes,
///    symbol names, boundary defects, and the semantic lookup.
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
/// is handed to exactly one stage.
pub fn build_with_analysis_inputs_measured(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    inputs: AnalysisInputs<'_>,
    rule_options: &RuleOptions,
) -> Result<(Program, BuildTimings), BuildError> {
    build_with_analysis_inputs_measured_incremental(
        facts,
        dialect,
        inputs,
        rule_options,
        BuildCaches::default(),
        crate::runtime_configuration::scan(facts, dialect),
    )
}

// Threads only with a second analysis worker; the `available_analysis_workers() <= 1` arm runs inline (wasm32-wasip1).
#[allow(clippy::disallowed_methods)]
pub(crate) fn build_with_analysis_inputs_measured_incremental(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    inputs: AnalysisInputs<'_>,
    rule_options: &RuleOptions,
    caches: BuildCaches<'_>,
    runtime_configuration: crate::RuntimeConfigurationPremise,
) -> Result<(Program, BuildTimings), BuildError> {
    let BuildCaches {
        ast_indexes: ast_indexes_cache,
        source_discovery: source_discovery_cache,
        interprocedural_graph: interprocedural_graph_cache,
        interprocedural_results: interprocedural_result_cache,
        typescript_indexes: typescript_indexes_cache,
        reachability: mut reachability_cache,
        late_stages: mut late_stage_cache,
        base_check,
        memos,
        defer_memo_end,
    } = caches;
    let emit_timings = std::env::var_os("SOLID_CHECKER_TIMINGS").is_some();
    let total_started = Instant::now();
    let mut clock = StageClock::new(emit_timings);
    let mut build_timings = BuildTimings::default();
    let substage_started = Instant::now();
    let owned_ast_indexes;
    let ast_indexes = if let Some(cache) = ast_indexes_cache {
        let current_paths = crate::base::reads::file_slots(facts)
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        cache.retain(|path, _| current_paths.contains(path.as_str()));
        for file in crate::base::reads::file_slots(facts) {
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
        owned_ast_indexes = crate::base::reads::file_slots(facts)
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        &owned_ast_indexes
    };
    let project_indexes = ProjectIndexes::new(facts, ast_indexes);
    build_timings.project_indexes = substage_started.elapsed();
    let typescript_unchanged = crate::cache::typescript_unchanged(facts);
    let owned_typescript_indexes;
    let typescript_indexes = if let Some(cache) = typescript_indexes_cache {
        // The patch re-roots aliases without the runtime redirects the
        // rebuild applies after TypeScript's alias closure: with any, rebuild.
        let patch_timings = (!typescript_unchanged && facts.runtime_symbol_redirects.is_empty())
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
        build_timings.typescript_indexes_patched = indexes_patched;
        if let Some((alias_roots, entity_symbols)) = patch_timings {
            build_timings.alias_roots = alias_roots;
            build_timings.entity_symbols = entity_symbols;
            build_timings.alias_and_entity_indexes = alias_roots + entity_symbols;
        }
        if (!typescript_unchanged && !indexes_patched) || cache.is_none() {
            let substage_started = Instant::now();
            let (indexes, alias_roots, entity_symbols) = build_typescript_indexes(
                &facts.typescript,
                dialect,
                crate::base::reads::file_slots(facts).len(),
                &facts.runtime_symbol_redirects,
            );
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
        let (indexes, alias_roots, entity_symbols) = build_typescript_indexes(
            &facts.typescript,
            dialect,
            crate::base::reads::file_slots(facts).len(),
            &facts.runtime_symbol_redirects,
        );
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
    // An import the analysis cannot answer from source is the package
    // boundary's question.
    let boundary_defects = discover_boundary_defects(facts, inputs, entities, dialect);
    build_timings.boundary_defects = substage_started.elapsed();
    let semantic_lookup = SemanticLookup::new(
        facts,
        ast_indexes,
        entities,
        &symbol_names,
        dialect,
        rule_options.runtime.program_is_closed(),
    )
    .with_router_install(rule_options.runtime.router_install.clone())
    .with_package_instances(rule_options.runtime.package_instances.clone());
    let semantic_lookup = match memos {
        Some(store) => semantic_lookup.with_memo_store(store),
        None => semantic_lookup,
    };
    // The memo store begins outside every recording, before the census below
    // reads a memo (ADR 0392's ordering); its cost is `memo_begin`'s, not the
    // census's.
    crate::base::reads::suppress(|| {
        let _ = semantic_lookup.memo_world();
    });
    // ADR 0347: which exports a namespace import hands out, read once per build.
    let substage_started = Instant::now();
    let (hand_outs, hand_out_reads) = crate::base::reads::record(|| {
        crate::attribution::namespace_hand_outs(
            &semantic_lookup,
            aliases,
            &typescript_indexes.symbols_by_root,
        )
    });
    // A whole-program census: what it was handed of each file is no reader's
    // own read (`indexes::hand_outs`).
    semantic_lookup.set_namespace_hand_outs(
        hand_outs,
        hand_out_reads.without_tables(crate::indexes::HAND_OUT_TABLES),
    );
    build_timings.namespace_hand_outs = substage_started.elapsed();
    let semantic_lookup = &semantic_lookup;
    if let Some((retained, mode)) = base_check {
        check_retained_base(
            retained,
            mode,
            semantic_lookup,
            &mut build_timings,
            emit_timings,
        );
    }
    let substage_started = Instant::now();
    // The memos begin before any stage reads one (ADR 0392's ordering).
    semantic_lookup.begin_memos();
    build_timings.memo_warm = substage_started.elapsed();
    if let Some(slot) = late_stage_cache.as_deref_mut() {
        slot.get_or_insert_with(Default::default);
    }
    // ADR 0840: each file's withheld calls, asked once per call on the
    // workers (the draft's withheld starts and callees, and the census's
    // impostor rows, read them).
    let withheld_calls = parallel_balanced_results(crate::base::reads::file_slots(facts), |file| {
        file.ast
            .calls
            .iter()
            .enumerate()
            .filter(|(_, call)| semantic_lookup.impostor_withheld(file, call.callee))
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    });
    let withheld = || {
        crate::base::reads::file_slots(facts)
            .iter()
            .zip(&withheld_calls)
            .flat_map(|(file, calls)| {
                calls
                    .iter()
                    .map(move |index| (file, &file.ast.calls[*index]))
            })
    };
    // Source discovery does not inspect missing exports, and the static prepass
    // owns them after the two independent index passes complete.
    let mut draft = ProgramDraft {
        development_feedback: if rule_options.development_feedback {
            crate::development_feedback::models(facts, semantic_lookup, dialect)
        } else {
            Vec::new()
        },
        static_defects: boundary_defects,
        withheld_starts: Arc::new(
            withheld()
                .map(|(file, call)| (file.path.shared(), u64::from(call.span.start)))
                .collect(),
        ),
        withheld_callees: withheld()
            .map(|(file, call)| {
                let callee = file.ast.peel_ts_sugar_span(call.callee);
                (
                    file.path.shared(),
                    u64::from(callee.start),
                    u64::from(callee.end),
                )
            })
            .collect(),
        withheld_calls: withheld_calls.clone(),
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
        runtime: &rule_options.runtime,
    };
    let discover = move || {
        let mut timings = BuildTimings::default();
        let sources = discover_sources(
            &source_context,
            source_discovery_cache,
            typescript_unchanged,
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
        typescript_unchanged,
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
        typed_sources: &source_discovery.typed_sources,
        setters: &source_discovery.setters,
        actions: &source_discovery.actions,
        prop_sources: &source_discovery.prop_sources,
        uncertain_prop_sources: &source_discovery.uncertain_prop_sources,
        props_reactivity: &source_discovery.props_reactivity,
        semantic_lookup,
        source_kinds: &source_discovery.source_kinds,
        source_primitives: &source_discovery.source_primitives,
        async_sources: &source_discovery.async_sources,
        source_owned_write: &source_discovery.source_owned_write,
        value_form_stores: &source_discovery.value_form_stores,
        reachable_calls,
        symbols_by_root: &typescript_indexes.symbols_by_root,
        rule_options,
        unchanged_paths: &source_discovery.unchanged_paths,
        source_reference_locations: std::sync::OnceLock::new(),
    };
    // The two after-await rules, the prepass's and the one over calls the
    // interprocedural pass attributed, ask the same project-wide index: it is
    // built once per build, by whichever asks first.
    let async_computations = static_rules::SharedAsyncComputations::default();
    static_rules::static_prepass(
        &analysis,
        late_stage_cache
            .as_deref_mut()
            .and_then(Option::as_mut)
            .map(|cache| &mut cache.chunks),
        &async_computations,
        &mut draft,
    );
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
            interprocedural_graph: interprocedural_graph_cache,
            interprocedural_results: interprocedural_result_cache,
            late_stages: late_stage_cache.as_deref_mut().and_then(Option::as_mut),
        },
        typescript_unchanged,
        &mut draft,
        &mut build_timings,
        &mut clock,
    );
    let substage_started = Instant::now();
    static_rules::reactive_read_after_await_through_calls(
        &analysis,
        &async_computations,
        &mut draft,
    );
    build_timings.read_after_await_through_calls = substage_started.elapsed();
    let substage_started = Instant::now();
    cleanup::collect_project(
        &analysis,
        late_stage_cache
            .as_deref_mut()
            .and_then(Option::as_mut)
            .map(|cache| &mut cache.chunks.leaf_operations),
        &mut draft,
    );
    build_timings.leaf_operations = substage_started.elapsed();
    clock.finish(&mut build_timings, ReactiveIrStage::LeafAndCleanup);
    let substage_started = Instant::now();
    static_api::check_project(
        &analysis,
        late_stage_cache
            .as_deref_mut()
            .and_then(Option::as_mut)
            .map(|cache| &mut cache.chunks.static_api),
        &mut draft,
    );
    build_timings.static_api_files = substage_started.elapsed();
    let substage_started = Instant::now();
    server_rules::check_project(&analysis, &mut draft);
    build_timings.server_rules = substage_started.elapsed();
    clock.finish(&mut build_timings, ReactiveIrStage::StaticApi);
    directives::discover_directive_creations(&analysis, &mut draft);
    draft.undecided_component_regions = semantic_lookup.undecided_component_regions();
    draft.event_dispatch_sites = semantic_lookup.event_dispatch_reports();
    clock.finish(&mut build_timings, ReactiveIrStage::Directives);
    owners::collect_project(
        &analysis,
        &project_indexes,
        &source_discovery.unchanged_paths,
        late_stage_cache.as_deref_mut().and_then(Option::as_mut),
        &mut draft,
        &mut build_timings,
    );
    crate::stored_throw::mark_owner_requirements(&analysis, &mut draft);
    // ADR 0883: where SC6002 proves the throw on every run of a JSX site, rc.21
    // never reaches NO_OWNER_EFFECT there, as for a creating call (ADR 0483).
    // Anywhere else that code may still fire, and which run does is not proved.
    let jsx_phase_sites = std::mem::take(&mut draft.jsx_phase_sites);
    draft.missing_owners.retain_mut(|requirement| {
        if requirement.operation != crate::OwnerRequirementOperation::Effect {
            return true;
        }
        match jsx_phase_sites
            .iter()
            .find(|(location, _)| *location == requirement.location)
        {
            Some((_, true)) => false,
            Some((_, false)) => {
                requirement.uncertain = true;
                requirement.runtime_uncertain = true;
                true
            }
            None => true,
        }
    });
    clock.finish(&mut build_timings, ReactiveIrStage::OwnerFixedPoint);
    // Static and compatibility passes deliberately operate on source facts.
    // Apply the compiler's stronger "this code was not emitted" fact once,
    // after every producer has run.
    draft
        .static_defects
        .extend(crate::object_members::unresolved_handler_defects(
            facts,
            semantic_lookup,
        ));
    for defect in dependency_source_call_error_defects(facts) {
        draft.push_defect(defect);
    }
    for defect in std::mem::take(&mut draft.deferred_defects) {
        draft.push_defect(defect);
    }
    draft.discard_deleted_static_diagnostics(facts);
    crate::effect_execution::gate_diagnostics(semantic_lookup, &mut draft);
    crate::attribution::discharge_closed_program_export_dispatch(
        semantic_lookup,
        entities,
        aliases,
        &typescript_indexes.symbols_by_root,
        &mut draft.static_defects,
        &mut draft.site_ends,
    );
    let site_ends = std::mem::take(&mut draft.site_ends);
    let owner_view = draft.owner_view.take();
    let mut program = draft.into_program(factory_instances);
    program.execution_paths = crate::execution_paths::execution_paths(
        facts,
        &rule_options.runtime,
        dialect,
        semantic_lookup,
    );
    // ADR 0517: a `<Loading on>` whose hold is not proven its own; ADR 0624: one
    // handed a function.
    let holds =
        crate::loading_hold::loading_on_outside_hold_obligations(facts, &program, semantic_lookup);
    program.static_violations.extend(holds);
    // ADR 0936: every site of the universe ends in a row, a census obligation,
    // the compiler's discard or a silence its producer named, or gets an
    // uncertifiable row; before the reach queries over the rows below.
    crate::site_accounting::account(&analysis, &mut program, &site_ends);
    // ADR 0458: after every producer and gate, over the rows that may be
    // reported, while the lookup's memos are still open.
    program.unreached_sites = crate::execution_role::unreached_report_sites(
        &program,
        semantic_lookup,
        late_stage_cache
            .and_then(Option::as_mut)
            .map(|cache| &mut cache.unreached),
        &source_discovery.unchanged_paths,
    );
    program.early_mention_sites =
        crate::indexes::early_mention_report_sites(&program, semantic_lookup);
    // ADR 0949: over the final rows, the project sites that may run each package function that
    // holds one (the ADR 0284 drop reports a dropped finding there), and the references no
    // exact call or tag decides.
    if let Some(view) = &owner_view {
        (program.package_entries, program.package_references) =
            crate::owners::package_entry_table(facts, semantic_lookup, view, &program);
    }
    // ADR 0947: the typed blocker of each row a veto formula holds, over the final rows.
    (program.blocked_by, program.hazards) =
        crate::blocked_by::typed_blockers(&program, semantic_lookup);
    if defer_memo_end {
        semantic_lookup.defer_end_memos();
    } else if let Some(report) = semantic_lookup.end_memos() {
        report_memos(&report, &mut build_timings, emit_timings);
    }
    program.runtime_configuration = runtime_configuration;
    crate::role_log::flush();
    crate::proof_log::flush();
    clock.finish(&mut build_timings, ReactiveIrStage::FinalOrdering);
    build_timings.closure_placements = semantic_lookup.closure_placements();
    build_timings.write_walk_steps = semantic_lookup.write_walk_steps();
    build_timings.reach_evaluations = semantic_lookup.reach_evaluations();
    (
        build_timings.bounded_walk_peak,
        build_timings.bounded_walks_exhausted,
    ) = semantic_lookup.bounded_walks();
    if emit_timings {
        // The walks' work (ADR 0546, ADR 0560): what the complexity ladder
        // gates on, per build.
        eprintln!(
            "{}",
            serde_json::json!({
                "reactiveIrWalkWork": {
                    "writeWalkSteps": build_timings.write_walk_steps,
                    "reachEvaluations": build_timings.reach_evaluations,
                    "boundedWalkPeak": build_timings.bounded_walk_peak,
                    "boundedWalksExhausted": build_timings.bounded_walks_exhausted,
                }
            })
        );
    }
    build_timings.total = total_started.elapsed();
    Ok((program, build_timings))
}

/// Review of ADR 0368: a project call the dependency-source program answers
/// as a call error (`Recovery`) and the project's own program does not
/// (`ProjectFacts::dependency_source_call_errors`). Every pass leaves a
/// `Recovery` call to TypeScript, which here reports nothing: the call stays
/// an uncertifiable dispatch obligation (SC9012), never a silent skip.
fn dependency_source_call_error_defects(facts: &ProjectFacts) -> Vec<StaticDefect> {
    facts
        .dependency_source_call_errors
        .iter()
        .map(|location| {
            let span = match (
                u32::try_from(location.start_byte),
                u32::try_from(location.end_byte),
            ) {
                (Ok(start), Ok(end)) => Some(Span::new(start, end)),
                _ => None,
            };
            let file = crate::base::reads::file_slots(facts)
                .iter()
                .find(|file| file.path.as_str() == location.path.as_ref());
            let call = file.zip(span).and_then(|(file, span)| {
                file.ast
                    .calls
                    .iter()
                    .find(|call| call.span == span || call.callee == span)
                    .map(|call| (file, call))
            });
            let (callee, location) = match call {
                Some((file, call)) => (
                    call.static_callee(&file.source)
                        .map_or_else(|| "dynamic call".to_owned(), str::to_owned),
                    crate::location(file.path.shared(), call.callee),
                ),
                None => ("dynamic call".to_owned(), location.clone()),
            };
            StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee,
                    member: None,
                },
                location,
                analysis_context: crate::DEPENDENCY_SOURCE_CALL_ERROR.into(),
                fixes: vec![],
                uncertain: true,
            }
        })
        .collect()
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

/// Shadow mode: updates the previous build's base indexes with the files
/// whose views changed and compares them with this build's refold
/// ([`crate::base::retained`]). A mismatch is reported like a program
/// mismatch, and the retained base is dropped.
fn check_retained_base(
    retained: &mut Option<crate::base::retained::RetainedBase>,
    mode: crate::shadow::ShadowMode,
    lookup: &SemanticLookup<'_>,
    timings: &mut BuildTimings,
    emit_timings: bool,
) {
    let table = crate::base::retained::table(retained, lookup.file_slots());
    let views = lookup.base_views_over(&table);
    let Some(check) = crate::base::retained::check(retained, &views) else {
        return;
    };
    timings.base_index_update = check.update;
    timings.base_index_changed_files = check.changed_files as u64;
    if emit_timings {
        eprintln!(
            "{}",
            serde_json::json!({
                "reactiveIrBaseIndexUpdate": {
                    "updateNs": u64::try_from(check.update.as_nanos()).unwrap_or(u64::MAX),
                    "fingerprintNs": u64::try_from(check.fingerprint.as_nanos()).unwrap_or(u64::MAX),
                    "refoldNs": u64::try_from(check.refold.as_nanos()).unwrap_or(u64::MAX),
                    "changedFiles": check.changed_files,
                    "editedFiles": check.edited_files,
                    "editedUpdateNs": u64::try_from(check.edited_update.as_nanos()).unwrap_or(u64::MAX),
                    "removedFiles": check.removed_files,
                    "deltaKeys": check.delta_keys,
                    "mismatch": check.mismatch.is_some(),
                }
            })
        );
    }
    if let Some(mismatch) = check.mismatch {
        *retained = None;
        crate::shadow::report_message(mode, &mismatch);
    }
}

/// Records one build's memo report ([`crate::base::memo`]) in the timings,
/// emits it with them, and appends it to `SOLID_CHECKER_IR_MEMO_LOG` (a
/// daemon's stderr goes nowhere).
fn report_memos(
    report: &crate::base::memo::build::MemoReport,
    timings: &mut BuildTimings,
    emit_timings: bool,
) {
    let begin = &report.begin;
    timings.memo_changed_files = begin.changed_files as u64;
    timings.memo_begin = std::time::Duration::from_nanos(
        begin.fingerprint_ns + begin.index_ns + begin.probe_ns + begin.mark_ns,
    );
    for (name, stats) in &report.tables {
        timings.memo_reused += stats.reused + stats.verified;
        timings.memo_computed += stats.fresh + stats.unchanged + stats.changed;
        if *name == "flow_site_role" {
            timings.flow_site_role_answers = [
                stats.reused + stats.verified,
                stats.fresh + stats.unchanged + stats.changed,
                stats.changed,
            ];
        }
        if *name == "dispatch_veto" {
            timings.dispatch_veto_answers = [
                stats.reused + stats.verified,
                stats.fresh + stats.unchanged + stats.changed,
                stats.changed,
            ];
        }
        if *name == "returned_primitive_invocations" {
            timings.returned_invocation_files = [
                stats.reused + stats.verified,
                stats.fresh + stats.unchanged + stats.changed,
                stats.changed,
            ];
        }
    }
    emit_memo_report(report, emit_timings);
}

/// The report of a sweep a build deferred and the caller ran after it
/// answered ([`crate::IncrementalBuilder::finish_deferred`]): printed as the
/// build's own would have been, but with no build's timings to fill.
pub(crate) fn report_deferred_memos(report: &crate::base::memo::build::MemoReport) {
    emit_memo_report(report, std::env::var_os("SOLID_CHECKER_TIMINGS").is_some());
}

fn emit_memo_report(report: &crate::base::memo::build::MemoReport, emit_timings: bool) {
    let line = serde_json::json!({ "reactiveIrMemo": report });
    if emit_timings {
        eprintln!("{line}");
    }
    if let Some(path) = std::env::var_os("SOLID_CHECKER_IR_MEMO_LOG")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

/// The cores the analysis may use: the machine's, capped by
/// `SOLID_CHECKER_IR_WORKERS` when it is set to a positive number. The cap
/// reproduces a smaller machine's schedule (a 4-core CI runner) on a larger
/// one; it changes timing only, never an answer.
fn analysis_parallelism() -> usize {
    static CAP: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    // Read once (ADR 0593): on Linux each call reads the cgroup quota files,
    // and a check asks for every fan-out.
    static AVAILABLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let available =
        *AVAILABLE.get_or_init(|| std::thread::available_parallelism().map_or(1, usize::from));
    let cap = CAP.get_or_init(|| {
        std::env::var("SOLID_CHECKER_IR_WORKERS")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|&cap| cap > 0)
    });
    cap.map_or(available, |cap| available.min(cap))
}

pub(crate) fn available_analysis_workers() -> usize {
    let available = analysis_parallelism();
    ANALYSIS_WORKER_LIMIT.with(|limit| available.min(limit.get()))
}

pub(crate) fn analysis_worker_limit_for_lanes(lanes: usize) -> usize {
    analysis_parallelism().div_ceil(lanes.max(1))
}

// Threads only with a second worker and 256 items; otherwise a plain map (wasm32-wasip1).
#[allow(clippy::disallowed_methods)]
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
    // A computation recording its reads on this thread records what the
    // workers read too ([`crate::base::reads`]).
    let recording = WorkerContext::of_caller();
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || {
                    on_worker(recording, || chunk.iter().map(analyze).collect::<Vec<_>>())
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| {
                joined(
                    handle
                        .join()
                        .expect("parallel Reactive IR analysis worker panicked"),
                )
            })
            .collect()
    })
}

/// [`parallel_slice_results`] with a state each worker builds once (`init`)
/// for its slice: what a worker learns while checking one item (a memo's
/// answer in this build) serves its next ones. Results come back in input
/// order and do not depend on the state.
#[allow(clippy::disallowed_methods)]
pub(crate) fn parallel_slice_with_state<T, S, R, I, F>(items: &[T], init: I, analyze: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    I: Fn() -> S + Sync,
    F: Fn(&S, &T) -> R + Sync,
{
    let workers = available_analysis_workers().min(items.len());
    if workers <= 1 || items.len() < 256 {
        let state = init();
        return items.iter().map(|item| analyze(&state, item)).collect();
    }
    let chunk_size = items.len().div_ceil(workers);
    let recording = WorkerContext::of_caller();
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let (init, analyze) = (&init, &analyze);
                scope.spawn(move || {
                    on_worker(recording, || {
                        let state = init();
                        chunk
                            .iter()
                            .map(|item| analyze(&state, item))
                            .collect::<Vec<_>>()
                    })
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| {
                joined(
                    handle
                        .join()
                        .expect("parallel Reactive IR analysis worker panicked"),
                )
            })
            .collect()
    })
}

/// What a parallel helper's workers inherit from the spawning thread:
/// whether it records its reads ([`crate::base::reads`]), and whether it is
/// checking a proof ([`crate::base::memo::derived::deferring`]).
#[derive(Clone, Copy)]
struct WorkerContext {
    recording: bool,
    deferring: bool,
}

impl WorkerContext {
    fn of_caller() -> Self {
        Self {
            recording: crate::base::reads::recording(),
            deferring: crate::base::memo::derived::is_deferring(),
        }
    }
}

/// A worker's part of a parallel helper: its reads, when the spawning
/// thread records, come back with its results.
fn on_worker<R>(
    context: WorkerContext,
    work: impl FnOnce() -> R,
) -> (R, Option<crate::base::reads::Reads>) {
    crate::base::memo::derived::deferring_as(context.deferring, || {
        if context.recording {
            let (value, reads) = crate::base::reads::record(work);
            (value, Some(reads))
        } else {
            (work(), None)
        }
    })
}

/// Hands a worker's reads to the joining thread's computation.
fn joined<R>((value, reads): (R, Option<crate::base::reads::Reads>)) -> R {
    if let Some(reads) = reads {
        crate::base::reads::replay(&reads);
    }
    value
}

/// [`parallel_slice_results`] for items whose costs vary widely: each worker
/// claims the next unclaimed item, and the results come back in input order.
/// Workers keep the caller's worker limit, so analysis nested in an item
/// spreads as it would on the caller's thread. Without threads (WASM), or
/// for a short slice, the items run in order on the caller's thread.
// Threads only with a second worker and 64 items; otherwise a plain map (wasm32-wasip1).
pub(crate) fn parallel_balanced_results<T, R, F>(items: &[T], analyze: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    parallel_balanced_results_from(64, items, analyze)
}

/// [`parallel_balanced_results`] with a state each worker builds once
/// (`init`) and carries from item to item: per-build caches whose answers
/// depend on the build alone, so an item's result does not depend on which
/// worker ran it or what it ran before. Without threads, or for a short
/// slice, one state runs the items in order on the caller's thread.
#[allow(clippy::disallowed_methods)]
pub(crate) fn parallel_balanced_with_state<T, S, R, I, F>(
    items: &[T],
    init: I,
    analyze: F,
) -> Vec<R>
where
    T: Sync,
    R: Send,
    I: Fn() -> S + Sync,
    F: Fn(&mut S, &T) -> R + Sync,
{
    parallel_balanced_with_state_from(64, items, init, analyze)
}

/// [`parallel_balanced_with_state`] for items each costly enough that
/// `threshold` of them, fewer than the default 64, already pay for the
/// threads.
#[allow(clippy::disallowed_methods)]
pub(crate) fn parallel_balanced_with_state_from<T, S, R, I, F>(
    threshold: usize,
    items: &[T],
    init: I,
    analyze: F,
) -> Vec<R>
where
    T: Sync,
    R: Send,
    I: Fn() -> S + Sync,
    F: Fn(&mut S, &T) -> R + Sync,
{
    let limit = available_analysis_workers();
    let workers = limit.min(items.len());
    if workers <= 1 || items.len() < threshold.max(2) {
        let mut state = init();
        return items.iter().map(|item| analyze(&mut state, item)).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let recording = WorkerContext::of_caller();
    std::thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| {
                let (init, analyze, next) = (&init, &analyze, &next);
                scope.spawn(move || {
                    let _limit = AnalysisWorkerLimit::enter(limit);
                    on_worker(recording, || {
                        let mut state = init();
                        let mut done = Vec::new();
                        loop {
                            let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(item) = items.get(index) else {
                                break;
                            };
                            done.push((index, analyze(&mut state, item)));
                        }
                        done
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut results = (0..items.len()).map(|_| None).collect::<Vec<_>>();
        for handle in handles {
            for (index, result) in joined(
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked"),
            ) {
                results[index] = Some(result);
            }
        }
        results
            .into_iter()
            .map(|result| result.expect("every item is claimed by one worker"))
            .collect()
    })
}

/// [`parallel_balanced_results`] for items each costly enough (milliseconds)
/// that `threshold` of them, fewer than the default 64, already pay for the
/// threads.
#[allow(clippy::disallowed_methods)]
pub(crate) fn parallel_balanced_results_from<T, R, F>(
    threshold: usize,
    items: &[T],
    analyze: F,
) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let limit = available_analysis_workers();
    let workers = limit.min(items.len());
    if workers <= 1 || items.len() < threshold.max(2) {
        return items.iter().map(analyze).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let recording = WorkerContext::of_caller();
    std::thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| {
                let (analyze, next) = (&analyze, &next);
                scope.spawn(move || {
                    let _limit = AnalysisWorkerLimit::enter(limit);
                    on_worker(recording, || {
                        let mut done = Vec::new();
                        loop {
                            let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(item) = items.get(index) else {
                                break;
                            };
                            done.push((index, analyze(item)));
                        }
                        done
                    })
                })
            })
            .collect::<Vec<_>>();
        let mut results = (0..items.len()).map(|_| None).collect::<Vec<_>>();
        for handle in handles {
            for (index, result) in joined(
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked"),
            ) {
                results[index] = Some(result);
            }
        }
        results
            .into_iter()
            .map(|result| result.expect("every item is claimed by one worker"))
            .collect()
    })
}

// Threads only with a second worker and 256 files; otherwise a plain map (wasm32-wasip1).
#[allow(clippy::disallowed_methods)]
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
    let recording = WorkerContext::of_caller();
    std::thread::scope(|scope| {
        files
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || on_worker(recording, || analyze(chunk)))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                joined(
                    handle
                        .join()
                        .expect("parallel Reactive IR analysis worker panicked"),
                )
            })
            .collect()
    })
}
