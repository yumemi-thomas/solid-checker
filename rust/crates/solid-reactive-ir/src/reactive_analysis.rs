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
    let overlap =
        ctx.facts.files.len() >= PARALLEL_INDEX_FILE_THRESHOLD && available_analysis_workers() > 1;
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
        async_reads,
        strict_read_obligations,
        write_action_obligations,
        dispatch_obligations,
        static_violations,
    } = local_access.result;
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
    // A row any other producer placed over the call (a read, a write, an
    // action) already speaks for it.
    let covered = |obligation: &crate::StaticDefect| {
        let at = &obligation.location;
        let over = |location: &typefacts::Location| {
            location.path == at.path
                && location.start_byte <= at.start_byte
                && at.end_byte <= location.end_byte
        };
        // Only a row whose role is reported speaks for the call; a read row
        // only for an obligation raised for a possible read.
        let read_only = obligation.analysis_context == crate::local_access::UNRESOLVED_HANDED_READ;
        draft.reads.iter().any(|read| {
            read_only
                && over(&read.location)
                && (read.execution.reports_untracked_read() || read.execution.reports_unplaced())
        }) || draft.writes.iter().any(|write| {
            over(&write.location)
                && (write.execution.reports_disallowed_write()
                    || write.execution.reports_unplaced())
        }) || draft.action_invocations.iter().any(|action| {
            over(&action.location)
                && (action.execution.reports_disallowed_write()
                    || action.execution.reports_unplaced())
        })
    };
    let handed = handed
        .into_iter()
        .filter(|obligation| !covered(obligation))
        .collect::<Vec<_>>();
    for obligation in handed {
        draft.push_defect(obligation);
    }
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
            (read.execution.reports_untracked_read()
                || read.execution.reports_unplaced()
                || attributed_at_tracked(read))
                && !async_rows.contains(&read.location)
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
            pending_origin: None,
            pending_apply: None,
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
                pending_origin: None,
                pending_apply: None,
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
    static_rules::component_returns_conditionally(ctx, draft);
    // After every read, write and action table is merged: the rule's
    // negative proof is that none of them lies inside the predicate.
    static_rules::result_access_callbacks(ctx, draft);
    let compat_started = Instant::now();
    upstream_compat::check_project(ctx, draft);
    timings.upstream_compat = compat_started.elapsed();
    interprocedural.factory_instances
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
