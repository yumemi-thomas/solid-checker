//! The coupled local/interprocedural stage.
//!
//! Local reads and the cross-file fixed point share source-discovery inputs
//! and may run concurrently on large projects. This module owns that policy,
//! its cache slots, timing split, and the merge into `ProgramDraft`; the
//! top-level pipeline only sequences the resulting stage.

use crate::{HashMap, HashSet};
use std::time::Instant;

use solid_facts::core::SourcePath;
use typefacts::Declaration;

use crate::cache::{
    CachedInterproceduralGraph, CachedInterproceduralResults, CachedLateStages, LocalAccessResult,
    PARALLEL_INDEX_FILE_THRESHOLD, SourceDiscoveryTypeScriptDelta,
};
use crate::indexes::ProjectIndexes;
use crate::interproc::InterproceduralContext;
use crate::local_access::{LocalAccessContext, LocalAccessReuse};
use crate::pipeline::{
    AnalysisContext, AnalysisWorkerLimit, ProgramDraft, analysis_worker_limit_for_lanes,
    available_analysis_workers,
};
use crate::source_discovery::SourceDiscovery;
use crate::static_rules;
use crate::symbols::references_for_sources;
use crate::timings::{ReactiveIrStage, StageClock};
use crate::upstream_compat;
use crate::{BuildTimings, ReactiveRead};

pub(crate) struct IncrementalCaches<'a> {
    pub(crate) interprocedural_graph:
        Option<&'a mut HashMap<SourcePath, CachedInterproceduralGraph>>,
    pub(crate) interprocedural_results: Option<&'a mut CachedInterproceduralResults>,
    pub(crate) late_stages: Option<&'a mut CachedLateStages>,
}

pub(crate) struct ProjectInputs<'a, 'facts> {
    pub(crate) ctx: &'a AnalysisContext<'facts>,
    pub(crate) source: &'a SourceDiscovery,
    pub(crate) project_indexes: &'a ProjectIndexes<'facts>,
    pub(crate) source_declarations: &'a HashMap<crate::SymbolId, Declaration>,
    pub(crate) typescript_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
}

