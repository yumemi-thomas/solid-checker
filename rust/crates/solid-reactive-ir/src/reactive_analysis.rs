//! The coupled local/interprocedural stage.
//!
//! Local reads and the cross-file fixed point share source-discovery inputs
//! and may run concurrently on large projects. This module owns that policy,
//! its cache slots, timing split, and the merge into `ProgramDraft`; the
//! top-level pipeline only sequences the resulting stage.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use solid_facts::core::SourcePath;
use typefacts::Declaration;

use crate::cache::{
    CachedInterproceduralGraph, CachedInterproceduralResults, CachedLateStages,
    CachedTypedAccessors, LocalAccessResult, PARALLEL_INDEX_FILE_THRESHOLD, ReusePlan,
    SourceDiscoveryTypeScriptDelta,
};
use crate::indexes::ProjectIndexes;
use crate::interproc::{InterproceduralContext, InterproceduralTimings};
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
    pub(crate) typed_accessors: Option<&'a mut HashMap<SourcePath, CachedTypedAccessors>>,
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

pub(crate) fn collect_project<'facts>(
    inputs: ProjectInputs<'_, 'facts>,
    mut caches: IncrementalCaches<'_>,
    reuse: ReusePlan,
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
        accessor_origins: &source.accessor_origins,
        setters: &source.setters,
        actions: &source.actions,
        source_primitives: &source.source_primitives,
        async_sources: &source.async_sources,
        source_async_options: &source.source_async_options,
        server_rendering: crate::source_discovery::project_server_rendering(
            ctx.facts,
            &ctx.rule_options.runtime,
        ),
        source_declarations,
        contract_reads: &source.contract_reads,
        contract_parameter_reads: &source.contract_parameter_reads,
        contract_returns: &source.contract_returns,
        bundled_returns: &source.bundled_returns,
        source_kinds: ctx.source_kinds,
        prop_sources: ctx.prop_sources,
        uncertain_prop_sources: ctx.uncertain_prop_sources,
        props_reactivity: ctx.props_reactivity,
    };
    let cached_interprocedural = reuse
        .late_stages_reusable
        .then(|| {
            caches
                .late_stages
                .as_deref()
                .and_then(|cache| cache.interprocedural.as_ref())
                .cloned()
        })
        .flatten();
    let references_by_source = if cached_interprocedural.is_some() {
        HashMap::new()
    } else {
        references_for_sources(
            &ctx.facts.typescript,
            ctx.symbols_by_root,
            ctx.accessors.keys(),
        )
    };
    let local_access_cache = caches
        .late_stages
        .as_deref_mut()
        .map(|cache| &mut cache.local_accesses);
    // Overlapping the two lanes only pays off on a large project, and only
    // where the host has a second worker: a wasm32-wasip1 reactor build has no
    // thread support and `Scope::spawn` panics there instead of degrading to
    // an inline call.
    let overlap = cached_interprocedural.is_none()
        && ctx.facts.files.len() >= PARALLEL_INDEX_FILE_THRESHOLD
        && available_analysis_workers() > 1;
    let interprocedural_context = InterproceduralContext {
        facts: ctx.facts,
        project_indexes,
        accessors: ctx.accessors,
        contracted_accessor_symbols: &source.contracted_accessor_symbols,
        returned_source_symbols: &source.returned_source_symbols,
        summary_source_symbols: &source.summary_source_symbols,
        source_phases: &source.source_phases,
        source_kinds: ctx.source_kinds,
        contract_reads: &source.contract_reads,
        contract_parameter_reads: &source.contract_parameter_reads,
        contract_callbacks: &source.contract_callbacks,
        contract_returns: &source.contract_returns,
        bundled_returns: &source.bundled_returns,
        source_primitives: &source.source_primitives,
        entities: ctx.entities,
        references_by_source: &references_by_source,
        symbol_names: ctx.symbol_names,
        changed_semantic_symbols: typescript_delta.map(|delta| &delta.semantic_symbol_ids),
        retained_source_paths: &source.retained_source_paths,
        lookup: ctx.semantic_lookup,
    };
    let run_local_access = || {
        local_access_context.build(
            local_access_cache,
            LocalAccessReuse {
                aggregate_reusable: reuse.late_stages_reusable,
                typescript_unchanged: reuse.typescript_unchanged,
                source_discovery_delta: typescript_delta,
                changed_source_symbols: &source.changed_source_symbols,
                retained_source_paths: &source.retained_source_paths,
                global_async_context_unchanged: reuse.late_stages_reusable,
            },
        )
    };
    let (local_access, interprocedural, local_elapsed, interprocedural_elapsed, reused) =
        std::thread::scope(|scope| {
            if let Some(mut cached) = cached_interprocedural {
                let started = Instant::now();
                let local_access = run_local_access();
                cached.timings = InterproceduralTimings::default();
                return (
                    local_access,
                    cached,
                    started.elapsed(),
                    Duration::ZERO,
                    true,
                );
            }
            if overlap {
                let worker_limit = analysis_worker_limit_for_lanes(2);
                let interprocedural = scope.spawn(move || {
                    let _worker_limit = AnalysisWorkerLimit::enter(worker_limit);
                    let started = Instant::now();
                    let result = interprocedural_context.build(
                        caches.typed_accessors,
                        caches.interprocedural_graph,
                        caches.interprocedural_results,
                    );
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
                    false,
                )
            } else {
                let started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = started.elapsed();
                let started = Instant::now();
                let interprocedural = interprocedural_context.build(
                    caches.typed_accessors,
                    caches.interprocedural_graph,
                    caches.interprocedural_results,
                );
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    started.elapsed(),
                    false,
                )
            }
        });
    timings.interprocedural_reused = reused;
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
    if !reused && let Some(cache) = caches.late_stages.as_deref_mut() {
        cache.interprocedural = Some(interprocedural.clone());
    }
    timings.local_accesses_reused = local_access.reused;
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
    // Unresolved-dispatch obligations and contract-incomplete consumer
    // obligations travel in one vector because the same interprocedural walk
    // discovers both, but they are two different finding kinds: `SC9012`
    // versus `SC9005`. Under one dedup identity two findings that merely start
    // at the same byte suppressed one another, and which survived depended on
    // push order. `push_defect` therefore derives the identity from the defect
    // kind's family and never from the vector it arrived in.
    for obligation in dispatch_obligations {
        draft.push_defect(obligation);
    }
    timings.absorb_interprocedural(&interprocedural.timings);
    draft.strict_read_obligations += interprocedural.reads.len();
    draft.reads.extend(interprocedural.reads.iter().cloned());
    for obligation in interprocedural.dispatch_obligations.iter() {
        draft.push_defect(obligation.clone());
    }
    static_rules::component_returns_conditionally(ctx, draft);
    draft.contract_exports = interprocedural.exports.clone();
    draft.contract_generation_obligations =
        interprocedural.contract_generation_obligations.to_vec();
    for obligation in interprocedural.contract_generation_obligations.iter() {
        draft.push_defect(crate::StaticDefect {
            kind: crate::StaticDefectKind::UnknownCallbackExecution {
                package: obligation.package.clone(),
                entrypoint: obligation.entrypoint.clone(),
                function: obligation.function.clone(),
                parameter: obligation.parameter,
                parameter_type: obligation.parameter_type.clone(),
                required_execution: obligation.required_execution.clone(),
                contract_stub: obligation.contract_stub.clone(),
            },
            location: obligation.location.clone(),
            analysis_context: obligation.message.clone(),
            fixes: vec![],
            uncertain: false,
        });
    }
    upstream_compat::check_project(
        ctx,
        caches
            .late_stages
            .as_deref_mut()
            .map(|cache| &mut cache.compat_reference_locations),
        reuse.late_stages_reusable,
        draft,
    );
    interprocedural.factory_instances
}

