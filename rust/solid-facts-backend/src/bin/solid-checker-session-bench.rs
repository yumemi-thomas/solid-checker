#![recursion_limit = "256"]

use std::{
    env,
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::json;
use solid_facts_backend::{
    NativeBuildTimings, NativeIncrementalSession, SourceChange, TypeFactsSession,
};

struct Options {
    project: PathBuf,
    typefacts: String,
    iterations: usize,
    warmups: usize,
    edit: Option<PathBuf>,
    edit_mode: EditMode,
    limits: TransferLimits,
}

#[derive(Default)]
struct TransferLimits {
    first_request_bytes: Option<u64>,
    first_response_bytes: Option<u64>,
    sample_request_bytes: Option<u64>,
    sample_response_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
enum EditMode {
    Prefix,
    SameSpanBody,
}

#[derive(Clone, Copy, Debug, Default)]
struct AnalysisTimings {
    wall: std::time::Duration,
    facts: NativeBuildTimings,
    reactive_ir: solid_reactive_ir::BuildTimings,
    solver: solid_reactive_solver::SolveTimings,
}

fn measured_pipeline_duration(timing: AnalysisTimings) -> std::time::Duration {
    timing
        .facts
        .source_analysis
        .saturating_add(timing.facts.type_facts)
        .saturating_add(timing.facts.hydrate)
        .saturating_add(timing.facts.join)
        .saturating_add(timing.reactive_ir.total)
        .saturating_add(timing.solver.total)
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("solid-checker-session-bench: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args()?;
    let project = options.project.canonicalize()?;
    let project_id = project.to_string_lossy().into_owned();
    let startup = Instant::now();
    let typescript = TypeFactsSession::open(&options.typefacts, &project_id, &[])?;
    let (mut session, sources) = NativeIncrementalSession::open_pipelined(project_id, typescript)?;
    let source_count = sources.len();
    let source_setup_ns = nanos(startup.elapsed());
    let mut reactive_ir = solid_reactive_ir::IncrementalBuilder::default();

    let first = Instant::now();
    let mut first_timings = analyze_full(&mut session, &mut reactive_ir)?;
    first_timings.wall = first.elapsed();
    let first_analysis_ns = nanos(first_timings.wall);

    let edit_path = options
        .edit
        .as_deref()
        .map(Path::canonicalize)
        .transpose()?;
    let original = edit_path
        .as_ref()
        .map(std::fs::read_to_string)
        .transpose()?;
    let mut version = 0_u64;
    for index in 0..options.warmups {
        let _ = analyze_iteration(
            &mut session,
            edit_path.as_deref(),
            original.as_deref(),
            options.edit_mode,
            index,
            &mut version,
            &mut reactive_ir,
        )?;
    }
    let mut samples = Vec::with_capacity(options.iterations);
    let mut timing_samples = Vec::with_capacity(options.iterations);
    for index in 0..options.iterations {
        let started = Instant::now();
        let mut timings = analyze_iteration(
            &mut session,
            edit_path.as_deref(),
            original.as_deref(),
            options.edit_mode,
            index + options.warmups,
            &mut version,
            &mut reactive_ir,
        )?;
        timings.wall = started.elapsed();
        samples.push(nanos(timings.wall));
        timing_samples.push(timings);
    }
    let chronological_samples = samples.clone();
    enforce_transfer_limits(&options.limits, first_timings, &timing_samples)?;
    let chronological_breakdown = timing_samples
        .iter()
        .enumerate()
        .map(|(index, timing)| {
            json!({
                "iteration": index,
                "wallNs": nanos(timing.wall),
                "sourceAnalysisNs": nanos(timing.facts.source_analysis),
                "typeFactsNs": nanos(timing.facts.type_facts),
                "typeFactsRoundtripNs": nanos(timing.facts.exchange.roundtrip),
                "serverAnalyzeNs": nanos(timing.facts.exchange.server_analyze),
                "serverDemandNs": nanos(timing.facts.exchange.server_demand),
                "serverAssemblyNs": nanos(timing.facts.exchange.server_assembly),
                "serverSortNs": nanos(timing.facts.exchange.server_sort),
                "serverCloseSymbolsNs": nanos(timing.facts.exchange.server_close_symbols),
                "responseDecodeNs": nanos(timing.facts.exchange.response_decode),
                "hydrateNs": nanos(timing.facts.hydrate),
                "joinNs": nanos(timing.facts.join),
                "reactiveIrNs": nanos(timing.reactive_ir.total),
                "indexesAndReachabilityNs": nanos(timing.reactive_ir.indexes_and_reachability),
                "reachabilityNs": nanos(timing.reactive_ir.reachability),
                "orchestrationNs": nanos(timing.wall.saturating_sub(measured_pipeline_duration(*timing))),
            })
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let cache = session.cache_stats();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 2,
            "project": project,
            "mode": if edit_path.is_some() { "incremental-update-and-full-analysis" } else { "warm-full-analysis" },
            "editMode": edit_path.map(|_| match options.edit_mode {
                EditMode::Prefix => "span-shifting-prefix",
                EditMode::SameSpanBody => "same-span-body",
            }),
            "sourceCount": source_count,
            "iterations": options.iterations,
            "warmups": options.warmups,
            "sourceSetupNs": source_setup_ns,
            "firstAnalysisNs": first_analysis_ns,
            "firstAnalysisBreakdown": type_facts_breakdown([first_timings.facts]),
            "firstRustPipelineBreakdown": rust_pipeline_breakdown([first_timings]),
            "samplesNs": samples,
            "samplesChronologicalNs": chronological_samples,
            "samplesChronologicalBreakdown": chronological_breakdown,
            "medianNs": percentile(&samples, 50),
            "p95Ns": percentile(&samples, 95),
            "minNs": samples.first().copied().unwrap_or(0),
            "maxNs": samples.last().copied().unwrap_or(0),
            "typeFactsBreakdown": type_facts_breakdown(timing_samples.iter().map(|timing| timing.facts)),
            "rustPipelineBreakdown": rust_pipeline_breakdown(timing_samples.iter().copied()),
            "cache": {
                "astEntries": cache.ast_entries,
                "compilerEntries": cache.compiler_entries,
            }
        }))?
    );
    Ok(())
}

fn enforce_transfer_limits(
    limits: &TransferLimits,
    first: AnalysisTimings,
    samples: &[AnalysisTimings],
) -> Result<(), Box<dyn std::error::Error>> {
    let max_request = samples
        .iter()
        .map(|timing| timing.facts.exchange.request_bytes)
        .max()
        .unwrap_or(0);
    let max_response = samples
        .iter()
        .map(|timing| timing.facts.exchange.response_bytes)
        .max()
        .unwrap_or(0);
    for (name, actual, limit) in [
        (
            "first request bytes",
            first.facts.exchange.request_bytes,
            limits.first_request_bytes,
        ),
        (
            "first response bytes",
            first.facts.exchange.response_bytes,
            limits.first_response_bytes,
        ),
        (
            "sample request bytes",
            max_request,
            limits.sample_request_bytes,
        ),
        (
            "sample response bytes",
            max_response,
            limits.sample_response_bytes,
        ),
    ] {
        if let Some(limit) = limit
            && actual > limit
        {
            return Err(format!("{name} {actual} exceeds limit {limit}").into());
        }
    }
    Ok(())
}

fn analyze_iteration(
    session: &mut NativeIncrementalSession,
    edit_path: Option<&Path>,
    original: Option<&str>,
    edit_mode: EditMode,
    iteration: usize,
    version: &mut u64,
    reactive_ir: &mut solid_reactive_ir::IncrementalBuilder,
) -> Result<AnalysisTimings, Box<dyn std::error::Error>> {
    if let (Some(path), Some(original)) = (edit_path, original) {
        *version = version.checked_add(1).ok_or("edit version overflow")?;
        let source = if iteration.is_multiple_of(2) {
            match edit_mode {
                EditMode::Prefix => format!("// solid-checker benchmark edit\n{original}"),
                EditMode::SameSpanBody => original.replacen("count() + 1", "count() + 2", 1),
            }
        } else {
            original.into()
        };
        if iteration.is_multiple_of(2)
            && matches!(edit_mode, EditMode::SameSpanBody)
            && source == original
        {
            return Err("same-span body edit marker `count() + 1` was not found".into());
        }
        let facts = session.edit(
            vec![SourceChange {
                path: path.to_string_lossy().into_owned(),
                version: *version,
                source: Some(source),
                compiler_options: Default::default(),
            }],
            None,
        )?;
        let (reactive_ir_timings, solver) = solve_facts(&facts, reactive_ir)?;
        return Ok(AnalysisTimings {
            wall: std::time::Duration::ZERO,
            facts: session.last_build_timings(),
            reactive_ir: reactive_ir_timings,
            solver,
        });
    }
    analyze_full(session, reactive_ir)
}

fn analyze_full(
    session: &mut NativeIncrementalSession,
    reactive_ir: &mut solid_reactive_ir::IncrementalBuilder,
) -> Result<AnalysisTimings, Box<dyn std::error::Error>> {
    let facts = session.analyze()?;
    let (reactive_ir_timings, solver) = solve_facts(&facts, reactive_ir)?;
    Ok(AnalysisTimings {
        wall: std::time::Duration::ZERO,
        facts: session.last_build_timings(),
        reactive_ir: reactive_ir_timings,
        solver,
    })
}

fn solve_facts(
    facts: &solid_facts::ProjectFacts,
    reactive_ir: &mut solid_reactive_ir::IncrementalBuilder,
) -> Result<
    (
        solid_reactive_ir::BuildTimings,
        solid_reactive_solver::SolveTimings,
    ),
    Box<dyn std::error::Error>,
> {
    let (program, reactive_ir) = reactive_ir.build_shared(facts)?;
    let (_findings, solver) = solid_reactive_solver::solve_measured(&program);
    Ok((reactive_ir, solver))
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let index = (samples.len() - 1) * percentile / 100;
    samples[index]
}

fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn type_facts_breakdown(
    samples: impl IntoIterator<Item = NativeBuildTimings>,
) -> serde_json::Value {
    let samples = samples.into_iter().collect::<Vec<_>>();
    let distribution = |values: Vec<u64>| {
        let mut values = values;
        values.sort_unstable();
        json!({
            "medianNs": percentile(&values, 50),
            "p95Ns": percentile(&values, 95),
            "minNs": values.first().copied().unwrap_or(0),
            "maxNs": values.last().copied().unwrap_or(0),
        })
    };
    let durations = |select: fn(NativeBuildTimings) -> std::time::Duration| {
        distribution(samples.iter().copied().map(select).map(nanos).collect())
    };
    let exchanges =
        |select: fn(solid_facts_backend::TypeFactsExchangeTimings) -> std::time::Duration| {
            distribution(
                samples
                    .iter()
                    .map(|timing| nanos(select(timing.exchange)))
                    .collect(),
            )
        };
    json!({
        "total": durations(|timing| timing.type_facts),
        "demandAssembly": durations(|timing| timing.demand_assembly),
        "requestAssembly": durations(|timing| timing.request_assembly),
        "semanticDemandAssembly": durations(|timing| timing.semantic_demand_assembly),
        "exchangeRoundtrip": exchanges(|timing| timing.roundtrip),
        "requestSend": exchanges(|timing| timing.request_send),
        "requestBytes": distribution(samples.iter().map(|timing| timing.exchange.request_bytes).collect()),
        "serverRequestDecode": exchanges(|timing| timing.server_request_decode),
        "serverAnalyze": exchanges(|timing| timing.server_analyze),
        "serverAsync": exchanges(|timing| timing.server_async),
        "serverDemand": exchanges(|timing| timing.server_demand),
        "serverAssembly": exchanges(|timing| timing.server_assembly),
        "serverSort": exchanges(|timing| timing.server_sort),
        "serverCloseSymbols": exchanges(|timing| timing.server_close_symbols),
        "encodeAndTransport": exchanges(|timing| timing.encode_and_transport()),
        "responseDecode": exchanges(|timing| timing.response_decode),
        "responseBytes": distribution(samples.iter().map(|timing| timing.exchange.response_bytes).collect()),
        "hydrate": durations(|timing| timing.hydrate),
        "join": durations(|timing| timing.join),
        "materializedSamples": samples.iter().filter(|timing| timing.exchange.server_materialized).count(),
        "retainedFiles": distribution(samples.iter().map(|timing| timing.exchange.server_retained_files).collect()),
        "recomputedFiles": distribution(samples.iter().map(|timing| timing.exchange.server_recomputed_files).collect()),
        "nonDurableFiles": distribution(samples.iter().map(|timing| timing.exchange.server_non_durable_files).collect()),
    })
}

fn rust_pipeline_breakdown(
    samples: impl IntoIterator<Item = AnalysisTimings>,
) -> serde_json::Value {
    let samples = samples.into_iter().collect::<Vec<_>>();
    let distribution = |values: Vec<u64>| {
        let mut values = values;
        values.sort_unstable();
        json!({
            "medianNs": percentile(&values, 50),
            "p95Ns": percentile(&values, 95),
            "minNs": values.first().copied().unwrap_or(0),
            "maxNs": values.last().copied().unwrap_or(0),
        })
    };
    let durations = |select: fn(AnalysisTimings) -> std::time::Duration| {
        distribution(samples.iter().copied().map(select).map(nanos).collect())
    };
    json!({
        "wallTotal": durations(|timing| timing.wall),
        "measuredTotal": durations(measured_pipeline_duration),
        "unattributedOrchestration": durations(|timing| timing.wall.saturating_sub(measured_pipeline_duration(timing))),
        "sourceAnalysis": durations(|timing| timing.facts.source_analysis),
        "sourceFilesReused": distribution(samples.iter().map(|timing| timing.facts.source_files_reused).collect()),
        "sourceFilesRecomputed": distribution(samples.iter().map(|timing| timing.facts.source_files_recomputed).collect()),
        "astFacts": durations(|timing| timing.facts.ast_facts),
        "compilerFacts": durations(|timing| timing.facts.compiler_facts),
        "fileFactAssembly": durations(|timing| timing.facts.file_fact_assembly),
        "reactiveIrTotal": durations(|timing| timing.reactive_ir.total),
        "reactiveIrCacheLookup": durations(|timing| timing.reactive_ir.cache_lookup),
        "reactiveIrReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.reused).count(),
        "sourceDiscoveryReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.source_discovery_reused_files).collect()),
        "sourceDiscoveryRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.source_discovery_recomputed_files).collect()),
        "typescriptIndexesReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.typescript_indexes_reused).count(),
        "reachabilityReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.reachability_reused).count(),
        "reachabilityReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.reachability_reused_files).collect()),
        "reachabilityRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.reachability_recomputed_files).collect()),
        "localAccessesReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.local_accesses_reused).count(),
        "localAccessReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.local_access_reused_files).collect()),
        "localAccessRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.local_access_recomputed_files).collect()),
        "interproceduralReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.interprocedural_reused).count(),
        "ownerFixedPointReusedSamples": samples.iter().filter(|timing| timing.reactive_ir.owner_fixed_point_reused).count(),
        "ownerReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.owner_reused_files).collect()),
        "ownerRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.owner_recomputed_files).collect()),
        "typedAccessorReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.typed_accessor_reused_files).collect()),
        "typedAccessorRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.typed_accessor_recomputed_files).collect()),
        "interproceduralGraphReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.interprocedural_graph_reused_files).collect()),
        "interproceduralGraphRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.interprocedural_graph_recomputed_files).collect()),
        "interproceduralResultReusedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.interprocedural_result_reused_files).collect()),
        "interproceduralResultRecomputedFiles": distribution(samples.iter().map(|timing| timing.reactive_ir.interprocedural_result_recomputed_files).collect()),
        "reactiveIr": {
            "indexesAndReachability": durations(|timing| timing.reactive_ir.indexes_and_reachability),
            "projectIndexes": durations(|timing| timing.reactive_ir.project_indexes),
            "aliasAndEntityIndexes": durations(|timing| timing.reactive_ir.alias_and_entity_indexes),
            "aliasRoots": durations(|timing| timing.reactive_ir.alias_roots),
            "entitySymbols": durations(|timing| timing.reactive_ir.entity_symbols),
            "symbolNameIndexes": durations(|timing| timing.reactive_ir.symbol_name_indexes),
            "contractResolution": durations(|timing| timing.reactive_ir.contract_resolution),
            "reachability": durations(|timing| timing.reactive_ir.reachability),
            "sourceDiscovery": durations(|timing| timing.reactive_ir.source_discovery),
            "typedAccessorsAndPropRoots": durations(|timing| timing.reactive_ir.typed_accessors_and_prop_roots),
            "propPropagationAndControlFlow": durations(|timing| timing.reactive_ir.prop_propagation_and_control_flow),
            "staticPrepass": durations(|timing| timing.reactive_ir.static_prepass),
            "localAndInterprocedural": durations(|timing| timing.reactive_ir.local_and_interprocedural),
            "localReadsAndWrites": durations(|timing| timing.reactive_ir.local_reads_and_writes),
            "interproceduralSummaries": durations(|timing| timing.reactive_ir.interprocedural_summaries),
            "interproceduralGraph": durations(|timing| timing.reactive_ir.interprocedural_graph),
            "interproceduralDirectSummaries": durations(|timing| timing.reactive_ir.interprocedural_direct_summaries),
            "interproceduralDirectIndex": durations(|timing| timing.reactive_ir.interprocedural_direct_index),
            "interproceduralDirectReferences": durations(|timing| timing.reactive_ir.interprocedural_direct_references),
            "interproceduralTypedAccessors": durations(|timing| timing.reactive_ir.interprocedural_typed_accessors),
            "interproceduralPropagation": durations(|timing| timing.reactive_ir.interprocedural_propagation),
            "interproceduralReturnedDirect": durations(|timing| timing.reactive_ir.interprocedural_returned_direct),
            "interproceduralReturnedDelta": durations(|timing| timing.reactive_ir.interprocedural_returned_delta),
            "interproceduralCallSummaryDelta": durations(|timing| timing.reactive_ir.interprocedural_call_summary_delta),
            "interproceduralFactoryPropagation": durations(|timing| timing.reactive_ir.interprocedural_factory_propagation),
            "interproceduralResultsAndExports": durations(|timing| timing.reactive_ir.interprocedural_results_and_exports),
            "interproceduralResultReads": durations(|timing| timing.reactive_ir.interprocedural_result_reads),
            "interproceduralExportSummaries": durations(|timing| timing.reactive_ir.interprocedural_export_summaries),
            "leafAndCleanup": durations(|timing| timing.reactive_ir.leaf_and_cleanup),
            "staticApi": durations(|timing| timing.reactive_ir.static_api),
            "directives": durations(|timing| timing.reactive_ir.directives),
            "ownerFixedPoint": durations(|timing| timing.reactive_ir.owner_fixed_point),
            "ownerFragmentBuild": durations(|timing| timing.reactive_ir.owner_fragment_build),
            "ownerGraphAssembly": durations(|timing| timing.reactive_ir.owner_graph_assembly),
            "ownerPropagation": durations(|timing| timing.reactive_ir.owner_propagation),
            "ownerRequirementEmission": durations(|timing| timing.reactive_ir.owner_requirement_emission),
            "finalOrdering": durations(|timing| timing.reactive_ir.final_ordering),
        },
        "solverTotal": durations(|timing| timing.solver.total),
        "solverFindingConstruction": durations(|timing| timing.solver.finding_construction),
        "solverFinalOrdering": durations(|timing| timing.solver.final_ordering),
    })
}

fn parse_args() -> Result<Options, Box<dyn std::error::Error>> {
    let mut project = PathBuf::from("tsconfig.json");
    let mut typefacts = env::var("SOLID_TYPEFACTS_BIN").unwrap_or_default();
    let mut iterations = 20_usize;
    let mut warmups = 3_usize;
    let mut edit = None;
    let mut edit_mode = EditMode::Prefix;
    let mut limits = TransferLimits::default();
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < arguments.len() {
        let argument = &arguments[index];
        let value = |index: &mut usize| -> Result<&str, Box<dyn std::error::Error>> {
            *index += 1;
            arguments
                .get(*index)
                .map(String::as_str)
                .ok_or_else(|| format!("{argument} requires a value").into())
        };
        match argument.as_str() {
            "--project" => project = value(&mut index)?.into(),
            "--typefacts" => typefacts = value(&mut index)?.into(),
            "--iterations" => iterations = value(&mut index)?.parse()?,
            "--warmups" => warmups = value(&mut index)?.parse()?,
            "--edit" => edit = Some(PathBuf::from(value(&mut index)?)),
            "--edit-mode" => {
                edit_mode = match value(&mut index)? {
                    "prefix" => EditMode::Prefix,
                    "same-span-body" => EditMode::SameSpanBody,
                    mode => return Err(format!("unsupported edit mode {mode:?}").into()),
                }
            }
            "--max-first-request-bytes" => {
                limits.first_request_bytes = Some(value(&mut index)?.parse()?)
            }
            "--max-first-response-bytes" => {
                limits.first_response_bytes = Some(value(&mut index)?.parse()?)
            }
            "--max-sample-request-bytes" => {
                limits.sample_request_bytes = Some(value(&mut index)?.parse()?)
            }
            "--max-sample-response-bytes" => {
                limits.sample_response_bytes = Some(value(&mut index)?.parse()?)
            }
            "-h" | "--help" => {
                println!(
                    "Usage: solid-checker-session-bench [OPTIONS]\n\n\
                     --project <PATH>       TypeScript project\n\
                     --typefacts <PATH>     TypeFacts service executable\n\
                     --iterations <COUNT>   Measured iterations (default: 20)\n\
                     --warmups <COUNT>      Warm-up iterations (default: 3)\n\
                     --edit <PATH>          Alternate an in-memory edit before each analysis\n\
                     --edit-mode <MODE>     prefix (default) or same-span-body\n\
                     --max-first-request-bytes <BYTES>\n\
                     --max-first-response-bytes <BYTES>\n\
                     --max-sample-request-bytes <BYTES>\n\
                     --max-sample-response-bytes <BYTES>"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown option {argument:?}").into()),
        }
        index += 1;
    }
    if typefacts.is_empty() {
        return Err("--typefacts or SOLID_TYPEFACTS_BIN is required".into());
    }
    if iterations == 0 {
        return Err("--iterations must be non-zero".into());
    }
    Ok(Options {
        project,
        typefacts,
        iterations,
        warmups,
        edit,
        edit_mode,
        limits,
    })
}