// Spawns only when `overlap` (a second worker); otherwise both lanes run inline (wasm32-wasip1).
#[allow(clippy::disallowed_methods)]
pub(crate) fn collect_project<'facts>(
    inputs: ProjectInputs<'_, 'facts>,
    mut caches: IncrementalCaches<'_>,
    typescript_unchanged: bool,
    draft: &mut ProgramDraft,
    timings: &mut BuildTimings,
    clock: &mut StageClock,
) -> usize {
    let ProjectInputs {
        ctx,
        source,
        project_indexes,
        source_declarations,
        typescript_delta,
    } = inputs;
    let local_access_context = LocalAccessContext {
        facts: ctx.facts,
        lookup: ctx.semantic_lookup,
        entities: ctx.entities,
        symbol_names: ctx.symbol_names,
        reachable_calls: ctx.reachable_calls,
        accessors: ctx.accessors,
        typed_sources: ctx.typed_sources,
        accessor_origins: &source.accessor_origins,
        setters: &source.setters,
        actions: &source.actions,
        source_primitives: &source.source_primitives,
        async_sources: &source.async_sources,
        source_async_options: &source.source_async_options,
        server_rendering: crate::source_discovery::project_server_rendering(
            ctx.facts,
            &ctx.rule_options.runtime,
            ctx.dialect,
        ),
        source_declarations,
        source_kinds: ctx.source_kinds,
        prop_sources: ctx.prop_sources,
        uncertain_prop_sources: ctx.uncertain_prop_sources,
        props_reactivity: ctx.props_reactivity,
        withdrawn_array_slots: &source.withdrawn_array_slots,
    };
    let references_by_source = references_for_sources(
        &ctx.facts.typescript,
        ctx.symbols_by_root,
        ctx.accessors.keys(),
    );
    let local_access_cache = caches
        .late_stages
        .as_deref_mut()
        .map(|cache| &mut cache.local_accesses);
    // Overlapping the two lanes only pays off on a large project, and only
    // where the host has a second worker: a wasm32-wasip1 reactor build has no
    // thread support and `Scope::spawn` panics there instead of degrading to
    // an inline call.
    let overlap = crate::base::reads::file_slots(ctx.facts).len() >= PARALLEL_INDEX_FILE_THRESHOLD
        && available_analysis_workers() > 1;
    let interprocedural_context = InterproceduralContext {
        facts: ctx.facts,
        project_indexes,
        accessors: ctx.accessors,
        returned_source_symbols: &source.returned_source_symbols,
        summary_source_symbols: &source.summary_source_symbols,
        source_phases: &source.source_phases,
        source_kinds: ctx.source_kinds,
        entities: ctx.entities,
        references_by_source: &references_by_source,
        symbol_names: ctx.symbol_names,
        unchanged_paths: &source.unchanged_paths,
        typed_sources: ctx.typed_sources,
        lookup: ctx.semantic_lookup,
    };
    let run_local_access = || {
        local_access_context.build(
            local_access_cache,
            LocalAccessReuse {
                typescript_unchanged,
                source_discovery_delta: typescript_delta,
                unchanged_paths: &source.unchanged_paths,
            },
        )
    };
    let (local_access, interprocedural, local_elapsed, interprocedural_elapsed) =
        std::thread::scope(|scope| {
            if overlap {
                let worker_limit = analysis_worker_limit_for_lanes(2);
                let interprocedural = scope.spawn(move || {
                    let _worker_limit = AnalysisWorkerLimit::enter(worker_limit);
                    let started = Instant::now();
                    let result = interprocedural_context
                        .build(caches.interprocedural_graph, caches.interprocedural_results);
                    (result, started.elapsed())
                });
                let local_worker_limit = AnalysisWorkerLimit::enter(worker_limit);
                let started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = started.elapsed();
                drop(local_worker_limit);
                let (interprocedural, interprocedural_elapsed) = interprocedural
                    .join()
                    .expect("parallel interprocedural analysis worker panicked");
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    interprocedural_elapsed,
                )
            } else {
                let started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = started.elapsed();
                let started = Instant::now();
                let interprocedural = interprocedural_context
                    .build(caches.interprocedural_graph, caches.interprocedural_results);
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    started.elapsed(),
                )
            }
        });
    let combined_elapsed = clock.elapsed();
    clock.record(timings, ReactiveIrStage::LocalReadsAndWrites, local_elapsed);
    clock.record(
        timings,
        ReactiveIrStage::InterproceduralSummaries,
        interprocedural_elapsed,
    );
    clock.record(
        timings,
        ReactiveIrStage::LocalAndInterprocedural,
        combined_elapsed,
    );
    clock.restart();
    timings.local_access_reused_files = local_access.reused_files;
    timings.local_access_recomputed_files = local_access.recomputed_files;
    let LocalAccessResult {
        reads,
        writes,
        action_invocations,
        mut async_reads,
        bounded_async_reads,
        walk_decided_reads,
        strict_read_obligations,
        write_action_obligations,
        dispatch_obligations,
        static_violations,
    } = local_access.result;
    // ADR 0835: `discover` keeps these per file; any left here are kept the
    // same way, never dropped.
    // Review R1: per rule, as `discover` keeps them.
    let placed = async_reads
        .iter()
        .filter_map(|read| {
            crate::pending_accounting::reported_rule(read).map(|rule| (read.location.clone(), rule))
        })
        .collect::<HashSet<_>>();
    async_reads.extend(bounded_async_reads.into_iter().filter(|read| {
        crate::pending_accounting::reported_rule(read)
            .is_none_or(|rule| !placed.contains(&(read.location.clone(), rule)))
    }));
    draft.reads = reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect::<Vec<ReactiveRead>>();
    draft.writes = writes.into_iter().map(|write| (*write).clone()).collect();
    draft.action_invocations = action_invocations
        .into_iter()
        .map(|action| (*action).clone())
        .collect();
    draft.static_violations.extend(static_violations);
    draft.async_reads = async_reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect();
    draft.strict_read_obligations = strict_read_obligations;
    draft.write_action_obligations = write_action_obligations;
    // Unresolved-dispatch obligations and unavailable-package-behavior
    // obligations are two different finding kinds: `SC9012` versus `SC9005`. Under one dedup identity two findings that merely start
    // at the same byte suppressed one another, and which survived depended on
    // push order. `push_defect` therefore derives the identity from the defect
    // kind's family and never from the vector it arrived in.
    let mut summaries = crate::interproc::completeness::SummaryMemo::default();
    // ADR 0802: a source no fact calls async may still be pending, by what its
    // compute reads: a value a caller handed it, a source a function form names.
    let open_computes =
        crate::local_access::OpenComputes::new(&interprocedural.source_call_targets);
    // ADR 0509: a call transport resolves exactly drops the obligation its
    // type alone, or its caller-supplied receiver, gave it.
    // A transported async source keeps it: where its read runs (a leaf
    // owner's callback, through helpers) is not the transported read's.
    let resolved_calls = interprocedural
        .source_call_targets
        .iter()
        .filter(|(_, target)| {
            !matches!(
                target,
                crate::interproc::accessor_provenance::Callable::Accessor(symbol)
                    if local_access_context.source_compute_open(symbol, &open_computes)
            ) && target_summary_complete(ctx, target, &mut summaries)
        })
        .map(|(callee, _)| callee)
        .chain(interprocedural.write_call_targets.keys())
        .collect::<HashSet<_>>();
    // A handed-member obligation is the last resort at its site: pushed after
    // every other producer's, it yields to a cause already recorded there.
    let mut handed = Vec::new();
    for mut obligation in dispatch_obligations {
        // ADR 0802: held by local access, which could not ask.
        if obligation.analysis_context == crate::local_access::HELD_OPEN_SOURCE {
            let holds = analyzed_file(ctx, obligation.location.path.as_ref()).is_some_and(|file| {
                local_access_context.value_holds_open_source(
                    file,
                    solid_facts::core::Span::new(
                        u32::try_from(obligation.location.start_byte).unwrap_or(0),
                        u32::try_from(obligation.location.end_byte).unwrap_or(0),
                    ),
                    &open_computes,
                )
            });
            if !holds {
                continue;
            }
            crate::local_access::UNRESOLVED_HANDED_READ
                .clone_into(&mut obligation.analysis_context);
        }
        if obligation.analysis_context == crate::local_access::UNRESOLVED_HANDED_MEMBER
            || obligation.analysis_context == crate::local_access::UNRESOLVED_HANDED_READ
        {
            if !resolved_calls.contains(&obligation.location) {
                handed.push(obligation);
            }
            continue;
        }
        // The type-only obligation yields only to a write or an action
        // transport now places; a transported read keeps it, as before.
        if obligation.analysis_context == crate::TYPE_ONLY_SOURCE
            && interprocedural
                .write_call_targets
                .contains_key(&obligation.location)
        {
            continue;
        }
        draft.push_defect(obligation);
    }
    // A call of a setter or action transport carried here (a hook's returned
    // tuple or object, a context value) writes or acts with its source's row;
    // the row its callee's type alone gave it is replaced.
    let transported = local_access_context.transported_writes(
        &interprocedural.write_call_targets,
        &interprocedural.source_call_targets,
    );
    draft
        .async_reads
        .extend(transported.async_reads.iter().map(|read| (**read).clone()));
    if !transported.writes.is_empty() || !transported.action_invocations.is_empty() {
        let replaced = transported
            .writes
            .iter()
            .map(|write| write.location.clone())
            .collect::<HashSet<_>>();
        draft
            .writes
            .retain(|write| !replaced.contains(&write.location));
        draft
            .writes
            .extend(transported.writes.iter().map(|write| (**write).clone()));
        draft.action_invocations.extend(
            transported
                .action_invocations
                .iter()
                .map(|action| (**action).clone()),
        );
    }
    draft
        .write_action_obligations
        .extend(transported.write_action_obligations);
    // ADR 0712: SC2009's transported obligations, where the file's own pass
    // said nothing at that call.
    for violation in transported.static_violations {
        if !draft
            .static_violations
            .iter()
            .any(|known| known.id == violation.id && known.location == violation.location)
        {
            draft.static_violations.push(violation);
        }
    }
    handed.extend(transported.dispatch_obligations);
    // F0: a standard iteration over a transported array runs its members, and
    // no obligation of its own stood at that call to keep.
    let mut open_members = interprocedural
        .source_call_targets
        .iter()
        .filter(|(_, target)| {
            matches!(
                target,
                crate::interproc::accessor_provenance::Callable::ArrayMembers(_)
            ) && !target_summary_complete(ctx, target, &mut summaries)
        })
        .map(|(callee, _)| callee)
        .collect::<Vec<_>>();
    open_members
        .sort_unstable_by_key(|callee| (callee.path.clone(), callee.start_byte, callee.end_byte));
    let open_members = open_members
        .into_iter()
        .filter_map(|callee| Some((analyzed_file(ctx, callee.path.as_ref())?, callee)))
        .collect::<Vec<_>>();
    handed.extend(local_access_context.open_array_member_obligations(&open_members));
    timings.absorb_interprocedural(&interprocedural.timings);
    draft.source_call_targets = interprocedural.source_call_targets.clone();
    draft.strict_read_obligations += interprocedural.reads.len();
    draft.reads.extend(interprocedural.reads.iter().cloned());
    draft.apply_compute_reads = crate::execution_role::apply_compute_reads(
        ctx.facts,
        &draft.reads,
        ctx.semantic_lookup,
        ctx.accessors,
        &source.accessor_origins,
    );
    // Recompute after both cache merges. A consumer in another file can change
    // while the literal's local read row is reused; a cached true is never proof.
    let mut consumers = crate::execution_role::ReadConsumerSummaries::new(ctx.semantic_lookup);
    for read in &mut draft.reads {
        read.project_consumer_non_strict = false;
        if !read.callee_callback_timing
            || !read.execution.reports_untracked_read()
            || read.summary_attributed
            || read.missing_jsx_census
            || read.host_callback_timing
            || read.callback_invocation_unproven
        {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(read.location.start_byte),
            u32::try_from(read.location.end_byte),
        ) else {
            continue;
        };
        if let Some(file) = analyzed_file(ctx, read.location.path.as_ref()) {
            read.project_consumer_non_strict =
                consumers.proves_non_strict(file, solid_facts::core::Span::new(start, end));
        }
    }
    for obligation in interprocedural.dispatch_obligations.iter() {
        let Some((call, context)) = crate::interproc::held_at_tracked_call(obligation) else {
            draft.push_defect(obligation.clone());
            continue;
        };
        if analyzed_file(ctx, obligation.location.path.as_ref()).is_some_and(|file| {
            local_access_context.tracked_call_hands_effect(file, call, &open_computes)
        }) {
            let mut kept = obligation.clone();
            kept.analysis_context = context.to_owned();
            draft.push_defect(kept);
        }
    }
    // The census: every reference of a proven source is followed, or an
    // obligation naming where it went.
    let mut callables = ctx
        .accessors
        .keys()
        .chain(ctx.setters.keys())
        .chain(ctx.actions.keys())
        .chain(ctx.prop_sources.keys())
        .collect::<Vec<_>>();
    callables.sort_unstable();
    callables.dedup();
    // The rows a census walk asks about: built once, shared by every worker.
    let census_rows = crate::value_use::row_index(
        draft
            .reads
            .iter()
            .map(|read| &read.location)
            .chain(draft.writes.iter().map(|write| &write.location))
            .chain(
                draft
                    .action_invocations
                    .iter()
                    .map(|action| &action.location),
            )
            .chain(handed.iter().map(|obligation| &obligation.location))
            .chain(draft.static_defects.iter().map(|defect| &defect.location))
            // ADR 0874: a withheld call's rows are about its target; the
            // value on its chain keeps a row of its own.
            .filter(|location| {
                !draft.withheld_callees.contains(&(
                    location.path.clone(),
                    location.start_byte,
                    location.end_byte,
                ))
            }),
    );
    // Every start in the order the rows below are filed: each reference of
    // each source, then (P13) each creating call no binding holds.
    let mut starts = Vec::new();
    for symbol in callables.iter().copied() {
        for reference in ctx.semantic_lookup.symbol_references(symbol.as_str()) {
            starts.push(CensusStart::Reference(symbol, reference));
        }
    }
    for file in ctx.semantic_lookup.file_slots() {
        for call in &file.ast.calls {
            let Some(primitive) = ctx.semantic_lookup.primitive_at_call(file, call.span) else {
                continue;
            };
            if !ctx.dialect.creates_reactive_source(primitive) {
                continue;
            }
            starts.push(CensusStart::Created {
                file,
                call: call.span,
                store: ctx.dialect.returns_store(primitive),
                tuple: ctx.dialect.returns_reactive_tuple(primitive),
            });
        }
    }
    // ADR 0909: a start's answer is a function of the build and the start
    // alone (`Census::begin_start`), so the starts are walked on the workers,
    // each with a census of its own, and filed below in order. ADR 0878's
    // step budget is per start, inside its walk.
    let census_sources = callables
        .iter()
        .map(|symbol| symbol.as_ref())
        .collect::<crate::HashSet<&str>>();
    let answers = crate::pipeline::parallel_balanced_with_state(
        &starts,
        || {
            // ADR 0811: a store member write SC2003 decides is that use's row
            // (`holder.store.count = 5`); its pass runs after this one.
            let member_writes = upstream_compat::MemberWrites::new(ctx);
            crate::value_use::Census::new(
                ctx.semantic_lookup,
                ctx.entities,
                ctx.symbol_names,
                census_sources.clone(),
                &interprocedural.source_call_targets,
                crate::attribution::ExportedCallers::new(
                    ctx.semantic_lookup,
                    ctx.aliases,
                    ctx.symbols_by_root,
                ),
                &census_rows,
            )
            .with_member_writes(Box::new(move |file| member_writes.decided(file)))
        },
        |census, start| match start {
            CensusStart::Reference(symbol, reference) => {
                let root = ctx.prop_sources.contains_key(*symbol)
                    || ctx.source_kinds.get(*symbol) == Some(&crate::ReactiveSourceKind::Store);
                let pending = ctx.async_sources.contains(*symbol);
                if root {
                    census.root_start(reference, pending)
                } else {
                    census.callable_start(reference, ctx.accessors.contains_key(*symbol), pending)
                }
            }
            CensusStart::Created {
                file,
                call,
                store,
                tuple,
            } => census.created_start(file, *call, *store, *tuple),
        },
    );
    let census_obligation =
        |callee: &str, location: typefacts::Location, blocker: &str| crate::StaticDefect {
            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                callee: callee.to_owned(),
                member: None,
            },
            location,
            analysis_context: blocker.to_owned(),
            fixes: vec![],
            uncertain: true,
        };
    // After every walk's own row: one SC9012 stands per start, and a
    // reference's row stays where it was.
    let mut use_rows = CensusRows::default();
    // A walk the step budget cut: unknown past the cut, so a row at its root
    // that nothing deduplicates or covers. A root in a dependency file is not
    // reported: the project's references of the same symbol carry it too.
    let mut budget_rows = Vec::new();
    let budget = |callee: &str, location: typefacts::Location| {
        census_obligation(
            callee,
            location,
            &crate::blockers::with_detail(crate::blockers::VALUE_USE_UNCLASSIFIED, "budget"),
        )
    };
    // ADR 0860: the rows past an escape, after every other.
    let mut escaped_rows = CensusRows::default();
    // The census's own rows at its references, in walk order: put in an
    // order of their own before they are pushed.
    let mut census_handed = Vec::new();
    for (start, answer) in starts.iter().zip(answers) {
        let crate::value_use::StartAnswer {
            open,
            cut,
            uses,
            escaped_uses,
        } = answer;
        match start {
            CensusStart::Reference(symbol, reference) => {
                let callee = ctx
                    .symbol_names
                    .get(*symbol)
                    .map_or_else(|| "this value".to_owned(), ToString::to_string);
                if cut {
                    budget_rows.push(budget(&callee, reference.clone()));
                    if let Some(package) = ctx.facts.dependency_source_root(reference.path.as_ref())
                    {
                        let mut project = ctx
                            .semantic_lookup
                            .symbol_references(symbol.as_str())
                            .into_iter()
                            .filter(|other| {
                                ctx.facts
                                    .dependency_source_root(other.path.as_ref())
                                    .is_none()
                            })
                            .collect::<Vec<_>>();
                        // Review D33: a dependency source the project never names
                        // still reaches it; its imports of the package carry the row.
                        if project.is_empty() {
                            project = project_imports_of(ctx, package);
                        }
                        budget_rows.extend(project.into_iter().map(|other| budget(&callee, other)));
                    }
                }
                // The obligation sits at each use the walk ended at, too.
                for (location, blocker) in uses {
                    use_rows.keep_least(census_obligation(&callee, location, blocker));
                }
                for (location, blocker) in escaped_uses {
                    escaped_rows.keep_least(census_obligation(&callee, location, blocker));
                }
                let Some(blocker) = open else {
                    continue;
                };
                // The budget row stands for a reference the cut left open.
                if cut && blocker == crate::blockers::VALUE_USE_UNCLASSIFIED {
                    continue;
                }
                census_handed.push(census_obligation(&callee, reference.clone(), blocker));
            }
            // P13: a source with no name has no reference to start from.
            CensusStart::Created { file, call, .. } => {
                if cut {
                    budget_rows.push(budget("this value", call.location(file.path.as_str())));
                    if let Some(package) = ctx.facts.dependency_source_root(file.path.as_str()) {
                        budget_rows.extend(
                            project_imports_of(ctx, package)
                                .into_iter()
                                .map(|import| budget("this value", import)),
                        );
                    }
                }
                for (location, blocker) in uses {
                    use_rows.keep_least(census_obligation("this value", location, blocker));
                }
                for (location, blocker) in escaped_uses {
                    escaped_rows.keep_least(census_obligation("this value", location, blocker));
                }
                let Some(blocker) = open else {
                    continue;
                };
                if cut && blocker == crate::blockers::VALUE_USE_UNCLASSIFIED {
                    continue;
                }
                census_handed.push(census_obligation(
                    "this value",
                    call.location(file.path.as_str()),
                    blocker,
                ));
            }
        }
    }
    // ADR 0840: a standard call the impostor census withholds may run any
    // function the program puts at that member, a setter or an accessor as
    // well as a method: the call is the use, and its row names the census.
    // ADR 0875: the row names the site that holds it.
    let mut holders = None;
    let files = ctx.semantic_lookup.file_slots();
    // The draft's withheld calls (asked once per call); a draft built without
    // them asks here.
    let mut withheld_calls = std::mem::take(&mut draft.withheld_calls);
    if withheld_calls.len() != files.len() {
        withheld_calls = files
            .iter()
            .map(|file| {
                file.ast
                    .calls
                    .iter()
                    .enumerate()
                    .filter(|(_, call)| ctx.semantic_lookup.impostor_withheld(file, call.callee))
                    .map(|(index, _)| index)
                    .collect()
            })
            .collect();
    }
    for (file, calls) in files.iter().zip(&withheld_calls) {
        for call in calls
            .iter()
            .map(|index| &file.ast.calls[*index])
            .filter(|call| !call.construct)
        {
            let holders = holders.get_or_insert_with(|| {
                crate::base::contributions::impostor_holders(ctx.semantic_lookup.file_slots())
            });
            let mut row = census_obligation(
                file.source_text(call.callee).unwrap_or("this call"),
                call.callee.location(file.path.as_str()),
                crate::blockers::STANDARD_MEMBER_IMPOSTOR,
            );
            if let Some(detail) = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(call.callee))
                .and_then(|member| file.source_text(member.property))
                .and_then(|member| crate::base::contributions::impostor_detail(member, holders))
            {
                row.analysis_context = crate::blockers::with_detail(
                    crate::blockers::STANDARD_MEMBER_IMPOSTOR,
                    &detail,
                );
            }
            handed.push(row);
        }
    }
    census_handed.sort_by(|a, b| census_row_order(a).cmp(&census_row_order(b)));
    handed.extend(census_handed);
    handed.extend(use_rows.into_rows());
    // A row any other producer placed over the call (a read, a write, an
    // action) already speaks for it.
    // Only a row whose role is reported speaks for the call; a read row
    // only for an obligation raised for a possible read. Those rows' spans by
    // path, sorted by start: a row over a span starts at or before it.
    let mut reported_reads = crate::HashMap::<std::sync::Arc<str>, Vec<(u64, u64)>>::default();
    for read in draft
        .reads
        .iter()
        .filter(|read| read.execution.reports_untracked_read() || read.execution.reports_unplaced())
    {
        reported_reads
            .entry(std::sync::Arc::clone(&read.location.path))
            .or_default()
            .push((read.location.start_byte, read.location.end_byte));
    }
    let mut reported_writes = crate::HashMap::<std::sync::Arc<str>, Vec<(u64, u64)>>::default();
    for location in draft
        .writes
        .iter()
        .filter(|write| {
            write.execution.reports_disallowed_write() || write.execution.reports_unplaced()
        })
        .map(|write| &write.location)
        .chain(
            draft
                .action_invocations
                .iter()
                .filter(|action| {
                    action.execution.reports_disallowed_write()
                        || action.execution.reports_unplaced()
                })
                .map(|action| &action.location),
        )
    {
        reported_writes
            .entry(std::sync::Arc::clone(&location.path))
            .or_default()
            .push((location.start_byte, location.end_byte));
    }
    for spans in reported_reads
        .values_mut()
        .chain(reported_writes.values_mut())
    {
        spans.sort_unstable();
    }
    let over = |rows: &crate::HashMap<std::sync::Arc<str>, Vec<(u64, u64)>>,
                at: &typefacts::Location| {
        rows.get(at.path.as_ref()).is_some_and(|spans| {
            spans[..spans.partition_point(|(start, _)| *start <= at.start_byte)]
                .iter()
                .any(|(_, end)| at.end_byte <= *end)
        })
    };
    let covered = |obligation: &crate::StaticDefect| {
        let at = &obligation.location;
        let read_only = obligation.analysis_context == crate::local_access::UNRESOLVED_HANDED_READ
            || obligation.analysis_context.starts_with("value-");
        (read_only && over(&reported_reads, at)) || over(&reported_writes, at)
    };
    let handed = handed
        .into_iter()
        .filter(|obligation| !covered(obligation))
        .collect::<Vec<_>>();
    // A use before the escape speaks for its span only where its own row
    // stands: one `covered` dropped leaves the escaped row there.
    let kept = handed
        .iter()
        .map(|obligation| {
            let at = &obligation.location;
            (at.path.clone(), at.start_byte, at.end_byte)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let escaped_rows = escaped_rows
        .into_rows()
        .filter(|obligation| {
            let at = &obligation.location;
            !kept.contains(&(at.path.clone(), at.start_byte, at.end_byte))
        })
        .collect::<Vec<_>>();
    for obligation in handed {
        draft.push_defect(obligation);
    }
    draft.static_defects.extend(budget_rows);
    draft.deferred_defects.extend(escaped_rows);
    // ADR 0509: a reported read of a proven async source with no async row
    // (a transported read, a module source read in another file) may meet the
    // source pending; give it the row the projection opens it with
    // (`options_opaque`: nothing it projects is a proven violation).
    let async_rows = draft
        .async_reads
        .iter()
        .map(|read| read.location.clone())
        .collect::<HashSet<_>>();
    // ADR 0771: so may a read a helper makes at a tracked position through a
    // value it was handed, with no boundary to catch it (`<p>{peek(a)}</p>`).
    let attributed_at_tracked = |read: &crate::ReactiveRead| {
        read.execution == crate::ExecutionRole::TrackedJsx
            && read.origin.is_some()
            && analyzed_file(ctx, read.location.path.as_ref()).is_some_and(|file| {
                let (Ok(start), Ok(end)) = (
                    u32::try_from(read.location.start_byte),
                    u32::try_from(read.location.end_byte),
                ) else {
                    return true;
                };
                crate::owners::read_loading_cover(
                    ctx.semantic_lookup,
                    file,
                    solid_facts::core::Span::new(start, end),
                    ctx.symbol_names,
                ) != crate::owners::LoadingCover::Covered
            })
    };
    let synthesized = draft
        .reads
        .iter()
        .filter(|read| {
            // Every condition is a question about the build; the loading-cover
            // walk (`attributed_at_tracked`) is asked last, of the reads the
            // others keep.
            !async_rows.contains(&read.location)
                // Local access decided the rows of a component's own source;
                // these are the sources it could not see: transported, in
                // another file, or at module scope, and a read a local
                // function makes at its call (`wrapUse()`, ADR 0510) whose
                // call it placed no row at.
                && (read.transported
                    || read.origin.is_some()
                    || read.declaration.path != read.location.path
                    || analyzed_file(ctx, read.declaration.path.as_ref())
                        .and_then(|file| {
                            let start = u32::try_from(read.declaration.start_byte).ok()?;
                            let end = u32::try_from(read.declaration.end_byte).ok()?;
                            Some(crate::indexes::at_module_scope(
                                &file.ast,
                                solid_facts::core::Span::new(start, end),
                            ))
                        })
                        .unwrap_or(false))
                && ctx
                    .entities
                    .get(&read.declaration)
                    .is_some_and(|symbol| source.async_sources.contains(symbol))
                // ADR 0632: a declared source's creating pass reads the served
                // value through a local function too: no row, the strict read
                // stands.
                && !analyzed_file(ctx, read.location.path.as_ref())
                    .is_some_and(|file| {
                        let span = solid_facts::core::Span::new(
                            u32::try_from(read.location.start_byte).unwrap_or(0),
                            u32::try_from(read.location.end_byte).unwrap_or(0),
                        );
                        ctx.entities.get(&read.declaration).is_some_and(|symbol| {
                            source
                                .source_async_options
                                .get(symbol)
                                .is_some_and(|options| options.declared_loading)
                        }) && crate::execution_role::served_in_creating_pass(
                            file,
                            span,
                            &read.declaration,
                            ctx.semantic_lookup,
                        )
                    })
                // A read inside a pending probe (`isPending(() => read())`)
                // is the legal way to meet the source pending.
                && !analyzed_file(ctx, read.location.path.as_ref())
                    .and_then(|file| {
                        let start = u32::try_from(read.location.start_byte).ok()?;
                        let end = u32::try_from(read.location.end_byte).ok()?;
                        let call = file.ast.call_at(solid_facts::core::Span::new(start, end))?;
                        Some(crate::execution_role::pending_accessor_probe(
                            file,
                            call.callee,
                            ctx.semantic_lookup,
                        ))
                    })
                    .unwrap_or(false)
                && (read.execution.reports_untracked_read()
                    || read.execution.reports_unplaced()
                    || attributed_at_tracked(read))
        })
        .map(|read| crate::AsyncRead {
            accessor: read.accessor.clone(),
            location: read.location.clone(),
            declaration: read.declaration.clone(),
            execution: read.execution,
            leaf_call_site_gates: Vec::new(),
            leaf_owner: None,
            under_loading: false,
            async_provenance: true,
            declared_loading: false,
            options_opaque: true,
            provenance_open: false,
            caught: false,
            component_prop: false,
            ssr_client_hole: false,
            server_rendering_unresolved: false,
            host_callback_timing: read.host_callback_timing,
            callee_callback_timing: read.callee_callback_timing,
            invocation_context_unproven: false,
            mount_unresolved: false,
            settle_unproven: false,
            call_attributed: read.execution == crate::ExecutionRole::TrackedJsx,
            creation_pass_unproven: false,
            root_pass_unproven: false,
            carrier_run_unproven: false,
            apply_first_run_pending: false,
            pending_origin: read.origin.clone(),
            pending_apply: None,
            origin: None,
            unaccounted: false,
            source_module_unresolved: false,
            walk_cut: false,
        })
        .collect::<Vec<_>>();
    draft.async_reads.extend(synthesized);
    // ADR 0802: the read of such a source has the row an open source has.
    let async_rows = draft
        .async_reads
        .iter()
        .map(|read| read.location.clone())
        .collect::<HashSet<_>>();
    let opened = draft
        .reads
        .iter()
        .filter(|read| {
            matches!(read.kind.as_ref(), "accessor" | "store-path")
                && !async_rows.contains(&read.location)
                // A proved strict read stands: it is wrong whether the source
                // is settled or pending, and a row here would withdraw it.
                && (read.execution == crate::ExecutionRole::TrackedJsx
                    || ((read.execution.reports_untracked_read()
                        || read.execution.reports_unplaced())
                        && read.is_uncertifiable()))
        })
        .filter_map(|read| {
            let file = analyzed_file(ctx, read.location.path.as_ref())?;
            let span = solid_facts::core::Span::new(
                u32::try_from(read.location.start_byte).ok()?,
                u32::try_from(read.location.end_byte).ok()?,
            );
            let symbol = ctx.entities.get(&read.declaration)?;
            // A probe meets the source pending by design, and a computation's
            // own read suspends it: the question moves to whoever reads that.
            if file.ast.call_at(span).is_some_and(|call| {
                crate::execution_role::pending_accessor_probe(
                    file,
                    call.callee,
                    ctx.semantic_lookup,
                ) || crate::execution_role::computation_read_suspends(
                    file,
                    call.callee,
                    ctx.entities,
                    ctx.symbol_names,
                    ctx.semantic_lookup,
                )
            }) || !local_access_context.source_compute_open(symbol, &open_computes)
            {
                return None;
            }
            let cover = crate::owners::read_loading_cover(
                ctx.semantic_lookup,
                file,
                span,
                ctx.symbol_names,
            );
            Some(crate::AsyncRead {
                accessor: read.accessor.clone(),
                location: read.location.clone(),
                declaration: read.declaration.clone(),
                execution: read.execution,
                leaf_call_site_gates: Vec::new(),
                leaf_owner: None,
                under_loading: cover == crate::owners::LoadingCover::Covered,
                async_provenance: true,
                declared_loading: false,
                options_opaque: true,
                provenance_open: true,
                caught: false,
                component_prop: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
                host_callback_timing: read.host_callback_timing,
                callee_callback_timing: read.callee_callback_timing,
                invocation_context_unproven: false,
                mount_unresolved: cover == crate::owners::LoadingCover::Unresolved,
                settle_unproven: false,
                call_attributed: false,
                creation_pass_unproven: false,
                root_pass_unproven: false,
                carrier_run_unproven: false,
                apply_first_run_pending: false,
                pending_origin: read.origin.clone(),
                pending_apply: None,
                origin: None,
                unaccounted: false,
                source_module_unresolved: false,
                walk_cut: false,
            })
        })
        .collect::<Vec<_>>();
    draft.async_reads.extend(opened);
    // ADR 0634: whichever layer reports it, a read of a store that may be its
    // raw value is no proven proxy read.
    for read in &mut draft.reads {
        if !read.uncertain
            && &*read.kind == "store-path"
            && ctx
                .semantic_lookup
                .store_declared_may_be_raw(&read.declaration)
        {
            read.uncertain = true;
        }
    }
    // ADR 0694: in a body only `dynamic()` mounts, a write itself is
    // uncertifiable, so a store Get that only leads to one claims no more.
    for read in &mut draft.reads {
        if read.kind.as_ref() != "store-path"
            || read.execution != crate::ExecutionRole::UntrackedUnwindowed
            || read.is_uncertifiable()
        {
            continue;
        }
        let place = |location: &typefacts::Location| {
            Some((
                analyzed_file(ctx, location.path.as_ref())?,
                solid_facts::core::Span::new(
                    u32::try_from(location.start_byte).ok()?,
                    u32::try_from(location.end_byte).ok()?,
                ),
            ))
        };
        // A row attributed at a call of a helper stands for the Get in it.
        let (Some((file, span)), Some((get_file, get))) = (
            place(&read.location),
            place(read.origin.as_ref().unwrap_or(&read.location)),
        ) else {
            continue;
        };
        if crate::execution_role::dynamic_component_entry(file, span, ctx.semantic_lookup)
            == Some(true)
            && !ctx.semantic_lookup.site_in_undecided_component(file, span)
            && upstream_compat::serves_write_path(get_file, get, ctx.semantic_lookup)
        {
            read.uncertain = true;
            read.via = crate::blockers::WRITE_PATH_VIA.into();
        }
    }
    // ADR 0570: a read not proven to run in the pass that created its source
    // (a later mount of a handed or module-scope memo) may meet it settled.
    for read in &mut draft.async_reads {
        if read.apply_first_run_pending {
            continue;
        }
        let Some(file) = analyzed_file(ctx, read.location.path.as_ref()) else {
            read.creation_pass_unproven = true;
            read.root_pass_unproven = true;
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(read.location.start_byte),
            u32::try_from(read.location.end_byte),
        ) else {
            read.creation_pass_unproven = true;
            continue;
        };
        // ADR 0682: a read attributed at a call in a body `<Dynamic>` mounts has
        // that body's window, as a direct read there has (ADR 0662).
        if read.execution == crate::ExecutionRole::UntrackedRendering {
            let span = solid_facts::core::Span::new(start, end);
            let role = crate::execution_role::dynamic_read_role(
                file,
                span,
                read.execution,
                ctx.semantic_lookup,
            );
            if role == crate::ExecutionRole::UntrackedUnwindowed {
                read.under_loading = crate::owners::read_loading_cover(
                    ctx.semantic_lookup,
                    file,
                    span,
                    ctx.symbol_names,
                ) == crate::owners::LoadingCover::Covered;
            }
            read.execution = role;
        }
        read.creation_pass_unproven = !crate::owners::read_in_creation_pass(
            ctx.semantic_lookup,
            file,
            solid_facts::core::Span::new(start, end),
            &read.declaration,
        );
        // ADR 0662: a mixed mount is no proof either way: the row must stay.
        if read.execution == crate::ExecutionRole::Unknown
            && crate::execution_role::dynamic_component_entry(
                file,
                solid_facts::core::Span::new(start, end),
                ctx.semantic_lookup,
            ) == Some(false)
        {
            read.invocation_context_unproven = true;
        }
        read.root_pass_unproven = !crate::owners::read_in_root_pass(
            ctx.semantic_lookup,
            file,
            solid_facts::core::Span::new(start, end),
        );
    }
    // ADR 0774: an async read no fact places may run at a tracked position with
    // no boundary. SC5001's row alone leaves SC5003 nothing to say at the site,
    // so such a read gets the pair `push_unplaced_async_reads` writes.
    let tracked = draft
        .async_reads
        .iter()
        .filter(|read| read.execution == crate::ExecutionRole::TrackedJsx)
        .map(|read| read.location.clone())
        .collect::<HashSet<_>>();
    let siblings = draft
        .async_reads
        .iter()
        .filter(|read| {
            ((read.execution == crate::ExecutionRole::Unknown && read.invocation_context_unproven)
                || read.component_prop
                || read.callee_callback_timing)
                && !read.under_loading
                && !read.declared_loading
                && !tracked.contains(&read.location)
                && analyzed_file(ctx, read.location.path.as_ref()).is_none_or(|file| {
                    let (Ok(start), Ok(end)) = (
                        u32::try_from(read.location.start_byte),
                        u32::try_from(read.location.end_byte),
                    ) else {
                        return true;
                    };
                    crate::owners::read_loading_cover(
                        ctx.semantic_lookup,
                        file,
                        solid_facts::core::Span::new(start, end),
                        ctx.symbol_names,
                    ) != crate::owners::LoadingCover::Covered
                })
        })
        .map(|read| crate::AsyncRead {
            execution: crate::ExecutionRole::TrackedJsx,
            invocation_context_unproven: false,
            component_prop: false,
            host_callback_timing: false,
            callee_callback_timing: false,
            call_attributed: true,
            ..read.clone()
        })
        .collect::<Vec<_>>();
    draft.async_reads.extend(siblings);
    hold_withdrawn_slot_rows(ctx, &source.withdrawn_array_slots, draft);
    hold_withheld_layer_rows(ctx, draft);
    static_rules::component_returns_conditionally(ctx, draft);
    // After every read, write and action table is merged: the rule's
    // negative proof is that none of them lies inside the predicate.
    static_rules::result_access_callbacks(ctx, draft);
    let compat_started = Instant::now();
    upstream_compat::check_project(ctx, draft);
    timings.upstream_compat = compat_started.elapsed();
    // ADR 0692: upstream's `package-callee` row, pushed last, holds the same
    // reference open as the census row before it (`observe(count)`): one
    // SC9012 per span, the ported one kept.
    let uncaptured = draft
        .static_defects
        .iter()
        .filter(|defect| {
            matches!(
                defect.kind,
                crate::StaticDefectKind::ReactiveSourceUncaptured { .. }
            )
        })
        .map(|defect| defect.location.clone())
        .collect::<HashSet<_>>();
    if !uncaptured.is_empty() {
        draft.static_defects.retain(|defect| {
            !(defect.analysis_context.starts_with("value-")
                && matches!(
                    defect.kind,
                    crate::StaticDefectKind::ReactiveDispatchUnresolved { .. }
                )
                && uncaptured.contains(&defect.location))
        });
    }
    hold_withdrawn_static_rows(draft);
    // ADR 0838: after every producer, every pending read is accounted for.
    crate::pending_accounting::account_pending_reads(
        ctx,
        &|symbol| {
            source.async_sources.contains(symbol)
                || local_access_context.source_compute_open(symbol, &open_computes)
        },
        &|symbol| {
            source
                .source_async_options
                .get(symbol)
                .is_some_and(|options| options.declared_loading)
        },
        &walk_decided_reads.into_iter().collect(),
        draft,
    );
    interprocedural.factory_instances
}

/// ADR 0803, review a9fc #7: a static row (upstream_compat's, a static rule's)
/// names no source symbol, so one that rests on a withdrawn slot is found by
/// place: its span holds a reference of one, which carries the
/// `tuple-iteration-written` obligation. Such a row is a claim about the
/// primitive's own slot, which nothing proves now: uncertain.
fn hold_withdrawn_static_rows(draft: &mut ProgramDraft) {
    let held_references = draft
        .static_defects
        .iter()
        .filter(|defect| defect.analysis_context == crate::blockers::TUPLE_ITERATION_WRITTEN)
        .map(|defect| defect.location.clone())
        .collect::<Vec<_>>();
    if held_references.is_empty() {
        return;
    }
    let holds = |at: &crate::Location| {
        held_references.iter().any(|reference| {
            reference.path == at.path
                && at.start_byte <= reference.start_byte
                && reference.end_byte <= at.end_byte
        })
    };
    for violation in &mut draft.static_violations {
        if !violation.uncertain && holds(&violation.location) {
            violation.uncertain = true;
            if !crate::blockers::named(&violation.analysis_context) {
                crate::blockers::TUPLE_ITERATION_WRITTEN
                    .clone_into(&mut violation.analysis_context);
            }
        }
    }
    for defect in &mut draft.static_defects {
        if !defect.uncertain && holds(&defect.location) {
            defect.uncertain = true;
            if !crate::blockers::named(&defect.analysis_context) {
                crate::blockers::TUPLE_ITERATION_WRITTEN.clone_into(&mut defect.analysis_context);
            }
        }
    }
}

/// ADR 0874: a row placed through a call the census withholds stands at its
/// site, uncertain: an impostor may run that callback anywhere, or never.
fn hold_withheld_layer_rows(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let lookup = ctx.semantic_lookup;
    let in_layer = |at: &crate::Location| {
        let (Ok(start), Ok(end)) = (u32::try_from(at.start_byte), u32::try_from(at.end_byte))
        else {
            return false;
        };
        lookup.file_by_path(at.path.as_ref()).is_some_and(|file| {
            crate::execution_role::runs_in_withheld_layer(
                file,
                solid_facts::core::Span::new(start, end),
                lookup,
            )
        })
    };
    for read in &mut draft.reads {
        if !read.uncertain && read.execution.reports_untracked_read() && in_layer(&read.location) {
            read.uncertain = true;
        }
    }
    for defect in &mut draft.static_defects {
        if !defect.uncertain && in_layer(&defect.location) {
            defect.uncertain = true;
            if !crate::blockers::named(&defect.analysis_context) {
                crate::blockers::STANDARD_MEMBER_IMPOSTOR.clone_into(&mut defect.analysis_context);
            }
        }
    }
    // A pending read a withheld call's callback makes in JSX: the tracked
    // rule's row stands beside the unplaced one, open.
    let tracked = draft
        .async_reads
        .iter()
        .filter(|read| {
            read.execution == crate::ExecutionRole::Unknown
                && in_layer(&read.location)
                && lookup
                    .file_by_path(read.location.path.as_ref())
                    .is_some_and(|file| {
                        let (Ok(start), Ok(end)) = (
                            u32::try_from(read.location.start_byte),
                            u32::try_from(read.location.end_byte),
                        ) else {
                            return false;
                        };
                        file.ast
                            .any_jsx_containing(solid_facts::core::Span::new(start, end))
                    })
        })
        .map(|read| crate::AsyncRead {
            execution: crate::ExecutionRole::TrackedJsx,
            options_opaque: true,
            ..read.clone()
        })
        .collect::<Vec<_>>();
    draft.async_reads.extend(tracked);
    // Anywhere else, the row the standard carrier's run would place (a
    // pending read in a memo's compute: SC5003's) stands too, opaque.
    let placed = draft
        .async_reads
        .iter()
        .filter(|read| read.execution == crate::ExecutionRole::Unknown)
        .filter_map(|read| {
            let file = lookup.file_by_path(read.location.path.as_ref())?;
            let start = u32::try_from(read.location.start_byte).ok()?;
            let end = u32::try_from(read.location.end_byte).ok()?;
            let span = solid_facts::core::Span::new(start, end);
            if file.ast.any_jsx_containing(span) {
                return None;
            }
            let call = crate::execution_role::withheld_layer_call(file, span, lookup)?;
            let allowed = crate::execution_role::allowed_callback_spans(file, lookup);
            let execution = crate::execution_role::semantic_execution_role(
                file,
                call,
                &allowed,
                ctx.entities,
                ctx.symbol_names,
                lookup,
            );
            (execution != crate::ExecutionRole::Unknown).then(|| crate::AsyncRead {
                execution,
                options_opaque: true,
                ..read.clone()
            })
        })
        .collect::<Vec<_>>();
    draft.async_reads.extend(placed);
}

/// ADR 0803: a withdrawn slot stays a source, so transport, the handed net and
/// the census follow it to every use; each row whose source is one is held
/// there. A read keeps its row, uncertain; a write or an async read is
/// replaced by the obligation at its own location.
fn hold_withdrawn_slot_rows(
    ctx: &AnalysisContext<'_>,
    withdrawn: &HashSet<crate::identity::SymbolId>,
    draft: &mut ProgramDraft,
) {
    if withdrawn.is_empty() {
        return;
    }
    let lookup = ctx.semantic_lookup;
    let is_withdrawn = |declaration: &crate::Location| {
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.start_byte),
            u32::try_from(declaration.end_byte),
        ) else {
            return false;
        };
        lookup
            .entity_at(
                declaration.path.as_ref(),
                solid_facts::core::Span::new(start, end),
            )
            .is_some_and(|entity| withdrawn.contains(&*entity.symbol))
    };
    let obligation = |at: &crate::Location, callee: &str| crate::StaticDefect {
        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
            callee: callee.to_owned(),
            member: None,
        },
        location: at.clone(),
        analysis_context: crate::blockers::TUPLE_ITERATION_WRITTEN.to_owned(),
        fixes: vec![],
        uncertain: true,
    };
    let mut held = Vec::new();
    for read in &mut draft.reads {
        if is_withdrawn(&read.declaration) || read.origin.as_ref().is_some_and(is_withdrawn) {
            read.uncertain = true;
            read.withdrawn_slot = true;
            held.push(obligation(&read.location, &read.accessor));
        }
    }
    draft.writes.retain(|write| {
        let keep = !is_withdrawn(&write.declaration);
        if !keep {
            held.push(obligation(&write.location, &write.setter));
        }
        keep
    });
    draft.async_reads.retain(|read| {
        let keep = !is_withdrawn(&read.declaration);
        if !keep {
            held.push(obligation(&read.location, &read.accessor));
        }
        keep
    });
    for defect in held {
        draft.push_defect(defect);
    }
}

