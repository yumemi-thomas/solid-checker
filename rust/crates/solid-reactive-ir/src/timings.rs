//! The Reactive IR pipeline's observable stage-timing contract.

use std::time::{Duration, Instant};

use crate::BuildTimings;
use crate::interproc::InterproceduralTimings;
use crate::owners::OwnerIncrementalTimings;

/// A stage reported when `SOLID_CHECKER_TIMINGS` is enabled.
///
/// Each variant owns both its emitted name and its [`BuildTimings`] field so
/// callers cannot accidentally pair one stage's duration with another stage's
/// label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReactiveIrStage {
    IndexesAndReachability,
    SourceDiscovery,
    TypedAccessorsAndPropRoots,
    PropPropagationAndControlFlow,
    StaticPrepass,
    LocalReadsAndWrites,
    InterproceduralSummaries,
    LocalAndInterprocedural,
    LeafAndCleanup,
    StaticApi,
    Directives,
    OwnerFixedPoint,
    FinalOrdering,
}

impl ReactiveIrStage {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 13] = [
        Self::IndexesAndReachability,
        Self::SourceDiscovery,
        Self::TypedAccessorsAndPropRoots,
        Self::PropPropagationAndControlFlow,
        Self::StaticPrepass,
        Self::LocalReadsAndWrites,
        Self::InterproceduralSummaries,
        Self::LocalAndInterprocedural,
        Self::LeafAndCleanup,
        Self::StaticApi,
        Self::Directives,
        Self::OwnerFixedPoint,
        Self::FinalOrdering,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::IndexesAndReachability => "indexes-and-reachability",
            Self::SourceDiscovery => "source-discovery",
            Self::TypedAccessorsAndPropRoots => "typed-accessors-and-prop-roots",
            Self::PropPropagationAndControlFlow => "prop-propagation-and-control-flow",
            Self::StaticPrepass => "static-prepass",
            Self::LocalReadsAndWrites => "local-reads-and-writes",
            Self::InterproceduralSummaries => "interprocedural-summaries",
            Self::LocalAndInterprocedural => "local-and-interprocedural",
            Self::LeafAndCleanup => "leaf-and-cleanup",
            Self::StaticApi => "static-api",
            Self::Directives => "directives",
            Self::OwnerFixedPoint => "owner-fixed-point",
            Self::FinalOrdering => "final-ordering",
        }
    }

    fn duration_mut(self, timings: &mut BuildTimings) -> &mut Duration {
        match self {
            Self::IndexesAndReachability => &mut timings.indexes_and_reachability,
            Self::SourceDiscovery => &mut timings.source_discovery,
            Self::TypedAccessorsAndPropRoots => &mut timings.typed_accessors_and_prop_roots,
            Self::PropPropagationAndControlFlow => &mut timings.prop_propagation_and_control_flow,
            Self::StaticPrepass => &mut timings.static_prepass,
            Self::LocalReadsAndWrites => &mut timings.local_reads_and_writes,
            Self::InterproceduralSummaries => &mut timings.interprocedural_summaries,
            Self::LocalAndInterprocedural => &mut timings.local_and_interprocedural,
            Self::LeafAndCleanup => &mut timings.leaf_and_cleanup,
            Self::StaticApi => &mut timings.static_api,
            Self::Directives => &mut timings.directives,
            Self::OwnerFixedPoint => &mut timings.owner_fixed_point,
            Self::FinalOrdering => &mut timings.final_ordering,
        }
    }
}

