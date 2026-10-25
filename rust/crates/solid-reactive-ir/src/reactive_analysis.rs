//! The coupled local/interprocedural stage.
//!
//! Local reads and the cross-file fixed point share source-discovery inputs
//! and may run concurrently on large projects. This module owns that policy,
//! its cache slots, timing split, and the merge into `ProgramDraft`; the
//! top-level pipeline only sequences the resulting stage.

use crate::HashMap;
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
    for obligation in dispatch_obligations {
        draft.push_defect(obligation);
    }
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
        if let Some(file) = ctx
            .semantic_lookup
            .file_by_path(read.location.path.as_ref())
        {
            read.project_consumer_non_strict =
                consumers.proves_non_strict(file, solid_facts::core::Span::new(start, end));
        }
    }
    for obligation in interprocedural.dispatch_obligations.iter() {
        draft.push_defect(obligation.clone());
    }
    static_rules::component_returns_conditionally(ctx, draft);
    // After every read, write and action table is merged: the rule's
    // negative proof is that none of them lies inside the predicate.
    static_rules::result_access_callbacks(ctx, draft);
    let compat_started = Instant::now();
    upstream_compat::check_project(ctx, draft);
    timings.upstream_compat = compat_started.elapsed();
    interprocedural.factory_instances
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