/// F0, ADR 0686: a transported target drops the handed obligation only if its
/// summary sees all it reaches. A setter, an action and an accessor are exact.
fn target_summary_complete(
    ctx: &AnalysisContext<'_>,
    target: &crate::interproc::accessor_provenance::Callable,
    summaries: &mut crate::interproc::completeness::SummaryMemo,
) -> bool {
    use crate::interproc::accessor_provenance::Callable;
    let mut complete = |path: &str, span: solid_facts::core::Span| {
        analyzed_file(ctx, path).is_some_and(|file| {
            file.ast
                .functions
                .iter()
                .find(|function| function.span == span)
                .is_some_and(|function| summaries.complete(file, function, ctx.semantic_lookup))
        })
    };
    match target {
        Callable::Function { path, span } => complete(path, *span),
        Callable::ArrayMembers(members) => members.iter().all(|(path, span)| complete(path, *span)),
        _ => true,
    }
}

/// The analyzed file at `path`: a selected slot, or a missing-path probe.
fn analyzed_file<'c>(ctx: &AnalysisContext<'c>, path: &str) -> Option<&'c solid_facts::FileFacts> {
    ctx.semantic_lookup.file_by_path(path)
}

/// Review D33: the project imports a budget row for a walk rooted in the
/// dependency package at `package` is reported at. The project's own imports
/// of the package, else, through the dependency files that import it, the
/// project's imports of the packages that reach it (a package the project
/// reaches only through another); with none at all, every project import of
/// a selected dependency package, so the row is never internal only.
/// Resolution is the compiler's attested one, else a relative path.
fn project_imports_of(ctx: &AnalysisContext<'_>, package: &str) -> Vec<typefacts::Location> {
    let lookup = ctx.semantic_lookup;
    let files = lookup.files();
    // Every non-type import, re-export and module load (`import()`,
    // `require`) that lands in a dependency package: who loads it, the
    // loader's package (None in the project), the landed package. A load with
    // no literal specifier may land anywhere: an edge to every package.
    let land = |file: &solid_facts::FileFacts, span: solid_facts::core::Span, module: &str| {
        let attested = lookup.resolved_imports().and_then(|imports| {
            match imports.specifier(file.path.as_str(), span, module) {
                solid_facts::SpecifierAttestation::Attested(row)
                    if !row.resolved_path.is_empty() =>
                {
                    Some(if row.included_path.is_empty() {
                        row.resolved_path.to_string()
                    } else {
                        row.included_path.to_string()
                    })
                }
                _ => None,
            }
        });
        // A load the compiler did not attest (a module load, often) may
        // still be a relative path to an analyzed file.
        attested.or_else(|| {
            solid_facts::resolve_relative_module_path(
                file.path.as_str(),
                module,
                files.iter().map(|file| file.path.as_str()),
            )
            .map(|path| path.to_string())
        })
    };
    let mut edges = Vec::new();
    let mut open_loads = Vec::new();
    for file in files {
        let from = ctx.facts.dependency_source_root(file.path.as_str());
        let loads = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| (import.span, Some(import.module.as_str()), false))
            .chain(
                file.ast
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| {
                        export
                            .module
                            .as_deref()
                            .map(|module| (export.span, Some(module), false))
                    }),
            )
            .chain(
                file.ast
                    .module_loads
                    .iter()
                    // A load's resolution is joined to its literal's span.
                    .map(|load| {
                        (
                            load.specifier_span.unwrap_or(load.span),
                            load.specifier.as_deref(),
                            true,
                        )
                    }),
            );
        for (span, module, dynamic) in loads {
            let Some(module) = module else {
                open_loads.push((from, span.location(file.path.as_str())));
                continue;
            };
            let landing = land(file, span, module);
            let landed = landing
                .as_deref()
                .and_then(|landing| ctx.facts.dependency_source_root(landing));
            // A dynamic load nothing places in a selected package's source or
            // in the project's own files (no attested resolution, or one to a
            // package's types) may land in any package.
            let Some(landed) = landed else {
                let in_project = landing.as_deref().is_some_and(|landing| {
                    files.iter().any(|other| {
                        other.path.as_str() == landing
                            && ctx.facts.dependency_source_root(landing).is_none()
                    })
                });
                if dynamic && !in_project {
                    open_loads.push((from, span.location(file.path.as_str())));
                }
                continue;
            };
            if from == Some(landed) {
                continue;
            }
            edges.push((from, landed, span.location(file.path.as_str())));
        }
    }
    for (from, location) in open_loads {
        for package in &ctx.facts.dependency_source_roots {
            if from != Some(package.as_str()) {
                edges.push((from, package.as_str(), location.clone()));
            }
        }
    }
    let mut reached = vec![package];
    let mut found = Vec::new();
    let mut index = 0;
    while index < reached.len() {
        let current = reached[index];
        index += 1;
        for (from, landed, location) in &edges {
            if *landed != current {
                continue;
            }
            match from {
                None => found.push(location.clone()),
                Some(importer) if !reached.contains(importer) => reached.push(importer),
                Some(_) => {}
            }
        }
    }
    if found.is_empty() {
        found = edges
            .iter()
            .filter(|(from, _, _)| from.is_none())
            .map(|(_, _, location)| location.clone())
            .collect();
    }
    // Review D33: no project load names a selected package at all; the row
    // still stands in the project, at the start of its first file.
    if found.is_empty()
        && let Some(file) = files
            .iter()
            .filter(|file| {
                ctx.facts
                    .dependency_source_root(file.path.as_str())
                    .is_none()
            })
            .min_by(|a, b| a.path.as_str().cmp(b.path.as_str()))
    {
        found.push(solid_facts::core::Span::new(0, 0).location(file.path.as_str()));
    }
    found
}