/// Times pipeline stages and emits their `SOLID_CHECKER_TIMINGS` lines.
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

    /// Ends the current stage and starts timing the next one.
    pub(crate) fn finish(&mut self, timings: &mut BuildTimings, stage: ReactiveIrStage) {
        let elapsed = self.started.elapsed();
        self.record(timings, stage, elapsed);
        self.started = Instant::now();
    }

    /// Records a duration measured outside the clock.
    pub(crate) fn record(
        &self,
        timings: &mut BuildTimings,
        stage: ReactiveIrStage,
        elapsed: Duration,
    ) {
        *stage.duration_mut(timings) = elapsed;
        if self.emit {
            eprintln!("{}", Self::stage_line(stage, elapsed));
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

    pub(crate) fn stage_line(stage: ReactiveIrStage, elapsed: Duration) -> String {
        format!(
            "{{\"reactiveIrStage\":\"{}\",\"elapsedNs\":{}}}",
            stage.as_str(),
            elapsed.as_nanos()
        )
    }
}

impl BuildTimings {
    /// Copies the source-discovery lane's contribution out of the
    /// [`BuildTimings`] it accumulated on its own thread.
    pub(crate) fn absorb_source_discovery(&mut self, lane: &Self) {
        self.source_discovery = lane.source_discovery;
        self.source_discovery_reused_files = lane.source_discovery_reused_files;
        self.source_discovery_recomputed_files = lane.source_discovery_recomputed_files;
        self.source_discovery_identity_misses = lane.source_discovery_identity_misses;
        self.typed_accessors_and_prop_roots = lane.typed_accessors_and_prop_roots;
        self.prop_propagation_and_control_flow = lane.prop_propagation_and_control_flow;
    }

    /// Copies the interprocedural stage's own timing breakdown.
    pub(crate) fn absorb_interprocedural(&mut self, timings: &InterproceduralTimings) {
        self.interprocedural_graph = timings.graph;
        self.interprocedural_graph_nodes = timings.graph_nodes;
        self.interprocedural_node_index = timings.node_index;
        self.interprocedural_resolver = timings.resolver;
        self.interprocedural_transport = timings.transport;
        self.interprocedural_merge = timings.merge;
        self.interprocedural_refresh = timings.refresh;
        self.interprocedural_direct_summaries = timings.direct_summaries;
        self.interprocedural_direct_index = timings.direct_index;
        self.interprocedural_direct_references = timings.direct_references;
        self.interprocedural_propagation = timings.propagation;
        self.interprocedural_returned_direct = timings.returned_direct;
        self.interprocedural_returned_delta = timings.returned_delta;
        self.interprocedural_call_summary_delta = timings.call_summary_delta;
        self.interprocedural_factory_propagation = timings.factory_propagation;
        self.interprocedural_results_and_exports = timings.results_and_exports;
        self.interprocedural_result_reads = timings.result_reads;
        self.interprocedural_export_summaries = timings.export_summaries;
        self.interprocedural_graph_reused_files = timings.graph_reused_files;
        self.interprocedural_graph_recomputed_files = timings.graph_recomputed_files;
        self.interprocedural_result_reused_files = timings.result_reused_files;
        self.interprocedural_result_recomputed_files = timings.result_recomputed_files;
        self.interprocedural_export_work = timings.export_work;
    }

    /// Copies the owner stage's own timing breakdown.
    pub(crate) fn absorb_owner(&mut self, timings: &OwnerIncrementalTimings) {
        self.owner_fragment_build = timings.fragment_build;
        self.owner_graph_assembly = timings.graph_assembly;
        self.owner_propagation = timings.propagation;
        self.owner_requirement_emission = timings.requirement_emission;
        self.owner_reused_files += timings.reused_files;
        self.owner_recomputed_files += timings.recomputed_files;
    }
}

/// One build's cross-generation reuse, emitted under `SOLID_CHECKER_TIMINGS`:
/// what the Type Facts delta named and what each retained cache reused.
pub(crate) fn reuse_line(
    facts: &solid_facts::ProjectFacts,
    timings: &BuildTimings,
    domain_reset: bool,
) -> serde_json::Value {
    let changes = facts.typescript_changes.as_ref();
    let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
    let count = |select: fn(&solid_facts::TypeScriptChanges) -> usize| changes.map(select);
    serde_json::json!({
        "reactiveIrReuse": {
            "generation": facts.generation.get(),
            "domainReset": domain_reset,
            "typescriptChanges": changes.is_some(),
            "typescriptUnchanged": changes.is_some_and(|changes| changes.unchanged),
            "entityPaths": count(|changes| changes.entity_paths.len()),
            "filePaths": count(|changes| changes.file_paths.len()),
            "symbolIds": count(|changes| changes.symbol_ids.len()),
            "entityPathSample": changes.map(|changes| changes.entity_paths.iter().take(4).collect::<Vec<_>>()),
            "symbolIdSample": changes.map(|changes| changes.symbol_ids.iter().take(16).collect::<Vec<_>>()),
            "typescriptIndexesReused": timings.typescript_indexes_reused,
            "typescriptIndexesPatched": timings.typescript_indexes_patched,
            "runtimeScan": [timings.runtime_scan_reused_files, timings.runtime_scan_scanned_files],
            "baseIndexChangedFiles": timings.base_index_changed_files,
            "memo": [timings.memo_reused, timings.memo_computed],
            "sourceDiscovery": [timings.source_discovery_reused_files, timings.source_discovery_recomputed_files],
            "sourceDiscoveryIdentityMisses": timings.source_discovery_identity_misses,
            "reachability": [timings.reachability_reused_files, timings.reachability_recomputed_files],
            "localAccess": [timings.local_access_reused_files, timings.local_access_recomputed_files],
            "interprocGraph": [timings.interprocedural_graph_reused_files, timings.interprocedural_graph_recomputed_files],
            "interprocResults": [timings.interprocedural_result_reused_files, timings.interprocedural_result_recomputed_files],
            "owners": [timings.owner_reused_files, timings.owner_recomputed_files],
            "lateStages": [timings.local_accesses_reused, timings.interprocedural_reused, timings.owner_fixed_point_reused],
            "substagesMs": {
                "cacheLookup": ms(timings.cache_lookup),
                "projectIndexes": ms(timings.project_indexes),
                "typescriptIndexes": ms(timings.alias_and_entity_indexes),
                "symbolNames": ms(timings.symbol_name_indexes),
                "boundaryDefects": ms(timings.boundary_defects),
                "namespaceHandOuts": ms(timings.namespace_hand_outs),
                "reusePlan": ms(timings.reuse_plan),
                "upstreamCompat": ms(timings.upstream_compat),
                "readAfterAwaitThroughCalls": ms(timings.read_after_await_through_calls),
                "leafOperations": ms(timings.leaf_operations),
                "staticApiFiles": ms(timings.static_api_files),
                "serverRules": ms(timings.server_rules),
                "reachability": ms(timings.reachability),
                "memoBegin": ms(timings.memo_begin),
                "baseIndexUpdate": ms(timings.base_index_update),
            },
            "interprocMs": {
                "graph": ms(timings.interprocedural_graph),
                "graphNodes": ms(timings.interprocedural_graph_nodes),
                "nodeIndex": ms(timings.interprocedural_node_index),
                "resolver": ms(timings.interprocedural_resolver),
                "transport": ms(timings.interprocedural_transport),
                "merge": ms(timings.interprocedural_merge),
                "refresh": ms(timings.interprocedural_refresh),
                "directSummaries": ms(timings.interprocedural_direct_summaries),
                "propagation": ms(timings.interprocedural_propagation),
                "resultReads": ms(timings.interprocedural_result_reads),
                "exportSummaries": ms(timings.interprocedural_export_summaries),
            },
            "totalNs": u64::try_from(timings.total.as_nanos()).unwrap_or(u64::MAX),
            "shadowNs": u64::try_from(timings.shadow.as_nanos()).unwrap_or(u64::MAX),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Performance tooling consumes these names; changing one is a protocol
    /// change rather than a private refactor.
    #[test]
    fn stage_vocabulary_and_line_shape_are_stable() {
        assert_eq!(
            ReactiveIrStage::ALL.map(ReactiveIrStage::as_str),
            [
                "indexes-and-reachability",
                "source-discovery",
                "typed-accessors-and-prop-roots",
                "prop-propagation-and-control-flow",
                "static-prepass",
                "local-reads-and-writes",
                "interprocedural-summaries",
                "local-and-interprocedural",
                "leaf-and-cleanup",
                "static-api",
                "directives",
                "owner-fixed-point",
                "final-ordering",
            ]
        );
        assert_eq!(
            StageClock::stage_line(ReactiveIrStage::SourceDiscovery, Duration::from_nanos(42)),
            "{\"reactiveIrStage\":\"source-discovery\",\"elapsedNs\":42}"
        );
    }
}