#[cfg(test)]
mod tests {
    use typefacts::Location;

    use crate::pipeline::ProgramDraft;
    use crate::{StaticDefect, StaticDefectFamily, StaticDefectKind};

    /// Contract-incomplete consumer obligations ride the same vector as
    /// unresolved-dispatch obligations, but they are two finding kinds
    /// (`SC9005` and `SC9012`). Under one dedup identity the pair that a
    /// contract-driven call site naturally produces at one start byte
    /// suppressed each other, and which one survived depended on push order.
    #[test]
    fn contract_and_dispatch_obligations_do_not_deduplicate_each_other() {
        let location = Location {
            path: "App.tsx".into(),
            start_byte: 12,
            end_byte: 20,
        };
        let contract = StaticDefect {
            kind: StaticDefectKind::PackageContractExportMissing {
                module: "reactive-package".into(),
                export: "mapValue".into(),
                reexported: false,
            },
            location: location.clone(),
            analysis_context: "unbound-contract-claims:callback arguments".into(),
            fixes: vec![],
            uncertain: false,
        };
        let dispatch = StaticDefect {
            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                callee: "mapValue".into(),
                member: None,
            },
            location,
            analysis_context: "contract-parameter-member-argument-unresolved".into(),
            fixes: vec![],
            uncertain: true,
        };
        assert_ne!(
            contract.kind.dedup_identity(),
            dispatch.kind.dedup_identity()
        );
        let mut draft = ProgramDraft::default();
        draft.push_defect(contract.clone());
        draft.push_defect(dispatch);
        assert_eq!(draft.static_defects.len(), 2);
        // The identity still deduplicates its own kind at the same byte.
        draft.push_defect(contract);
        assert_eq!(draft.static_defects.len(), 2);
    }

    /// A contract-generation obligation projects to the same dialect rule as
    /// a contract-incomplete consumer obligation (`SC9005`) but keeps its own
    /// dedup identity: one call site can produce both at one start byte, and
    /// a shared identity would drop whichever was pushed second.
    #[test]
    fn generation_obligations_keep_their_own_identity_within_the_contract_rule() {
        let generation = StaticDefectKind::UnknownCallbackExecution {
            package: "partial-package".into(),
            entrypoint: ".".into(),
            function: "withValue".into(),
            parameter: 0,
            parameter_type: "() => void".into(),
            required_execution: "synchronous".into(),
            contract_stub: "{}".into(),
        };
        let consumer = StaticDefectKind::PackageContractExportMissing {
            module: "partial-package".into(),
            export: "withValue".into(),
            reexported: false,
        };
        assert_eq!(
            generation.family(),
            StaticDefectFamily::UnknownCallbackExecution
        );
        assert_eq!(
            consumer.family(),
            StaticDefectFamily::PackageContractIncomplete
        );
        assert_ne!(generation.dedup_identity(), consumer.dedup_identity());
    }

    /// The identity strings are load-bearing: they are the keys of one
    /// identity space shared by every stage that files a static defect, so a
    /// rename silently merges or splits dedup groups. Pin the mapping of every
    /// kind that reaches [`ProgramDraft::push_defect`].
    #[test]
    fn dedup_identities_are_pinned_per_finding_family() {
        assert_eq!(
            StaticDefectFamily::PackageContractIncomplete.dedup_identity(),
            "package-contract-incomplete"
        );
        assert_eq!(
            StaticDefectFamily::ReactiveDispatchUnresolved.dedup_identity(),
            "reactive-dispatch-unresolved"
        );
        assert_eq!(
            StaticDefectFamily::UnknownCallbackExecution.dedup_identity(),
            "unknown-callback-execution"
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

        // Kinds that share a dialect rule share the identity; the two
        // contract families deliberately do not.
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
        assert_eq!(
            StaticDefectKind::PackageContractEnvironmentDependent {
                module: "pkg".into(),
                export: "value".into(),
                reexported: false,
            }
            .dedup_identity(),
            StaticDefectFamily::PackageContractIncomplete.dedup_identity()
        );
    }
}