/// One start of the value-use census (ADR 0909).
enum CensusStart<'s, 'a> {
    /// A reference of a source.
    Reference(&'s crate::SymbolId, typefacts::Location),
    /// P13: a creating call no binding holds.
    Created {
        file: &'a solid_facts::FileFacts,
        call: solid_facts::core::Span,
        store: bool,
        tuple: bool,
    },
}

/// The census's rows, one per span. Walks run in the order of their sources'
/// symbol ids, which hash the file's absolute path, so where several walks end
/// at one span the row is the least by blocker and callee, never the first
/// walk's: the same program gives the same rows wherever it lies.
#[derive(Default)]
struct CensusRows(std::collections::BTreeMap<(std::sync::Arc<str>, u64, u64), crate::StaticDefect>);

impl CensusRows {
    fn keep_least(&mut self, row: crate::StaticDefect) {
        use std::collections::btree_map::Entry;
        let at = &row.location;
        match self.0.entry((at.path.clone(), at.start_byte, at.end_byte)) {
            Entry::Vacant(slot) => {
                slot.insert(row);
            }
            Entry::Occupied(mut slot) => {
                if census_row_order(&row) < census_row_order(slot.get()) {
                    slot.insert(row);
                }
            }
        }
    }

    fn into_rows(self) -> impl Iterator<Item = crate::StaticDefect> {
        self.0.into_values()
    }
}

/// A census row's order among rows of one file: its span, then its blocker and
/// callee. Never a symbol id or a walk's turn.
fn census_row_order(row: &crate::StaticDefect) -> (&str, u64, u64, &str, &str) {
    let callee = match &row.kind {
        crate::StaticDefectKind::ReactiveDispatchUnresolved { callee, .. } => callee.as_str(),
        _ => "",
    };
    (
        row.location.path.as_ref(),
        row.location.start_byte,
        row.location.end_byte,
        row.analysis_context.as_str(),
        callee,
    )
}

#[cfg(test)]
mod tests {
    use typefacts::Location;

    use crate::pipeline::ProgramDraft;
    use crate::{StaticDefect, StaticDefectFamily, StaticDefectKind};

    /// Unavailable-package-behavior obligations and unresolved-dispatch
    /// obligations are two finding kinds (`SC9005` and `SC9012`). Under one
    /// dedup identity the pair that a call site into an unanalyzed package
    /// naturally produces at one start byte suppressed each other, and which
    /// one survived depended on push order.
    #[test]
    fn unavailable_behavior_and_dispatch_obligations_do_not_deduplicate_each_other() {
        let location = Location {
            path: "App.tsx".into(),
            start_byte: 12,
            end_byte: 20,
        };
        let unavailable = StaticDefect {
            kind: StaticDefectKind::PackageBehaviorUnavailable {
                module: "reactive-package".into(),
                export: "mapValue".into(),
                reexported: false,
            },
            location: location.clone(),
            analysis_context: "callback arguments".into(),
            fixes: vec![],
            uncertain: false,
        };
        let dispatch = StaticDefect {
            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                callee: "mapValue".into(),
                member: None,
            },
            location,
            analysis_context: "parameter-member-argument-unresolved".into(),
            fixes: vec![],
            uncertain: true,
        };
        assert_ne!(
            unavailable.kind.dedup_identity(),
            dispatch.kind.dedup_identity()
        );
        let mut draft = ProgramDraft::default();
        draft.push_defect(unavailable.clone());
        draft.push_defect(dispatch);
        assert_eq!(draft.static_defects.len(), 2);
        // The identity still deduplicates its own kind at the same byte.
        draft.push_defect(unavailable);
        assert_eq!(draft.static_defects.len(), 2);
    }

    /// The identity strings are load-bearing: they are the keys of one
    /// identity space shared by every stage that files a static defect, so a
    /// rename silently merges or splits dedup groups. Pin the mapping of every
    /// kind that reaches [`ProgramDraft::push_defect`].
    #[test]
    fn dedup_identities_are_pinned_per_finding_family() {
        assert_eq!(
            StaticDefectFamily::PackageBehaviorUnavailable.dedup_identity(),
            "package-behavior-unavailable"
        );
        assert_eq!(
            StaticDefectFamily::ReactiveDispatchUnresolved.dedup_identity(),
            "reactive-dispatch-unresolved"
        );
        assert_eq!(
            StaticDefectFamily::ReactiveObjectDestructure.dedup_identity(),
            "no-destructure"
        );
        assert_eq!(
            StaticDefectFamily::ReactiveReadAfterAwait.dedup_identity(),
            "reactive-read-after-await"
        );
        assert_eq!(
            StaticDefectFamily::ComponentReturnsConditionally.dedup_identity(),
            "components-return-once"
        );

        // Kinds that share a dialect rule share the identity.
        assert_eq!(
            StaticDefectKind::ReactiveCallbackUnresolved {
                callee: "map".into()
            }
            .dedup_identity(),
            StaticDefectKind::ReactiveDispatchUnresolved {
                callee: "map".into(),
                member: None
            }
            .dedup_identity()
        );
        assert_eq!(
            StaticDefectKind::StructuredReturnUnresolved {
                function: "make".into(),
                property: "value".into(),
                reason: "shorthand".into(),
            }
            .dedup_identity(),
            StaticDefectFamily::ReactiveDispatchUnresolved.dedup_identity()
        );
    }
}
