mod daemon_cache;
mod idle_memory;
mod json_output;
mod snapshot_emission;

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use solid_facts_backend::{
    BackendError, ImportIdentityMeasurement, RequestedRuleEnablement, SemanticDemandOptions,
    SourceFile, TypeFactsProvider, TypeFactsSession, accepted_package_contract_statuses,
    analyze_project_accepted_measured_with_enablement, attest_import_identities,
    build_project_native_measured_with_demands, bundled_first_party_contract_index,
    contract_identity_scope, default_typefacts_executable, dialect,
    encode_inferred_contract_workflow, merge_contract_proposals, merge_plans,
    read_accepted_contract_catalog, review_contract_document,
    semantic_demand_options_for_enablement, validate_contract_document,
};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeRendering, RuntimeTarget};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    project_id: String,
    /// Absent means "detect from the project's resolved solid-js"; the
    /// default dialect is the fallback when nothing resolves.
    #[serde(default)]
    dialect: Option<String>,
    generation: u64,
    sources: Vec<SourceFile>,
    typefacts_executable: String,
    #[serde(default)]
    typefacts_args: Vec<String>,
    /// Host-acquired exact imports paired with stable-v1 main documents
    /// and proof-issued receipts.
    #[serde(default)]
    accepted_contract_catalog: String,
    #[serde(default)]
    presets: Vec<String>,
    #[serde(default)]
    enable_rules: Vec<String>,
    #[serde(default = "json_format")]
    format: String,
    #[serde(default)]
    certify: bool,
    #[serde(default)]
    check_contracts: bool,
    #[serde(default)]
    validate_contract_paths: Vec<String>,
    #[serde(default)]
    emit_contract: String,
    /// Private, non-contract input recipes for the runtime probe. This is a
    /// sidecar because constructing an argument is not evidence of behavior.
    /// Generator-owned declaration query used only to derive and validate
    /// probe construction recipes. It never enters contract inference.
    #[serde(default)]
    declaration_probe_plan: String,
    /// Where to write the analyzing program's own module inventory, as JSON.
    ///
    /// The generator's runtime-module closure is a syntax walk in *its*
    /// process; this is the file list the program in *this* process actually
    /// opened. Asking for it turns the generator's closure record from a
    /// reconstruction into an attestation, and makes the walk's own output
    /// checkable against it.
    ///
    /// Only a generation run asks: it is a read of a program that is already
    /// built, but it is still two round trips, and an ordinary analysis has no
    /// consumer for the answer.
    #[serde(default)]
    emit_module_inventory: String,
    /// Generator-owned JSON containing exact static
    /// importer/specifier/runtime-target triples for this package analysis.
    /// Empty outside package-contract generation.
    #[serde(default)]
    runtime_module_resolutions: String,
    /// Full Phase-7 exact package resolution for the artifact being emitted.
    #[serde(default)]
    contract_resolution: String,
    #[serde(default)]
    emit_proposal_plan: String,
    #[serde(default)]
    merge_contract_paths: Vec<String>,
    #[serde(default)]
    merge_contract_output: String,
    #[serde(default)]
    merge_proposal_plan_paths: Vec<String>,
    #[serde(default)]
    merge_proposal_plan_output: String,
    #[serde(default)]
    review_contract: String,
    #[serde(default)]
    review_output: String,
    #[serde(default)]
    runtime_probe_proposal: String,
    #[serde(default)]
    runtime_probe_proposal_plan: String,
    #[serde(default)]
    runtime_probe_request: String,
    #[serde(default)]
    runtime_probe_plan_output: String,
    #[serde(default)]
    runtime_probe_runs: String,
    #[serde(default)]
    runtime_probe_evaluation_output: String,
    #[serde(default)]
    package_name: String,
    #[serde(default)]
    package_version: String,
    #[serde(default)]
    contract_entry_file: String,
    #[serde(default)]
    contract_package_root: String,
    #[serde(default)]
    help: bool,
    #[serde(default)]
    serve: bool,
    #[serde(default)]
    runtime: RuntimeEnvironment,
}

/// Strips the `-project <path>` pair the session now supplies itself, leaving
/// only producer-specific flags.
fn producer_arguments(arguments: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        if argument == "-project" {
            let _ = rest.next();
            continue;
        }
        kept.push(argument.clone());
    }
    kept
}

fn json_format() -> String {
    "json".into()
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let started = Instant::now();
    #[cfg(feature = "dialect-v2")]
    {
        let arguments = std::env::args().collect::<Vec<_>>();
        if arguments.len() == 2
            && solid_facts_backend::is_compiler_certification_session_argument(&arguments[1])
        {
            solid_facts_backend::serve_compiler_certification_session()?;
            return Ok(0);
        }
    }
    // A JSON request arrives on stdin only when the caller passed no
    // arguments; argv invocations must not block waiting for stdin EOF.
    let mut encoded = String::new();
    if std::env::args().len() <= 1 && !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut encoded)?;
    }
    let mut request: Request = if encoded.trim().is_empty() {
        request_from_args()?
    } else {
        serde_json::from_str(&encoded)?
    };
    if request.help {
        print_help();
        return Ok(0);
    }
    request.presets.sort();
    request.presets.dedup();
    request.enable_rules.sort();
    request.enable_rules.dedup();
    // The inventory attests *the generation run's* program. Asking for one
    // without asking for a contract would hand a caller an attestation with
    // nothing to attest, so it is refused rather than silently written.
    if !request.emit_module_inventory.is_empty() && request.emit_contract.is_empty() {
        return Err("--emit-module-inventory requires --emit-contract".into());
    }
    if !request.runtime_module_resolutions.is_empty() && request.emit_contract.is_empty() {
        return Err("--runtime-module-resolutions requires --emit-contract".into());
    }
    if !request.runtime_module_resolutions.is_empty() && request.contract_package_root.is_empty() {
        return Err("--runtime-module-resolutions requires --contract-package-root".into());
    }
    if !request.emit_contract.is_empty()
        && (request.contract_resolution.is_empty() || request.emit_proposal_plan.is_empty())
    {
        return Err(
            "--emit-contract requires --contract-resolution and --emit-proposal-plan".into(),
        );
    }
    let dialect = match request.dialect.as_deref() {
        Some(id) => dialect::by_id(id).ok_or_else(|| {
            format!(
                "unknown dialect {id:?}; known dialects: {}",
                dialect::ALL
                    .iter()
                    .map(|dialect| dialect.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?,
        None => dialect::detect(if request.contract_package_root.is_empty() {
            Path::new(&request.project_id)
        } else {
            Path::new(&request.contract_package_root)
        }),
    };
    if !request.validate_contract_paths.is_empty() {
        for path in &request.validate_contract_paths {
            validate_contract_document(&fs::read(path)?)?;
        }
        return Ok(0);
    }
    if !request.merge_contract_paths.is_empty() || !request.merge_contract_output.is_empty() {
        if request.merge_contract_paths.is_empty() || request.merge_contract_output.is_empty() {
            return Err(
                "--merge-contract and --merge-contract-output are required together".into(),
            );
        }
        let documents = request
            .merge_contract_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()?;
        let merged = merge_contract_proposals(documents.iter().map(Vec::as_slice), true)?;
        fs::write(&request.merge_contract_output, &merged)?;
        if request.merge_proposal_plan_paths.is_empty()
            || request.merge_proposal_plan_output.is_empty()
        {
            return Err(
                "proposal merge requires --merge-proposal-plan and --merge-proposal-plan-output"
                    .into(),
            );
        }
        let plans = request
            .merge_proposal_plan_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()?;
        fs::write(
            &request.merge_proposal_plan_output,
            merge_plans(&merged, plans)?,
        )?;
        return Ok(0);
    }
    if !request.review_contract.is_empty() || !request.review_output.is_empty() {
        if request.review_contract.is_empty() || request.review_output.is_empty() {
            return Err("--review-contract and --review-output are required together".into());
        }
        fs::write(
            &request.review_output,
            review_contract_document(&fs::read(&request.review_contract)?)?,
        )?;
        return Ok(0);
    }
    if !request.runtime_probe_proposal.is_empty()
        || !request.runtime_probe_proposal_plan.is_empty()
        || !request.runtime_probe_request.is_empty()
        || !request.runtime_probe_plan_output.is_empty()
        || !request.runtime_probe_runs.is_empty()
        || !request.runtime_probe_evaluation_output.is_empty()
    {
        if [
            &request.runtime_probe_proposal,
            &request.runtime_probe_proposal_plan,
            &request.runtime_probe_request,
            &request.runtime_probe_plan_output,
        ]
        .iter()
        .any(|value| value.is_empty())
        {
            return Err("runtime probe planning requires --runtime-probe-proposal, --runtime-probe-proposal-plan, --runtime-probe-request, and --runtime-probe-plan-output".into());
        }
        if request.runtime_probe_runs.is_empty()
            != request.runtime_probe_evaluation_output.is_empty()
        {
            return Err(
                "--runtime-probe-runs and --runtime-probe-evaluation-output are required together"
                    .into(),
            );
        }
        let planned = solid_facts_backend::plan_runtime_probes(
            &fs::read(&request.runtime_probe_proposal)?,
            &fs::read(&request.runtime_probe_proposal_plan)?,
            &fs::read(&request.runtime_probe_request)?,
        )?;
        fs::write(&request.runtime_probe_plan_output, planned.bytes())?;
        if !request.runtime_probe_runs.is_empty() {
            fs::write(
                &request.runtime_probe_evaluation_output,
                solid_facts_backend::evaluate_runtime_probe_runs(
                    &planned,
                    &fs::read(&request.runtime_probe_runs)?,
                )?,
            )?;
        }
        return Ok(0);
    }
    #[cfg(unix)]
    {
        if request.serve {
            return daemon::serve(&request);
        }
        if daemon::enabled() && daemon::eligible(&request) {
            match daemon::check(&request) {
                Ok(code) => return Ok(code),
                Err(error) => {
                    eprintln!("solid-checker: daemon unavailable ({error}); running one-shot");
                }
            }
        }
    }
    #[cfg(not(unix))]
    if request.serve {
        return Err("--serve requires a Unix platform".into());
    }
    let diagnostics = env!("CARGO_BIN_NAME") == "solid-checker-rust";
    let declaration_probe = if request.declaration_probe_plan.is_empty() {
        None
    } else {
        Some(prepare_declaration_probe(Path::new(
            &request.declaration_probe_plan,
        ))?)
    };
    // `Session::open` spawns the producer, verifies the compatibility
    // handshake, and opens the project. It returns as soon as the process is
    // live, so `sidecarSpawnNs` measures startup plus handshake rather than
    // the TypeScript program build — that lands on the first request needing
    // the built program, which is the source fetch below.
    let mut typescript = TypeFactsSession::open(
        &request.typefacts_executable,
        &request.project_id,
        &producer_arguments(&request.typefacts_args),
    )?;
    if let Some(probe) = declaration_probe {
        emit_declaration_probe_plan(&mut typescript, &probe)?;
        return Ok(0);
    }
    let sidecar_spawn_ns = started.elapsed().as_nanos();
    let mut sources_bytes = 0usize;
    let sources_wire_bytes = 0u64;
    if request.sources.is_empty() {
        request.sources = typescript.configured_sources()?;
        sources_bytes = request.sources.iter().map(|s| s.source.len()).sum();
    }
    let source_setup_ns = started.elapsed().as_nanos();
    let requested_enablement = RequestedRuleEnablement {
        presets: &request.presets,
        rules: &request.enable_rules,
        runtime: request.runtime.clone(),
    };
    let mut semantic_demand_options = if diagnostics {
        semantic_demand_options_for_enablement(
            dialect,
            Path::new(&request.project_id),
            requested_enablement.clone(),
        )?
    } else {
        SemanticDemandOptions::NONE
    };
    semantic_demand_options.contract_probe_parameters = !request.emit_contract.is_empty();
    let (mut facts, native_timings) = {
        let (facts, timings) = build_project_native_measured_with_demands(
            dialect,
            request.project_id.clone(),
            request.generation,
            request.sources.clone(),
            &mut typescript,
            semantic_demand_options,
        )?;
        (facts, Some(timings))
    };
    if !request.contract_package_root.is_empty() {
        let package_root = Path::new(&request.contract_package_root).canonicalize()?;
        if !package_root.is_dir() {
            return Err("--contract-package-root must be a package directory".into());
        }
        facts.project_id = package_root
            .join("tsconfig.json")
            .to_string_lossy()
            .into_owned();
    }
    if !request.runtime_module_resolutions.is_empty() {
        facts.runtime_symbol_redirects = runtime_symbol_redirects(
            &facts,
            &mut typescript,
            Path::new(&request.contract_package_root),
            Path::new(&request.runtime_module_resolutions),
        )?;
    }
    let facts_complete_ns = started.elapsed().as_nanos();
    // Contracts are bound to the installed package an import resolves to, not
    // to the specifier's name, so the analysis needs the compiler's own
    // resolution for every specifier a contract could describe. This is the
    // one-shot path's equivalent of what `NativeIncrementalSession::attested`
    // does for a retained session: the same scope rule, issued at the one point
    // where the session is live and the whole file set is known.
    //
    // `--check-contracts` needs it for the same reason and not merely for
    // symmetry: that report answers "is my contract coverage complete?", and a
    // contract every import refuses covers nothing. Skipping the attestation
    // there let the report call such a contract `published` while the analysis
    // refused it everywhere.
    let mut import_identity = ImportIdentityMeasurement::default();
    if diagnostics {
        let scope = contract_identity_scope(&facts);
        if !scope.is_empty() {
            let (index, measurement) = attest_import_identities(&mut typescript, &scope)?;
            facts.resolved_imports = Some(index);
            import_identity = measurement;
        }
    }
    let import_identity_ns = started
        .elapsed()
        .as_nanos()
        .saturating_sub(facts_complete_ns);
    if diagnostics && request.check_contracts {
        let project = Path::new(&facts.project_id);
        let directory = if project.is_dir() {
            project
        } else {
            project.parent().unwrap_or_else(|| Path::new("."))
        };
        let bundled =
            bundled_first_party_contract_index(dialect.id, directory, &facts, &request.runtime)?;
        let catalog = if request.accepted_contract_catalog.is_empty() {
            let candidate = directory.join(".solid-checker/accepted-contracts.json");
            candidate.is_file().then_some(candidate)
        } else {
            Some(PathBuf::from(&request.accepted_contract_catalog))
        };
        let contracts = catalog
            .as_deref()
            .map(read_accepted_contract_catalog)
            .transpose()?
            .unwrap_or_default()
            .with_fallback(bundled);
        let statuses = accepted_package_contract_statuses(dialect, project, &facts, &contracts)?;
        let actionable = statuses
            .iter()
            .filter(|status| status.needs_action())
            .collect::<Vec<_>>();
        let stale = statuses
            .iter()
            .filter(|status| status.status == "stale")
            .count();
        // The contract report has no "default" rendering distinct from text,
        // and `contract check` is meant to be run with no flags at all, so the
        // unspecified format resolves to text instead of failing.
        match request.format.as_str() {
            "json" => {
                let report = serde_json::json!({
                    "packages": statuses,
                    // `missing` keeps its original meaning: the number of
                    // packages whose contract cannot certify. `stale` breaks
                    // out the drift subset so CI can report it separately.
                    "missing": actionable.len(),
                    "stale": stale,
                });
                let mut stdout = io::stdout().lock();
                stdout.write_all(&json_output::go_compatible(&report, true)?)?;
                stdout.write_all(b"\n")?;
            }
            "text" | "default" => {
                for status in &statuses {
                    println!(
                        "{}: {} ({})",
                        status.name, status.status, status.contract_path
                    );
                    if let Some(detail) = &status.detail {
                        println!("  {detail}");
                    }
                    if let Some(remedy) = &status.remedy {
                        println!("  -> {remedy}");
                    }
                }
                if statuses.is_empty() {
                    println!("No imported Solid packages need contracts.");
                } else if actionable.is_empty() {
                    println!(
                        "\nEvery imported Solid package has a contract for its installed version."
                    );
                } else {
                    println!(
                        "\n{} of {} package contracts need action{}.",
                        actionable.len(),
                        statuses.len(),
                        if stale > 0 {
                            format!(" ({stale} stale)")
                        } else {
                            String::new()
                        }
                    );
                }
            }
            format => return Err(format!("unsupported format {format:?}").into()),
        }
        return Ok(i32::from(!actionable.is_empty()));
    }
    if diagnostics {
        let discovered_catalog = if request.accepted_contract_catalog.is_empty() {
            let project = Path::new(&facts.project_id);
            let directory = if project.is_dir() {
                project
            } else {
                project.parent().unwrap_or_else(|| Path::new("."))
            };
            let candidate = directory.join(".solid-checker/accepted-contracts.json");
            candidate.is_file().then_some(candidate)
        } else {
            Some(PathBuf::from(&request.accepted_contract_catalog))
        };
        let project = Path::new(&facts.project_id);
        let directory = if project.is_dir() {
            project
        } else {
            project.parent().unwrap_or_else(|| Path::new("."))
        };
        let bundled =
            bundled_first_party_contract_index(dialect.id, directory, &facts, &request.runtime)?;
        let contracts = discovered_catalog
            .as_deref()
            .map(read_accepted_contract_catalog)
            .transpose()?
            .unwrap_or_default()
            .with_fallback(bundled);
        let (analysis, diagnostic_timings) = analyze_project_accepted_measured_with_enablement(
            dialect,
            Path::new(&facts.project_id),
            &request.sources,
            &facts,
            &contracts,
            requested_enablement,
        )?;
        let contract_emission_ns = Cell::new(0_u128);
        let module_inventory_ns = Cell::new(0_u128);
        let report_timings = || {
            if std::env::var_os("SOLID_CHECKER_TIMINGS").is_none() {
                return;
            }
            let (source_analysis_ns, type_facts_ns) = native_timings.map_or((0, 0), |timings| {
                (
                    timings.source_analysis.as_nanos(),
                    timings.type_facts.as_nanos(),
                )
            });
            eprintln!(
                "{}",
                serde_json::json!({
                    "sidecarSpawnNs": sidecar_spawn_ns,
                    "sourcesFetchNs": source_setup_ns.saturating_sub(sidecar_spawn_ns),
                    "sourcesBytes": sources_bytes,
                    "sourcesWireBytes": sources_wire_bytes,
                    "sourceSetupNs": source_setup_ns,
                    "sourceAnalysisNs": source_analysis_ns,
                    "typeFactsNs": type_facts_ns,
                    "factsTotalNs": facts_complete_ns.saturating_sub(source_setup_ns),
                    "importIdentityNs": import_identity_ns,
                    "importIdentityFilesRequested": import_identity.requested,
                    "importIdentityFilesAttested": import_identity.attested,
                    "importIdentityFilesUnknown": import_identity.unknown,
                    "importIdentitySpecifiers": import_identity.specifiers,
                    "importIdentityModules": import_identity.modules,
                    "contractBindingsBound": analysis.program.contract_binding.bound,
                    "contractBindingsRefused": analysis.program.contract_binding.refused,
                    "irNs": diagnostic_timings.reactive_ir.as_nanos(),
                    "solveAndSnapshotNs": diagnostic_timings.solve_and_snapshot.as_nanos(),
                    "contractEmissionNs": contract_emission_ns.get(),
                    "moduleInventoryNs": module_inventory_ns.get(),
                    "totalNs": started.elapsed().as_nanos(),
                })
            );
        };
        if !request.emit_contract.is_empty() {
            let phase_started = Instant::now();
            emit_package_contract(&request, &analysis.program, &facts)?;
            contract_emission_ns.set(phase_started.elapsed().as_nanos());
            // After the contract, not before: a run that cannot emit a
            // contract has no closure record to attest, and writing the
            // inventory first would leave an attestation of a generation that
            // produced nothing.
            if !request.emit_module_inventory.is_empty() {
                let phase_started = Instant::now();
                write_module_inventory(&mut typescript, &request)?;
                module_inventory_ns.set(phase_started.elapsed().as_nanos());
            }
            // Emission normally produces no stdout, and the generator depends
            // on that. `--format json` is a caller explicitly asking for the
            // diagnostics of the same analysis, which is otherwise only
            // obtainable by running the whole project a second time -- and the
            // second run is the one that cannot see which obligations the
            // emitter attributed to which export. The default format is
            // untouched, so the generator's process contract is unchanged.
            if request.format != "json" {
                report_timings();
                return Ok(0);
            }
        }
        let snapshot = &analysis.snapshot;
        let emission = snapshot_emission::emit(
            dialect,
            &request.format,
            &request.project_id,
            snapshot,
            request.certify,
            started.elapsed(),
        )?;
        io::stdout().write_all(&emission.output)?;
        report_timings();
        return Ok(emission.exit_code);
    } else {
        serde_json::to_writer(io::stdout(), &facts)?;
    }
    Ok(0)
}

fn request_from_args() -> Result<Request, Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mut project = PathBuf::from("tsconfig.json");
    let mut typefacts = default_typefacts_executable();
    let mut dialect_id: Option<String> = None;
    let mut accepted_contract_catalog = String::new();
    let mut presets = Vec::new();
    let mut enable_rules = Vec::new();
    let mut format = "default".to_owned();
    let mut certify = false;
    let mut check_contracts = false;
    let mut validate_contract_paths = Vec::new();
    let mut emit_contract = String::new();
    let mut declaration_probe_plan = String::new();
    let mut emit_module_inventory = String::new();
    let mut runtime_module_resolutions = String::new();
    let mut contract_resolution = String::new();
    let mut emit_proposal_plan = String::new();
    let mut merge_contract_paths = Vec::new();
    let mut merge_contract_output = String::new();
    let mut merge_proposal_plan_paths = Vec::new();
    let mut merge_proposal_plan_output = String::new();
    let mut review_contract = String::new();
    let mut review_output = String::new();
    let mut runtime_probe_proposal = String::new();
    let mut runtime_probe_proposal_plan = String::new();
    let mut runtime_probe_request = String::new();
    let mut runtime_probe_plan_output = String::new();
    let mut runtime_probe_runs = String::new();
    let mut runtime_probe_evaluation_output = String::new();
    let mut package_name = String::new();
    let mut package_version = String::new();
    let mut contract_entry_file = String::new();
    let mut contract_package_root = String::new();
    let mut runtime = RuntimeEnvironment::default();
    let mut help = false;
    let mut serve = false;
    let mut args = arguments.into_iter();
    while let Some(argument) = args.next() {
        if let Some(value) = argument.strip_prefix("--project=") {
            project = PathBuf::from(value);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--dialect=") {
            dialect_id = Some(value.to_owned());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--typefacts=") {
            typefacts = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--accepted-contracts=") {
            accepted_contract_catalog = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--preset=") {
            presets.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--enable-rule=") {
            enable_rules.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--format=") {
            format = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--validate-contract=") {
            validate_contract_paths.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--emit-contract=") {
            emit_contract = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--declaration-probe-plan=") {
            declaration_probe_plan = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--emit-module-inventory=") {
            emit_module_inventory = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-module-resolutions=") {
            runtime_module_resolutions = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--contract-resolution=") {
            contract_resolution = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--emit-proposal-plan=") {
            emit_proposal_plan = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-contract=") {
            merge_contract_paths.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-contract-output=") {
            merge_contract_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-proposal-plan=") {
            merge_proposal_plan_paths.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-proposal-plan-output=") {
            merge_proposal_plan_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--review-contract=") {
            review_contract = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--review-output=") {
            review_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--package-name=") {
            package_name = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-proposal=") {
            runtime_probe_proposal = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-proposal-plan=") {
            runtime_probe_proposal_plan = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-request=") {
            runtime_probe_request = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-plan-output=") {
            runtime_probe_plan_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-runs=") {
            runtime_probe_runs = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-evaluation-output=") {
            runtime_probe_evaluation_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--package-version=") {
            package_version = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--contract-entry-file=") {
            contract_entry_file = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--contract-package-root=") {
            contract_package_root = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-target=") {
            runtime.target = Some(parse_runtime_target(value)?);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-build=") {
            runtime.build = Some(parse_runtime_build(value)?);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--rendering=") {
            runtime.rendering = Some(parse_runtime_rendering(value)?);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-condition=") {
            runtime.conditions.insert(value.to_owned());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-conditions=") {
            runtime.conditions.extend(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|condition| !condition.is_empty())
                    .map(str::to_owned),
            );
            continue;
        }
        if let Some(value) = argument.strip_prefix("--framework-transform=") {
            runtime.framework_transforms.insert(value.to_owned());
            continue;
        }
        match argument.as_str() {
            "--project" | "-project" => {
                project = PathBuf::from(args.next().ok_or("--project needs a path")?)
            }
            "--typefacts" => typefacts = args.next().ok_or("--typefacts needs a path")?,
            "--dialect" => dialect_id = Some(args.next().ok_or("--dialect needs an id")?),
            "--accepted-contracts" => {
                accepted_contract_catalog =
                    args.next().ok_or("--accepted-contracts needs a path")?
            }
            "--preset" => presets.push(args.next().ok_or("--preset needs a name")?),
            "--enable-rule" => {
                enable_rules.push(args.next().ok_or("--enable-rule needs a rule name")?)
            }
            "--format" => format = args.next().ok_or("--format needs a value")?,
            "--certify" => certify = true,
            "--check-contracts" => check_contracts = true,
            "--serve" => serve = true,
            "--help" | "-h" => help = true,
            "--validate-contract" => {
                validate_contract_paths.push(args.next().ok_or("--validate-contract needs a path")?)
            }
            "--emit-contract" => {
                emit_contract = args.next().ok_or("--emit-contract needs a path")?
            }
            "--declaration-probe-plan" => {
                declaration_probe_plan =
                    args.next().ok_or("--declaration-probe-plan needs a path")?
            }
            "--emit-module-inventory" => {
                emit_module_inventory = args.next().ok_or("--emit-module-inventory needs a path")?
            }
            "--runtime-module-resolutions" => {
                runtime_module_resolutions = args
                    .next()
                    .ok_or("--runtime-module-resolutions needs a path")?
            }
            "--contract-resolution" => {
                contract_resolution = args.next().ok_or("--contract-resolution needs a path")?
            }
            "--emit-proposal-plan" => {
                emit_proposal_plan = args.next().ok_or("--emit-proposal-plan needs a path")?
            }
            "--merge-contract" => {
                merge_contract_paths.push(args.next().ok_or("--merge-contract needs a path")?)
            }
            "--merge-contract-output" => {
                merge_contract_output = args.next().ok_or("--merge-contract-output needs a path")?
            }
            "--merge-proposal-plan" => merge_proposal_plan_paths
                .push(args.next().ok_or("--merge-proposal-plan needs a path")?),
            "--merge-proposal-plan-output" => {
                merge_proposal_plan_output = args
                    .next()
                    .ok_or("--merge-proposal-plan-output needs a path")?
            }
            "--review-contract" => {
                review_contract = args.next().ok_or("--review-contract needs a path")?
            }
            "--review-output" => {
                review_output = args.next().ok_or("--review-output needs a path")?
            }
            "--runtime-probe-proposal" => {
                runtime_probe_proposal =
                    args.next().ok_or("--runtime-probe-proposal needs a path")?
            }
            "--runtime-probe-proposal-plan" => {
                runtime_probe_proposal_plan = args
                    .next()
                    .ok_or("--runtime-probe-proposal-plan needs a path")?
            }
            "--runtime-probe-request" => {
                runtime_probe_request = args.next().ok_or("--runtime-probe-request needs a path")?
            }
            "--runtime-probe-plan-output" => {
                runtime_probe_plan_output = args
                    .next()
                    .ok_or("--runtime-probe-plan-output needs a path")?
            }
            "--runtime-probe-runs" => {
                runtime_probe_runs = args.next().ok_or("--runtime-probe-runs needs a path")?
            }
            "--runtime-probe-evaluation-output" => {
                runtime_probe_evaluation_output = args
                    .next()
                    .ok_or("--runtime-probe-evaluation-output needs a path")?
            }
            "--package-name" => package_name = args.next().ok_or("--package-name needs a value")?,
            "--package-version" => {
                package_version = args.next().ok_or("--package-version needs a value")?
            }
            "--contract-entry-file" => {
                contract_entry_file = args.next().ok_or("--contract-entry-file needs a path")?
            }
            "--contract-package-root" => {
                contract_package_root = args.next().ok_or("--contract-package-root needs a path")?
            }
            "--runtime-target" => {
                runtime.target = Some(parse_runtime_target(
                    &args.next().ok_or("--runtime-target needs a value")?,
                )?)
            }
            "--runtime-build" => {
                runtime.build = Some(parse_runtime_build(
                    &args.next().ok_or("--runtime-build needs a value")?,
                )?)
            }
            "--rendering" => {
                runtime.rendering = Some(parse_runtime_rendering(
                    &args.next().ok_or("--rendering needs a value")?,
                )?)
            }
            "--program-boundary" => {
                runtime.program_boundary = Some(parse_program_boundary(
                    &args.next().ok_or("--program-boundary needs a value")?,
                )?)
            }
            "--runtime-condition" => {
                runtime
                    .conditions
                    .insert(args.next().ok_or("--runtime-condition needs a value")?);
            }
            "--runtime-conditions" => {
                runtime.conditions.extend(
                    args.next()
                        .ok_or("--runtime-conditions needs a comma-separated value")?
                        .split(',')
                        .map(str::trim)
                        .filter(|condition| !condition.is_empty())
                        .map(str::to_owned),
                );
            }
            "--framework-transform" => {
                runtime
                    .framework_transforms
                    .insert(args.next().ok_or("--framework-transform needs a value")?);
            }
            unknown => return Err(format!("unknown argument {unknown:?}").into()),
        }
    }
    let project = if !help
        && validate_contract_paths.is_empty()
        && merge_contract_paths.is_empty()
        && merge_contract_output.is_empty()
        && review_contract.is_empty()
        && runtime_probe_proposal.is_empty()
    {
        project.canonicalize()?
    } else {
        project
    };
    presets.sort();
    presets.dedup();
    enable_rules.sort();
    enable_rules.dedup();
    Ok(Request {
        project_id: project.to_string_lossy().into_owned(),
        dialect: dialect_id,
        generation: 1,
        sources: vec![],
        typefacts_executable: typefacts,
        typefacts_args: vec!["-project".into(), project.to_string_lossy().into_owned()],
        accepted_contract_catalog,
        presets,
        enable_rules,
        format,
        certify,
        check_contracts,
        validate_contract_paths,
        emit_contract,
        declaration_probe_plan,
        emit_module_inventory,
        runtime_module_resolutions,
        contract_resolution,
        emit_proposal_plan,
        merge_contract_paths,
        merge_contract_output,
        merge_proposal_plan_paths,
        merge_proposal_plan_output,
        review_contract,
        review_output,
        runtime_probe_proposal,
        runtime_probe_proposal_plan,
        runtime_probe_request,
        runtime_probe_plan_output,
        runtime_probe_runs,
        runtime_probe_evaluation_output,
        package_name,
        package_version,
        contract_entry_file,
        contract_package_root,
        help,
        serve,
        runtime,
    })
}

fn parse_runtime_target(value: &str) -> Result<RuntimeTarget, Box<dyn std::error::Error>> {
    match value {
        "browser" => Ok(RuntimeTarget::Browser),
        "node" => Ok(RuntimeTarget::Node),
        _ => Err(format!("unknown runtime target {value:?}; expected browser or node").into()),
    }
}

fn parse_runtime_build(value: &str) -> Result<RuntimeBuild, Box<dyn std::error::Error>> {
    match value {
        "development" => Ok(RuntimeBuild::Development),
        "production" => Ok(RuntimeBuild::Production),
        _ => Err(
            format!("unknown runtime build {value:?}; expected development or production").into(),
        ),
    }
}

fn parse_runtime_rendering(value: &str) -> Result<RuntimeRendering, Box<dyn std::error::Error>> {
    match value {
        "csr" => Ok(RuntimeRendering::Csr),
        "string-ssr" => Ok(RuntimeRendering::StringSsr),
        "streaming-ssr" => Ok(RuntimeRendering::StreamingSsr),
        _ => Err(format!(
            "unknown rendering mode {value:?}; expected csr, string-ssr, or streaming-ssr"
        )
        .into()),
    }
}

fn parse_program_boundary(
    value: &str,
) -> Result<solid_reactive_ir::ProgramBoundary, Box<dyn std::error::Error>> {
    match value {
        "open" => Ok(solid_reactive_ir::ProgramBoundary::Open),
        "closed" => Ok(solid_reactive_ir::ProgramBoundary::Closed),
        _ => Err(format!("unknown program boundary {value:?}; expected open or closed").into()),
    }
}

fn print_help() {
    println!(
        "Usage: solid-checker-rust [OPTIONS]\n\
         \n\
         Options:\n\
           --project <PATH>             TypeScript project (default: tsconfig.json)\n\
           --format <default|text|json> Output format (default: default)\n\
           --dialect <ID>               Solid dialect (default: detect from solid-js; fallback: solid-v2)\n\
           --certify                    Exit 1 unless the project is certified\n\
           --check-contracts            Report imported Solid packages whose contract is\n\
                                        missing, unverified, or stale (audited against a\n\
                                        version this project no longer installs)\n\
           --accepted-contracts <PATH>  Load a host-acquired catalog of stable-v1\n\
                                        documents, proof receipts, and exact resolved imports\n\
           --preset <NAME>              Enable a catalog preset (repeatable)\n\
           --enable-rule <NAME>         Explicitly enable one rule (repeatable)\n\
           --runtime-target <browser|node>\n\
                                        Select the runtime target explicitly\n\
           --runtime-build <development|production>\n\
                                        Select the build mode explicitly\n\
           --rendering <csr|string-ssr|streaming-ssr>\n\
                                        Select the rendering mode explicitly\n\
           --runtime-condition <NAME>   Select an exact package/runtime condition\n\
           --framework-transform <NAME> Select an explicit framework/compiler transform\n\
           --program-boundary <open|closed>\n\
                                        Assert whether code outside this project may import\n\
                                        from it. `closed` lets an exported symbol's caller set\n\
                                        be enumerated; it never licenses guessing one\n\
           --validate-contract <PATH>   Validate a contract and artifact hashes\n\
           --emit-contract <PATH>       Write a generated solid-reactivity.json contract.\n\
                                        With --format json the same analysis also writes\n\
                                        its diagnostics to stdout\n\
           --emit-module-inventory <PATH>\n\
                                        Write the analyzing program's own module inventory\n\
                                        beside the emitted contract: the files it included\n\
                                        and where each package-local specifier resolved.\n\
                                        Requires --emit-contract\n\
           --runtime-module-resolutions <PATH>\n\
                                        Exact package-local ESM resolution map used to seed\n\
                                        contract analysis. Requires --emit-contract\n\
           --contract-resolution <PATH> Full exact package/artifact resolution record used\n\
                                        to bind a stable-v1 proposal. Required by\n\
                                        --emit-contract\n\
           --package-name <NAME>        Package name used by --emit-contract\n\
           --package-version <VERSION>  Exact package version used by --emit-contract\n\
           --typefacts <PATH>           TypeFacts service executable\n\
           --serve                      Run the retained per-project check daemon (Unix only).\n\
                                        Release checks use it by default; set\n\
                                        SOLID_CHECKER_DAEMON=0 for one-shot analysis.\n\
           -h, --help                   Print help"
    );
}

#[derive(Clone, Copy)]
struct UnresolvedClaimDomains {
    reactive_reads: bool,
    returns: bool,
    callbacks: bool,
    owner_requirements: bool,
    async_behavior: bool,
}

impl UnresolvedClaimDomains {
    const fn all() -> Self {
        Self {
            reactive_reads: true,
            returns: true,
            callbacks: true,
            owner_requirements: true,
            async_behavior: true,
        }
    }

    /// The contract field names these domains write, so the attribution note
    /// and the review plan's `unknown-sentinel` items name the same fields.
    fn names(self) -> Vec<&'static str> {
        [
            ("reactiveReads", self.reactive_reads),
            ("returns", self.returns),
            ("callbacks", self.callbacks),
            ("ownerRequirements", self.owner_requirements),
            ("asyncBehavior", self.async_behavior),
        ]
        .into_iter()
        .filter_map(|(name, enabled)| enabled.then_some(name))
        .collect()
    }
}

fn unresolved_claim_domains(kind: &solid_reactive_ir::StaticDefectKind) -> UnresolvedClaimDomains {
    use solid_reactive_ir::StaticDefectKind;
    match kind {
        // A proven reactive source was handed to a callee with no inspectable
        // body and no contract row. `reactiveReads`, because whether the callee
        // subscribes is exactly what is unproven. And `returns`, for the same
        // reason `ReactiveDispatchUnresolved` needs it: what that callee hands
        // back is described from the local accessor index, which knows nothing
        // about it, so a possibly-reactive property placed in the returned
        // object is emitted as a certified-negative omission
        // (`const derived = observe(value); return { derived }`).
        //
        // Reads-only was not provable here. Every shape that reaches this arm
        // in generation today also raises the package's missing-contract-export
        // obligation, which already erases all five domains
        // (fixtures/package-contracts/uncaptured-source-return pins that), so
        // the reads-only claim was never *tested* -- it was masked. Being
        // covered by another obligation in the shapes one can build is not a
        // proof that no shape escapes it, and the escaping direction publishes
        // a wrong `returns`.
        StaticDefectKind::ReactiveSourceUncaptured { .. } => UnresolvedClaimDomains {
            reactive_reads: true,
            returns: true,
            callbacks: false,
            owner_requirements: false,
            async_behavior: false,
        },
        StaticDefectKind::ReactiveCallbackUnresolved { .. }
        | StaticDefectKind::UnknownCallbackExecution { .. } => UnresolvedClaimDomains {
            reactive_reads: false,
            returns: false,
            callbacks: true,
            owner_requirements: false,
            async_behavior: false,
        },
        StaticDefectKind::StructuredReturnUnresolved { .. } => UnresolvedClaimDomains {
            reactive_reads: false,
            returns: true,
            callbacks: false,
            owner_requirements: false,
            async_behavior: false,
        },
        // An unresolved dispatch proves exactly one thing: the possible runtime
        // implementations do not share one reactive-read summary. Two domains
        // depend on that and no more.
        //
        // `reactiveReads`, because that is the summary the obligation says is
        // unproven. `returns`, because the return description is derived from
        // the same resolved callee summary and does *not* fail closed on its
        // own: a value produced by the unresolved dispatch and placed in a
        // returned object is described from the local accessor index, which
        // knows nothing about it, so a possibly-reactive property is emitted as
        // a certified-negative omission. (`StructuredReturnUnresolved` looks
        // like the guard for that, but it fires only for a shorthand property
        // bound to an import with no project declaration -- an orthogonal
        // condition that this shape does not meet. Pinned by the
        // fixtures/package-contracts/unresolved-dispatch-attribution and
        // unresolved-dispatch-domains-control pair: the control resolves the
        // dispatch and the contract then claims `returns.properties.value` is
        // an accessor, which is precisely the claim the unresolved variant
        // cannot make.)
        //
        // `callbacks`, `ownerRequirements` and `asyncBehavior` are proven by
        // passes that do not consult the dispatch, and erasing them here was
        // discarding four independently established claims to record one.
        StaticDefectKind::ReactiveDispatchUnresolved { .. } => UnresolvedClaimDomains {
            reactive_reads: true,
            returns: true,
            callbacks: false,
            owner_requirements: false,
            async_behavior: false,
        },
        // A missing or environment-dependent contract export says nothing at
        // all about the surface behind it, so every domain stays unknown.
        _ => UnresolvedClaimDomains::all(),
    }
}

/// Marks the requested domains unknown, reporting whether anything changed.
///
/// A value export has no claim to erase, and a caller that recorded it as
/// marked would put an attribution note on the review plan for a decision the
/// contract does not contain.
fn mark_summary_claims_unknown(
    summary: &mut solid_reactive_ir::ContractExport,
    domains: UnresolvedClaimDomains,
) -> bool {
    let mut marked = false;
    if summary.kind != "function" {
        return marked;
    }
    if domains.reactive_reads {
        summary.reactive_reads = unknown_contract_claim();
        marked = true;
    }
    if domains.returns {
        summary.returns = unknown_contract_claim();
        marked = true;
    }
    if domains.callbacks {
        summary.callbacks = unknown_contract_claim();
        marked = true;
    }
    if domains.owner_requirements {
        summary.owner_requirements = unknown_contract_claim();
        marked = true;
    }
    if domains.async_behavior {
        summary.async_behavior = unknown_contract_claim();
        marked = true;
    }
    marked
}

fn unknown_contract_claim<T>() -> solid_reactive_ir::ContractClaim<T> {
    solid_reactive_ir::ContractClaim::Open
}

#[derive(Clone, Copy)]
struct UnresolvedExportIndex<'a> {
    facts: &'a solid_facts::ProjectFacts,
    aliases: &'a HashMap<String, String>,
    names_by_identity: &'a HashMap<String, Vec<String>>,
    names_by_symbol: &'a HashMap<String, Vec<String>>,
    /// Whether `names_by_identity`/`names_by_symbol` were built at all, i.e.
    /// whether the request named an entry file.
    ///
    /// Without one, `exports` is the whole project's export map keyed by the
    /// exported name, and no identity channel exists to join a declaration to
    /// it: the name *is* the key in that mode, and there is nothing more exact
    /// to prefer over it.
    entry_joined: bool,
    /// Whether every name in `exports` was joined to an identity or a symbol.
    ///
    /// Only then does "this function's identity is in neither map" prove the
    /// function is not an export. If one export never resolved to an entity,
    /// the maps cannot distinguish a private helper from *that* export, and
    /// the answer has to stay undecidable rather than certify a negative.
    exports_fully_joined: bool,
    /// The call graph's answer to "which functions can reach this obligation",
    /// computed where the graph lives (`solid-reactive-ir`).
    obligation_reach: &'a [solid_reactive_ir::ObligationReach],
}

/// How an open semantic leaf was attributed to the exports it affects.
///
/// The rungs are ordered by how directly they tie the obligation to a name,
/// and every rung above the last is exact: a lexical containment or a Type
/// Facts runtime identity, never a name-text match. The last one is the
/// admission that nothing tied it to anything.
#[derive(Clone, Copy, Eq, PartialEq)]
enum AttributionMechanism {
    /// The obligation's innermost enclosing function is itself an export.
    Joined,
    /// An outer function on the enclosing chain is an export -- the obligation
    /// sits in an anonymous callback, a named local helper, or a method inside
    /// it.
    EnclosingChain,
    /// The obligation's own location carries a Type Facts symbol whose other
    /// references sit inside exported functions.
    IdentityWidening,
    /// No enclosing function is an export, and the call graph proves exactly
    /// which exports can reach the one the obligation sits in.
    Reachability,
    /// A contract-generation obligation naming its exported function directly.
    ObligationIdentity,
    /// Nothing identified the obligation's function, so every export of the
    /// entrypoint is marked. This is the surviving fail-closed rung.
    FallbackAll,
}

impl AttributionMechanism {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Joined => "joined",
            Self::EnclosingChain => "enclosing-chain",
            Self::IdentityWidening => "identity-widening",
            Self::Reachability => "reachability",
            Self::ObligationIdentity => "obligation-identity",
            Self::FallbackAll => "fallback-all",
        }
    }
}

/// The export names one function declaration resolves to, by exact identity.
///
/// The Type Facts runtime identity first, then the canonical symbol. An alias
/// (`export { ProviderRoot as Root }`) and a cross-file re-export both resolve
/// here, and both of an aliased pair (`export { Panel, Panel as Root }`) come
/// back together. A same-named unrelated function does not resolve at all.
///
/// The two answers are different claims and callers depend on the difference:
///
/// - `None` — *undecidable*. Nothing names this declaration, or its name
///   carries no Type Facts entity, or the export set itself is not fully
///   joined to identities. Callers must widen.
/// - `Some(vec![])` — *decided*: the declaration was named, its identity was
///   resolved, and it is none of this entrypoint's exports. A private helper.
///
/// There is deliberately no name-text join. Matching a declaration to
/// `exports[local_name]` attributes an obligation inside a private `Render` to
/// an unrelated exported `Render`, and stops at the first name of an aliased
/// pair — both are wrong in the direction that publishes a claim about the
/// wrong export.
fn export_names_for_function(
    index: UnresolvedExportIndex<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
) -> Option<Vec<String>> {
    // Arrow bindings included: `export const X = () => {}` has neither
    // `name` nor `method_name`, and reading only those made every arrow export
    // unnameable at every rung of the ladder.
    let name = solid_reactive_ir::function_binding_name(file, function)?;
    if !index.entry_joined {
        // No entry file: `exports` is keyed by the project-wide exported name
        // and no identity channel exists, so the name is the only join there
        // is. Absence is still undecidable here, not a proven negative.
        let local_name = file.source_text(name.span)?;
        return exports
            .contains_key(local_name)
            .then(|| vec![local_name.to_owned()]);
    }
    let entity_location = typefacts::Location {
        path: file.path.to_string().into(),
        start_byte: u64::from(name.span.start),
        end_byte: u64::from(name.span.end),
    };
    let symbol = index.facts.typescript.entities().find(|entity| {
        entity.location.path == entity_location.path
            && entity.location.start_byte == entity_location.start_byte
            && entity.location.end_byte == entity_location.end_byte
    })?;
    let names = (!symbol.runtime_identity.is_empty())
        .then(|| {
            index
                .names_by_identity
                .get(symbol.runtime_identity.as_ref())
        })
        .flatten()
        .or_else(|| {
            index
                .names_by_symbol
                .get(&canonical_symbol(&symbol.symbol, index.aliases))
        })
        .cloned();
    match names {
        Some(names) => Some(names),
        // Nothing matched. That is a proven negative only when every export
        // *is* in the maps; otherwise the unjoined export could be this one.
        None => index.exports_fully_joined.then(Vec::new),
    }
}

fn file_at<'a>(index: UnresolvedExportIndex<'a>, path: &str) -> Option<&'a solid_facts::FileFacts> {
    index
        .facts
        .files
        .iter()
        .find(|file| file.path.as_str() == path)
}

fn span_of(location: &typefacts::Location) -> solid_facts::core::Span {
    solid_facts::core::Span::new(
        u32::try_from(location.start_byte).unwrap_or(u32::MAX),
        u32::try_from(location.end_byte).unwrap_or(u32::MAX),
    )
}

/// Canonical TypeScript symbol after applying the package generator's exact
/// declaration-to-runtime redirects.
fn runtime_canonical_symbol(index: UnresolvedExportIndex<'_>, symbol: &str) -> String {
    let mut current = canonical_symbol(symbol, index.aliases);
    let mut seen = HashSet::new();
    while seen.insert(current.clone()) {
        let Some(next) = index.facts.runtime_symbol_redirects.get(&current) else {
            break;
        };
        current = canonical_symbol(next, index.aliases);
    }
    current
}

/// Walks the enclosing-function chain outward from `location` and stops at the
/// first function that is an export.
///
/// Outward, not just the innermost: an obligation inside an anonymous arrow
/// passed to `createSignal`, or inside a named local helper, belongs to the
/// exported function that lexically contains it. Reading only the innermost
/// function is what sent those obligations to the mark-everything fallback.
///
/// Returns the depth as well, so the caller can report whether the innermost
/// function answered (`joined`) or an outer one did (`enclosing-chain`).
fn export_names_along_enclosing_chain(
    index: UnresolvedExportIndex<'_>,
    location: &typefacts::Location,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
) -> Option<(usize, Vec<String>)> {
    let file = file_at(index, location.path.as_ref())?;
    let mut chain = file
        .ast
        .functions_body_containing(span_of(location))
        .collect::<Vec<_>>();
    chain.sort_by_key(|function| function.body.end - function.body.start);
    chain.iter().enumerate().find_map(|(depth, function)| {
        export_names_for_function(index, file, function, exports)
            .filter(|names| !names.is_empty())
            .map(|names| (depth, names))
    })
}

/// The export names every function the call graph says can reach the
/// obligation resolves to.
///
/// `None` means the question was not answerable and the caller must fall back:
/// either the IR could not enumerate the reaching set soundly, or one of the
/// functions it named is no longer joinable to this entrypoint's facts. An
/// empty `Some` is a different answer -- the enumeration succeeded and *no*
/// export of this entrypoint can reach the obligation, so nothing is marked.
///
/// Which is why an unnameable reaching function propagates the `None` from
/// [`export_names_for_function`] rather than contributing no names: "I cannot
/// tell what this function is" read as "it is not an export" is exactly the
/// substitution that turns the documented fail-closed answer into a certified
/// negative.
///
/// The same substitution has a second entrance, which
/// [`module_surface_is_unaccounted`] closes: a reaching function that is
/// *decided: not an export of this entrypoint* but is published by its own
/// module, with no reference to it anywhere else in the project, is entered by
/// importers the call graph never saw.
fn export_names_from_reachability(
    index: UnresolvedExportIndex<'_>,
    reach: &solid_reactive_ir::ObligationReach,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
) -> Option<Vec<String>> {
    if !reach.complete {
        return None;
    }
    let mut names = Vec::new();
    for body in &reach.reaching {
        let file = file_at(index, body.path.as_ref())?;
        let span = span_of(body);
        let function = file
            .ast
            .functions
            .iter()
            .find(|function| function.body == span)?;
        let resolved = export_names_for_function(index, file, function, exports)?;
        if resolved.is_empty() && module_surface_is_unaccounted(index, file, function) {
            return None;
        }
        for name in resolved {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Some(names)
}

/// Whether a function published by its own module has no consumer anywhere in
/// the analyzed project -- so the call graph's caller enumeration for it cannot
/// be the whole entry set.
///
/// Asked only of a function the ladder decided is **not** an export of this
/// entrypoint. An entrypoint export is entered by consumers of the package, and
/// attribution answers for those by marking that export's own name; a
/// module-private function is entered only from inside the project, which is
/// exactly what the call graph enumerates. The gap is the third case: a
/// function its module publishes, that this entrypoint does not, and that
/// nothing in the project references. Either it exists for importers outside
/// the analyzed file set, or -- the shape this closes -- the importers are
/// inside it and bound to a *different* declaration of the same module.
///
/// That is what a sibling `channel.d.ts` beside `channel.js` does. TypeScript
/// resolves `./channel.js` to the declaration file, so the call in `index.js`
/// carries the declaration's runtime identity and the implementation's symbol
/// has no reference outside `channel.js` at all. The graph then enumerated the
/// helper alone, reported `complete`, and the obligation attributed to no
/// export: every export that really does reach it was published certified.
///
/// There is no fact that pairs a declaration file with the runtime module it
/// describes -- `ImportFact` carries only specifier text, and the compiler
/// treats the two files as unrelated modules -- so this cannot be resolved
/// exactly and reports itself incomplete instead. Emission then widens to
/// `fallback-all` and the marker records the widening.
///
/// Accounting is by exact identity, never by name text: a reference counts when
/// its Type Facts runtime identity or canonical symbol is the function's own.
fn module_surface_is_unaccounted(
    index: UnresolvedExportIndex<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    let Some(name) = solid_reactive_ir::function_binding_name(file, function) else {
        // Unnameable: `export_names_for_function` already answered `None` for
        // it, so this decision is never reached with one.
        return false;
    };
    let published = file.ast.exports.iter().any(|export| {
        export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .any(|specifier| specifier.local.span == name.span)
    });
    if !published {
        return false;
    }
    let Some(declaration) = index.facts.typescript.entities().find(|entity| {
        entity.location.path.as_ref() == file.path.as_str()
            && entity.location.start_byte == u64::from(name.span.start)
            && entity.location.end_byte == u64::from(name.span.end)
    }) else {
        return false;
    };
    let identity = declaration.runtime_identity.as_ref();
    let symbol = runtime_canonical_symbol(index, &declaration.symbol);
    let referenced_elsewhere = index.facts.typescript.entities().any(|entity| {
        entity.location.path.as_ref() != file.path.as_str()
            && ((!identity.is_empty() && entity.runtime_identity.as_ref() == identity)
                || (!symbol.is_empty()
                    && runtime_canonical_symbol(index, &entity.symbol) == symbol))
    });
    !referenced_elsewhere
}

/// The attribution ladder: which exports one unresolved obligation belongs to.
fn attribute_unresolved_obligation(
    index: UnresolvedExportIndex<'_>,
    location: &typefacts::Location,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
) -> (AttributionMechanism, Vec<String>) {
    if let Some((depth, names)) = export_names_along_enclosing_chain(index, location, exports) {
        let mechanism = if depth == 0 {
            AttributionMechanism::Joined
        } else {
            AttributionMechanism::EnclosingChain
        };
        return (mechanism, names);
    }
    // The obligation's own location may *be* a declaration rather than sit in
    // one -- an exported-helper obligation is filed at the function span, which
    // no body contains. Widen through the exact symbol at that location.
    if let Some(seed) = index.facts.typescript.entities().find(|entity| {
        entity.location.path == location.path
            && entity.location.start_byte == location.start_byte
            && entity.location.end_byte == location.end_byte
    }) {
        let seed_symbol = runtime_canonical_symbol(index, &seed.symbol);
        let seed_identity = seed.runtime_identity.as_ref();
        let mut widened = Vec::new();
        for reference in index.facts.typescript.entities().filter(|entity| {
            (!seed_identity.is_empty() && entity.runtime_identity.as_ref() == seed_identity)
                || (!seed_symbol.is_empty()
                    && runtime_canonical_symbol(index, &entity.symbol) == seed_symbol)
        }) {
            let Some((_, names)) =
                export_names_along_enclosing_chain(index, &reference.location, exports)
            else {
                continue;
            };
            for name in names {
                if !widened.contains(&name) {
                    widened.push(name);
                }
            }
        }
        if !widened.is_empty() {
            return (AttributionMechanism::IdentityWidening, widened);
        }
    }
    if let Some(reach) = index
        .obligation_reach
        .iter()
        .find(|reach| &reach.location == location)
        && let Some(names) = export_names_from_reachability(index, reach, exports)
    {
        return (AttributionMechanism::Reachability, names);
    }
    (
        AttributionMechanism::FallbackAll,
        exports.keys().cloned().collect(),
    )
}

/// Whether the published `parameter-member` reactive-read row already carries
/// this obligation's uncertainty, so the ladder has nothing to add.
///
/// An exported helper that invokes a member of one of its own parameters has
/// callers outside the analyzed project, so project analysis keeps the
/// obligation explicit (`EXPORTED_PARAMETER_MEMBER_DISPATCH`, raised in
/// solid-reactive-ir/src/interproc.rs). Contract emission may discharge it,
/// because `contract_export_function` serializes the same parameter provenance
/// as a `parameter-member` reactive read and a consumer resolves that row
/// against the argument it actually passes -- the pair
/// fixtures/package-contracts/parameter-member-read and
/// fixtures/reactive-ir/package-parameter-member-consumer pins exactly that.
///
/// The discharge holds only where the row is actually published, so the
/// question is asked of the exports the ladder would mark -- not of the helper
/// alone. The provenance does not survive a hop: a caller forwarding a member
/// of its own parameter (`helper(props.client)`) re-establishes no parameter of
/// its own, so an entrypoint export one or more frames above the helper
/// publishes no row at all and a consumer of *that* export is told nothing. The
/// blanket `analysis_context` filter this replaces discharged those exports
/// too, and their `reactiveReads` was emitted as a certified negative -- pinned
/// by fixtures/package-contracts/parameter-member-forwarded, whose `channelFor`
/// export is the covered control and whose `forwarded` export is the hop.
///
/// All-or-nothing, deliberately: when one attributed export is uncovered the
/// obligation is attributed to the whole set, including the helper whose own
/// row was fine. Marking a subset would leave the note claiming a narrower
/// attribution than the ladder computed, and the direction of the error here
/// is the safe one.
fn parameter_member_row_covers(
    index: UnresolvedExportIndex<'_>,
    defect: &solid_reactive_ir::StaticDefect,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
) -> bool {
    if defect.analysis_context != solid_reactive_ir::EXPORTED_PARAMETER_MEMBER_DISPATCH {
        return false;
    }
    let (_, names) = attribute_unresolved_obligation(index, &defect.location, exports);
    !names.is_empty()
        && names.iter().all(|name| {
            exports.get(name).is_some_and(|summary| {
                summary.reactive_reads.is_open()
                    || summary.reactive_reads.known().is_some_and(|reads| {
                        reads.iter().any(|read| read.kind == "parameter-member")
                    })
            })
        })
}

fn mark_unresolved_export_claims(
    index: UnresolvedExportIndex<'_>,
    defect: &solid_reactive_ir::StaticDefect,
    domains: UnresolvedClaimDomains,
    exports: &mut BTreeMap<String, solid_reactive_ir::ContractExport>,
) {
    let (mechanism, names) = attribute_unresolved_obligation(index, &defect.location, exports);
    let marked = names
        .into_iter()
        .filter(|name| {
            exports
                .get_mut(name)
                .is_some_and(|summary| mark_summary_claims_unknown(summary, domains))
        })
        .collect::<Vec<_>>();
    report_unknown_claim_attribution(
        defect.kind.variant_name(),
        &defect.analysis_context,
        &defect.location,
        mechanism,
        domains,
        &marked,
    );
}

/// Machine-readable context for a locally unresolved proposal domain.
///
/// The normalized proposal remains the authority; this stderr record explains
/// why attribution widened or found no export when a generation run refuses.
/// It is diagnostic provenance only and is never decoded as contract semantics.
const UNKNOWN_CLAIM_ATTRIBUTION_MARKER: &str = "solid-checker:unknown-claim-attribution=";

fn report_unknown_claim_attribution(
    obligation: &str,
    analysis_context: &str,
    location: &typefacts::Location,
    mechanism: AttributionMechanism,
    domains: UnresolvedClaimDomains,
    exports: &[String],
) {
    // An empty `exports` is still reported. Nothing was marked, so no
    // proposal claim will carry it -- but "the ladder resolved this obligation
    // to no export at all" is a narrowing decision, and the proposal plan is
    // where a narrowing decision has to be visible. Leaving it silent
    // made the interesting case (reachability proving no export reaches the
    // obligation) indistinguishable from the analyzer never having seen the
    // obligation, and the reviewer had nothing to check the narrowing against.
    // would make that refusal indistinguishable from an analysis that never
    // observed the obligation.
    let note = serde_json::json!({
        "obligation": obligation,
        "analysisContext": analysis_context,
        "path": location.path.as_ref(),
        "startByte": location.start_byte,
        "endByte": location.end_byte,
        "mechanism": mechanism.as_str(),
        "domains": domains.names(),
        "exports": exports,
    });
    eprintln!("{UNKNOWN_CLAIM_ATTRIBUTION_MARKER}{note}");
}

/// One import specifier as the analyzing program's own resolver answered it.
///
/// Every field is read off the producer's `ModuleImportFact` and none is
/// derived from another: `resolvedPath` is already a realpath, `symlinkPath` is
/// the path before that realpath was taken and is empty when the resolver saw
/// no divergence, and `includedPath` is the compiler's own declaration-to-input
/// redirect and is empty for an ordinary shipped `.d.ts`. A consumer that needs
/// the declaration-to-implementation edge and finds `includedPath` empty does
/// not have it, and must not reconstruct it by pairing file names.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryImport<'a> {
    /// The importing file. Together with the byte range this joins to a
    /// consumer's own syntax facts by exact span rather than by specifier text.
    path: &'a str,
    start_byte: u64,
    end_byte: u64,
    text: &'a str,
    resolution: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    resolved_path: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    included_path: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    symlink_path: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    extension: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    paths_pattern: &'a str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryModule<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    declaration_file: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeModuleResolutionDocument {
    schema_version: u64,
    resolutions: Vec<RuntimeModuleResolution>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeModuleResolution {
    importer: String,
    specifier: String,
    target: String,
}

/// Joins declaration-bound import symbols to the exact runtime implementation
/// selected for the same static ESM edge.
///
/// The generator supplies paths only from its resolver's successful `file`
/// answer. This side still proves both symbol ends: the import binding must be
/// a runtime-referenced named/default binding in the exact importer, and the
/// exact target module must export that same name through compiler entities.
/// A missing join adds nothing; two targets for one declaration root remove the
/// redirect entirely. There is no filename pairing or name-only fallback.
fn runtime_symbol_redirects(
    facts: &solid_facts::ProjectFacts,
    typescript: &mut TypeFactsSession,
    package_root: &Path,
    path: &Path,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let package_root = package_root.canonicalize()?;
    let document: RuntimeModuleResolutionDocument = serde_json::from_slice(&fs::read(path)?)?;
    if document.schema_version != 1 {
        return Err(format!(
            "unsupported runtime module resolution schemaVersion {}",
            document.schema_version
        )
        .into());
    }
    let import_paths = document
        .resolutions
        .iter()
        .map(|resolution| resolution.importer.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let graph = typescript
        .module_graph(&typefacts::ModuleGraphDemand::default().import_paths(import_paths))?;
    if !graph.is_complete() {
        return Ok(HashMap::new());
    }
    let aliases = canonical_symbol_aliases(facts);
    let entity_at = |file: &solid_facts::FileFacts, span: solid_facts::core::Span| {
        facts.typescript.entities().find(|entity| {
            entity.location.path.as_ref() == file.path.as_str()
                && entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
        })
    };
    let mut redirects = HashMap::<String, String>::new();
    let mut ambiguous = HashSet::new();
    for resolution in document.resolutions {
        let importer_path = Path::new(&resolution.importer).canonicalize()?;
        let target_path = Path::new(&resolution.target).canonicalize()?;
        if !importer_path.starts_with(&package_root) || !target_path.starts_with(&package_root) {
            continue;
        }
        let declaration_redirect = graph.imports.iter().any(|import| {
            import.text.as_ref() == resolution.specifier
                && same_canonical_path(Path::new(import.specifier.path.as_ref()), &importer_path)
                && import.included_path.is_empty()
                && (import.resolved_path.ends_with(".d.ts")
                    || import.resolved_path.ends_with(".d.mts")
                    || import.resolved_path.ends_with(".d.cts"))
        });
        if !declaration_redirect {
            continue;
        }
        let Some(importer) = facts
            .files
            .iter()
            .find(|file| same_canonical_path(Path::new(file.path.as_str()), &importer_path))
        else {
            continue;
        };
        let imports =
            importer.ast.imports.iter().filter(|import| {
                !import.type_only && import.module.as_str() == resolution.specifier
            });
        for import in imports {
            let bindings = import.bindings.iter().filter(|binding| {
                binding.runtime_referenced
                    && !binding.type_only
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
            });
            for binding in bindings {
                let exported = match binding.kind {
                    solid_facts::ast::ImportKind::Named => binding.imported.as_deref(),
                    solid_facts::ast::ImportKind::Default => Some("default"),
                    _ => None,
                };
                let Some(exported) = exported else {
                    continue;
                };
                let Some(source) = entity_at(importer, binding.local.span) else {
                    continue;
                };
                let Some(target) = entry_export_entity(facts, &target_path, exported) else {
                    continue;
                };
                let source = canonical_symbol(&source.symbol, &aliases);
                let target = canonical_symbol(&target.symbol, &aliases);
                if source.is_empty() || target.is_empty() || source == target {
                    continue;
                }
                if redirects
                    .get(&source)
                    .is_some_and(|existing| existing != &target)
                {
                    redirects.remove(&source);
                    ambiguous.insert(source);
                } else if !ambiguous.contains(&source) {
                    redirects.insert(source, target);
                }
            }
        }
    }
    Ok(redirects)
}

/// Writes the analyzing program's own module inventory beside the contract.
///
/// Two demands, in this order, because the second is scoped by the first: the
/// inventory names every file the program included, and import provenance is
/// then asked only of the files inside the package being described. A
/// package-local importer is the only one whose specifiers can disagree with
/// the generator's own walk of that package, so asking about every file in the
/// program would pay for the whole `node_modules` closure to answer a question
/// about one directory. With no `--contract-package-root` there is no such
/// directory and every included file is asked about.
///
/// `complete` is [`ModuleGraph::is_complete`] verbatim: a scoped answer that
/// covered less than it asked for. The consumer's contract is to fail closed on
/// a `false` rather than to reconcile it against its own walk, which is the
/// weaker source this file exists to replace.
fn write_module_inventory(
    typescript: &mut TypeFactsSession,
    request: &Request,
) -> Result<(), Box<dyn std::error::Error>> {
    let inventory = typescript.module_graph(&typefacts::ModuleGraphDemand::inventory())?;
    // Both spellings of the package directory, because the program's own is not
    // predictable from here. TypeScript takes a realpath only where resolution
    // walked a symlink under `node_modules`, so a project whose tsconfig names
    // files through a symlinked path -- `/var/folders/...` on macOS, and every
    // temporary directory an ecosystem probe generates in -- holds those files
    // under that spelling while `canonicalize` reports the other. Filtering by
    // one alone silently matched nothing, which turned the scoped import request
    // into an unscoped one: the same answer, computed for the whole program.
    // Both name the same directory, so accepting either is not a widening.
    let package_root = if request.contract_package_root.is_empty() {
        None
    } else {
        Some(PathBuf::from(&request.contract_package_root))
    };
    let canonical_root = package_root
        .as_deref()
        .and_then(|root| root.canonicalize().ok());
    let local = |path: &str| match &package_root {
        None => true,
        Some(root) => {
            Path::new(path).starts_with(root)
                || canonical_root
                    .as_deref()
                    .is_some_and(|canonical| Path::new(path).starts_with(canonical))
        }
    };
    let import_paths = inventory
        .modules
        .iter()
        .filter(|module| local(&module.path))
        .map(|module| module.path.to_string())
        .collect::<Vec<_>>();
    let graph = typescript
        .module_graph(&typefacts::ModuleGraphDemand::default().import_paths(import_paths))?;
    let document = serde_json::json!({
        "schemaVersion": 1,
        "projectId": request.project_id,
        // The spelling the caller used, not the canonical one: it is the
        // namespace the consumer's own paths are in, and the consumer normalizes
        // both sides itself rather than deriving one from the other.
        "packageRoot": package_root
            .as_deref()
            .map(|root| root.to_string_lossy().into_owned())
            .unwrap_or_default(),
        "complete": graph.is_complete(),
        // Already ordered by path, and by importing path then specifier start
        // byte, by the producer -- so the file is deterministic without a sort
        // here, and a sort here would hide it if that ever stopped being true.
        "modules": graph
            .modules
            .iter()
            .map(|module| InventoryModule {
                path: &module.path,
                declaration_file: module.declaration_file,
            })
            .collect::<Vec<_>>(),
        "imports": graph
            .imports
            .iter()
            .map(|import| InventoryImport {
                path: &import.specifier.path,
                start_byte: import.specifier.start_byte,
                end_byte: import.specifier.end_byte,
                text: &import.text,
                resolution: match import.resolution {
                    typefacts::ModuleResolution::Unresolved => "unresolved",
                    typefacts::ModuleResolution::Relative => "relative",
                    typefacts::ModuleResolution::NodeModules => "nodeModules",
                    typefacts::ModuleResolution::NonRelative => "nonRelative",
                },
                resolved_path: &import.resolved_path,
                included_path: &import.included_path,
                symlink_path: &import.symlink_path,
                extension: &import.extension,
                paths_pattern: &import.paths_pattern,
            })
            .collect::<Vec<_>>(),
        "unknownImportPaths": graph
            .unknown_import_paths
            .iter()
            .collect::<Vec<_>>(),
    });
    let output = Path::new(&request.emit_module_inventory);
    if let Some(directory) = output.parent()
        && !directory.as_os_str().is_empty()
    {
        fs::create_dir_all(directory)?;
    }
    fs::write(output, json_output::go_compatible(&document, true)?)?;
    Ok(())
}

fn emit_package_contract(
    request: &Request,
    program: &solid_reactive_ir::Program,
    facts: &solid_facts::ProjectFacts,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.package_name.is_empty() {
        return Err("--package-name is required with --emit-contract".into());
    }
    if request.package_version.is_empty() {
        return Err("--package-version is required with --emit-contract".into());
    }
    // SC9 findings are proof obligations, not permission to discard every
    // independently known export. After resolving the requested entrypoint we
    // attribute each one to the narrowest claim domain it can invalidate and
    // keep that exact semantic leaf open. Consumers then fail closed only
    // when they demand that claim. Proven violations remain diagnostics, but
    // they do not alter the package's descriptive runtime contract.
    let output = Path::new(&request.emit_contract);
    let entities_by_location = facts
        .typescript
        .entities()
        .map(|entity| (entity.location.clone(), entity))
        .collect::<HashMap<_, _>>();
    let mut exports = if request.contract_entry_file.is_empty() {
        (*program.contract_exports).clone()
    } else {
        contract_exports_for_entry_file(
            facts,
            program,
            Path::new(&request.contract_entry_file),
            &entities_by_location,
        )?
    };
    let entry_entities_by_name = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        exports
            .keys()
            .filter_map(|name| {
                entry_export_entity_indexed(
                    facts,
                    &entities_by_location,
                    &entry_file,
                    name,
                    &mut HashSet::new(),
                )
                .map(|entity| (name.clone(), entity))
            })
            .collect::<HashMap<_, _>>()
    };
    let exported_names_by_identity = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(identity) = entry_entities_by_name
                .get(name)
                .copied()
                .map(|entity| entity.runtime_identity.as_ref())
                .filter(|identity| !identity.is_empty())
            else {
                continue;
            };
            names
                .entry(identity.to_owned())
                .or_default()
                .push(name.clone());
        }
        names
    };
    let symbol_aliases = canonical_symbol_aliases(facts);
    let exported_names_by_symbol = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(symbol) = entry_entities_by_name
                .get(name)
                .copied()
                .map(|entity| canonical_symbol(&entity.symbol, &symbol_aliases))
                .filter(|symbol| !symbol.is_empty())
            else {
                continue;
            };
            names.entry(symbol).or_default().push(name.clone());
        }
        names
    };
    let joined_export_names = exported_names_by_identity
        .values()
        .chain(exported_names_by_symbol.values())
        .flatten()
        .collect::<HashSet<_>>();
    let unresolved_export_index = UnresolvedExportIndex {
        facts,
        aliases: &symbol_aliases,
        names_by_identity: &exported_names_by_identity,
        names_by_symbol: &exported_names_by_symbol,
        entry_joined: !request.contract_entry_file.is_empty(),
        exports_fully_joined: exports
            .keys()
            .all(|name| joined_export_names.contains(name)),
        obligation_reach: &program.obligation_reach,
    };
    for unresolved in &program.contract_generation_obligations {
        let target_names =
            if request.contract_entry_file.is_empty() || unresolved.function_identity.is_empty() {
                if exports.contains_key(&unresolved.function) {
                    vec![unresolved.function.clone()]
                } else {
                    Vec::new()
                }
            } else {
                exported_names_by_identity
                    .get(&unresolved.function_identity)
                    .cloned()
                    .unwrap_or_default()
            };
        let mut marked = Vec::new();
        for name in target_names {
            let Some(summary) = exports.get_mut(&name) else {
                continue;
            };
            // The obligation proves only that the callback list is
            // incomplete. Preserve every independently known claim and make
            // the uncertainty explicit instead of refusing the whole export.
            summary.callbacks = solid_reactive_ir::ContractClaim::Open;
            marked.push(name);
        }
        report_unknown_claim_attribution(
            "UnknownCallbackExecution",
            "contract-generation-obligation",
            &unresolved.location,
            AttributionMechanism::ObligationIdentity,
            UnresolvedClaimDomains {
                reactive_reads: false,
                returns: false,
                callbacks: true,
                owner_requirements: false,
                async_behavior: false,
            },
            &marked,
        );
    }
    // Decided against the export set as generation left it, before any of the
    // marking below moves it: whether another channel already carries an
    // obligation must not depend on which obligation happened to be attributed
    // first.
    let attributable = program
        .static_defects
        .iter()
        .filter(|defect| {
            defect.kind.is_unresolved_obligation()
                && !defect.kind.refused_through_generation_obligations()
                && !parameter_member_row_covers(unresolved_export_index, defect, &exports)
        })
        .collect::<Vec<_>>();
    for defect in attributable {
        mark_unresolved_export_claims(
            unresolved_export_index,
            defect,
            unresolved_claim_domains(&defect.kind),
            &mut exports,
        );
    }
    let contract = solid_reactive_ir::PackageContract {
        package: solid_reactive_ir::ContractPackage {
            name: request.package_name.clone(),
            version: request.package_version.clone(),
            integrity: String::new(),
        },
        entrypoints: [(
            ".".into(),
            solid_reactive_ir::ContractEntrypoint { exports },
        )]
        .into(),
        source_path: String::new(),
    };
    contract.validate().map_err(|error| error.to_string())?;
    let resolution: solid_facts_backend::ResolvedImport =
        serde_json::from_slice(&fs::read(&request.contract_resolution)?)?;
    let proposal = encode_inferred_contract_workflow(&contract, &resolution, true)?;
    fs::write(output, proposal.document)?;
    fs::write(&request.emit_proposal_plan, proposal.plan)?;
    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeclarationProbeRequest {
    source_path: String,
    target: String,
    output: String,
    exports: BTreeMap<String, usize>,
}

#[derive(Clone)]
struct DeclarationCall {
    export: String,
    location: typefacts::Location,
}

fn append_declaration_call(
    source: &mut String,
    path: &str,
    export: &str,
    arguments: &[String],
) -> DeclarationCall {
    let start = source.len();
    source.push_str("__solid_checker_package[");
    source.push_str(&serde_json::to_string(export).expect("a string always serializes"));
    source.push_str("](");
    source.push_str(&arguments.join(", "));
    source.push(')');
    let end = source.len();
    source.push_str(";\n");
    DeclarationCall {
        export: export.to_owned(),
        location: typefacts::Location {
            path: path.to_owned().into(),
            start_byte: u64::try_from(start).unwrap_or(u64::MAX),
            end_byte: u64::try_from(end).unwrap_or(u64::MAX),
        },
    }
}

fn declaration_source_prefix(probe: &DeclarationProbeRequest) -> Result<String, serde_json::Error> {
    let source = format!(
        "import * as __solid_checker_package from {};\n",
        serde_json::to_string(&probe.target)?
    );
    Ok(source)
}

fn prepare_declaration_probe(
    path: &Path,
) -> Result<DeclarationProbeRequest, Box<dyn std::error::Error>> {
    let request: DeclarationProbeRequest = serde_json::from_slice(&fs::read(path)?)?;
    if request.exports.len() > 1024 || request.exports.values().any(|arity| *arity > 16) {
        return Err("declaration probe plan exceeds its bounded export/arity limit".into());
    }
    let mut source = declaration_source_prefix(&request)?;
    let source_path = Path::new(&request.source_path);
    for (export, arity) in &request.exports {
        append_declaration_call(
            &mut source,
            &request.source_path,
            export,
            &vec!["null as never".to_owned(); *arity],
        );
    }
    fs::write(source_path, source)?;
    Ok(request)
}

fn object_recipe(shape: &typefacts::ObjectConstructionShape) -> Option<serde_json::Value> {
    let mut properties = serde_json::Map::new();
    for property in shape.required_properties.iter() {
        let witness = match property.witness {
            typefacts::ConstructionWitness::EmptyArray => "empty-array",
            typefacts::ConstructionWitness::EmptyObject => "empty-object",
            typefacts::ConstructionWitness::Unknown => return None,
        };
        properties.insert(
            property.name.to_string(),
            serde_json::Value::String(witness.to_owned()),
        );
    }
    Some(serde_json::json!({ "kind": "object", "properties": properties }))
}

fn recipe_javascript(recipe: &serde_json::Value) -> Option<String> {
    match recipe {
        serde_json::Value::String(value) if value == "empty-array" => Some("[]".into()),
        serde_json::Value::String(value) if value == "empty-object" => Some("{}".into()),
        serde_json::Value::Object(object) if object.get("kind")?.as_str()? == "object" => {
            let properties = object.get("properties")?.as_object()?;
            let mut rendered = Vec::with_capacity(properties.len());
            for (name, value) in properties {
                rendered.push(format!(
                    "{}: {}",
                    serde_json::to_string(name).ok()?,
                    recipe_javascript(value)?
                ));
            }
            Some(format!("{{{}}}", rendered.join(", ")))
        }
        serde_json::Value::Object(object) if object.get("kind")?.as_str()? == "factory" => {
            let export = object.get("export")?.as_str()?;
            let arguments = object.get("arguments")?.as_array()?;
            let arguments = arguments
                .iter()
                .map(recipe_javascript)
                .collect::<Option<Vec<_>>>()?;
            Some(format!(
                "__solid_checker_package[{}]({})",
                serde_json::to_string(export).ok()?,
                arguments.join(", ")
            ))
        }
        _ => None,
    }
}

fn resolved_calls_by_location(
    table: &solid_facts::TypeScriptTable,
) -> HashMap<typefacts::Location, &typefacts::ResolvedCall> {
    table
        .entities()
        .filter_map(|entity| {
            entity
                .resolved_call
                .as_deref()
                .map(|call| (entity.location.clone(), call))
        })
        .collect()
}

fn query_declaration_calls(
    typescript: &mut TypeFactsSession,
    calls: &[DeclarationCall],
    object_shapes: bool,
) -> Result<solid_facts::TypeScriptTable, BackendError> {
    typescript.semantic(
        calls
            .iter()
            .map(|call| typefacts::v3::EntityDemand {
                location: call.location.clone(),
                resolved_call: true,
                parameter_object_shape: object_shapes,
                ..typefacts::v3::EntityDemand::default()
            })
            .collect(),
    )
}

fn emit_declaration_probe_plan(
    typescript: &mut TypeFactsSession,
    probe: &DeclarationProbeRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    if probe.exports.is_empty() {
        let fragment = ProbePlanFragment {
            schema_version: 2,
            source: "typescript-value-domain",
            exports: BTreeMap::new(),
        };
        fs::write(
            &probe.output,
            format!("{}\n", serde_json::to_string_pretty(&fragment)?),
        )?;
        return Ok(());
    }
    let mut discovery_source = declaration_source_prefix(probe)?;
    let mut discovery_calls = Vec::new();
    for (export, arity) in &probe.exports {
        discovery_calls.push(append_declaration_call(
            &mut discovery_source,
            &probe.source_path,
            export,
            &vec!["null as never".to_owned(); *arity],
        ));
    }
    let discovery = query_declaration_calls(typescript, &discovery_calls, true)?;
    let discovered = resolved_calls_by_location(&discovery);
    let mut candidates = Vec::<(String, usize, serde_json::Value)>::new();
    for call in &discovery_calls {
        let Some(resolved) = discovered.get(&call.location) else {
            continue;
        };
        for argument in resolved.arguments.iter() {
            let Some(parameter) = argument.parameter.as_ref() else {
                continue;
            };
            let Some(shape) = parameter.object_shape.as_ref() else {
                continue;
            };
            let Some(recipe) = object_recipe(shape) else {
                continue;
            };
            let Ok(index) = usize::try_from(argument.argument_index) else {
                continue;
            };
            candidates.push((call.export.clone(), index, recipe));
        }
    }

    // Validate each completed candidate through the ordinary TypeScript call
    // validity path. Shape discovery alone is construction input, never proof
    // that generic inference accepts the completed expression.
    let mut validation_source = declaration_source_prefix(probe)?;
    let mut validation_calls = Vec::new();
    for (export, index, recipe) in &candidates {
        let Some(arity) = probe.exports.get(export) else {
            continue;
        };
        if index >= arity {
            continue;
        }
        let mut arguments = vec!["null as never".to_owned(); *arity];
        let Some(rendered) = recipe_javascript(recipe) else {
            continue;
        };
        arguments[*index] = rendered;
        validation_calls.push(append_declaration_call(
            &mut validation_source,
            &probe.source_path,
            export,
            &arguments,
        ));
    }
    typescript.update(vec![typefacts::v3::FileChange {
        path: probe.source_path.clone(),
        source: validation_source.into_bytes(),
        deleted: false,
        version: 1,
    }])?;
    let validation = query_declaration_calls(typescript, &validation_calls, false)?;
    let validated_calls = resolved_calls_by_location(&validation);
    let mut exports = BTreeMap::<String, BTreeMap<usize, Vec<serde_json::Value>>>::new();
    for ((export, index, recipe), call) in candidates.iter().zip(&validation_calls) {
        if validated_calls
            .get(&call.location)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
        {
            exports
                .entry(export.clone())
                .or_default()
                .entry(*index)
                .or_default()
                .push(recipe.clone());
        }
    }

    for parameters in exports.values_mut() {
        for recipes in parameters.values_mut() {
            let mut seen = HashSet::new();
            recipes.retain(|recipe| seen.insert(recipe.to_string()));
        }
    }
    let fragment = ProbePlanFragment {
        schema_version: 2,
        source: "typescript-value-domain",
        exports,
    };
    fs::write(
        &probe.output,
        format!("{}\n", serde_json::to_string_pretty(&fragment)?),
    )?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbePlanFragment {
    schema_version: u32,
    source: &'static str,
    exports: BTreeMap<String, BTreeMap<usize, Vec<serde_json::Value>>>,
}

fn contract_exports_for_entry_file(
    facts: &solid_facts::ProjectFacts,
    program: &solid_reactive_ir::Program,
    entry_file: &Path,
    entities_by_location: &HashMap<typefacts::Location, &typefacts::EntityFact>,
) -> Result<BTreeMap<String, solid_reactive_ir::ContractExport>, Box<dyn std::error::Error>> {
    let entry_file = entry_file.canonicalize()?;
    let mut visiting = HashSet::new();
    let names = exported_names_for_file(facts, &entry_file, &mut visiting)?;
    let entry_entities_by_name = names
        .iter()
        .filter_map(|name| {
            entry_export_entity_indexed(
                facts,
                entities_by_location,
                &entry_file,
                name,
                &mut HashSet::new(),
            )
            .map(|entity| (name.clone(), entity))
        })
        .collect::<HashMap<_, _>>();
    let symbol_aliases = canonical_symbol_aliases(facts);
    let generated_owner_requirements =
        generated_owner_requirements_by_symbol(facts, program, &symbol_aliases);
    let mut exports = BTreeMap::new();
    for name in names {
        let summary = program.contract_exports.get(&name).cloned().ok_or_else(|| {
            format!(
                "emit package contract: entry file {} exports {name:?}, but no semantic summary was produced",
                entry_file.display()
            )
        })?;
        let summary = promote_entry_callable(
            facts,
            &entry_file,
            &name,
            entry_entities_by_name.get(&name).copied(),
            summary,
        )?;
        let summary = attach_generated_owner_requirements(
            facts,
            &symbol_aliases,
            &generated_owner_requirements,
            &entry_file,
            &name,
            entry_entities_by_name.get(&name).copied(),
            summary,
        );
        exports.insert(name, summary);
    }
    unify_runtime_alias_summaries(&entry_entities_by_name, &mut exports);
    if exports.is_empty() {
        return Err(format!(
            "emit package contract: entry file {} has no runtime ESM exports",
            entry_file.display()
        )
        .into());
    }
    Ok(exports)
}

type FunctionKey = (String, u32, u32);

#[derive(Default)]
struct GeneratedOwnerRequirements {
    by_symbol: HashMap<String, Vec<solid_reactive_ir::OwnerRequirementOperation>>,
    by_function: HashMap<FunctionKey, Vec<solid_reactive_ir::OwnerRequirementOperation>>,
}

fn canonical_symbol_aliases(facts: &solid_facts::ProjectFacts) -> HashMap<String, String> {
    let mut aliases = facts
        .typescript
        .symbols()
        .filter(|symbol| !symbol.alias_target().is_empty())
        .map(|symbol| (symbol.id().to_owned(), symbol.alias_target().to_owned()))
        .collect::<HashMap<_, _>>();
    for _ in 0..aliases.len() {
        let previous = aliases.clone();
        let mut changed = false;
        for target in aliases.values_mut() {
            if let Some(next) = previous.get(target)
                && next != target
            {
                *target = next.clone();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    aliases
}

fn canonical_symbol(symbol: &str, aliases: &HashMap<String, String>) -> String {
    aliases
        .get(symbol)
        .map_or_else(|| symbol.to_owned(), Clone::clone)
}

/// Indexes exact owner requirements by canonical compiler symbol and by exact
/// function identity. Symbols follow aliases and re-exports; the function key
/// is the fail-closed fallback for an anonymous default export, which has no
/// name symbol. An operation belongs only to its immediate containing function
/// so a nested closure cannot make its outer factory require an owner merely
/// because their spans nest.
fn generated_owner_requirements_by_symbol(
    facts: &solid_facts::ProjectFacts,
    program: &solid_reactive_ir::Program,
    aliases: &HashMap<String, String>,
) -> GeneratedOwnerRequirements {
    let entities = facts
        .typescript
        .entities()
        .map(|entity| {
            (
                (
                    entity.location.path.to_string(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ),
                entity,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut function_symbols = HashMap::<FunctionKey, Option<String>>::new();

    for file in &facts.files {
        for function in &file.ast.functions {
            let Some(name) = function.name.as_ref() else {
                continue;
            };
            let key = (
                file.path.to_string(),
                u64::from(name.span.start),
                u64::from(name.span.end),
            );
            let Some(entity) = entities.get(&key) else {
                continue;
            };
            if entity.symbol.is_empty() {
                continue;
            }
            function_symbols.insert(
                (
                    file.path.to_string(),
                    function.span.start,
                    function.span.end,
                ),
                Some(canonical_symbol(&entity.symbol, aliases)),
            );
        }
    }

    let mut indexed = GeneratedOwnerRequirements::default();
    for requirement in program.missing_owners.iter().filter(|requirement| {
        !requirement.runtime_uncertain
            && !requirement.conditional_owner
            && !requirement.component_uncertain
    }) {
        let Some(file) = facts
            .files
            .iter()
            .find(|file| file.path.as_str() == requirement.location.path.as_ref())
        else {
            continue;
        };
        let span = solid_facts::core::Span::new(
            u32::try_from(requirement.location.start_byte).unwrap_or(u32::MAX),
            u32::try_from(requirement.location.end_byte).unwrap_or(u32::MAX),
        );
        let Some(function) = file
            .ast
            .functions_body_containing(span)
            .min_by_key(|function| function.body.end - function.body.start)
        else {
            continue;
        };
        let key = (
            file.path.to_string(),
            function.span.start,
            function.span.end,
        );
        let operations = indexed.by_function.entry(key.clone()).or_default();
        if !operations.contains(&requirement.operation) {
            operations.push(requirement.operation);
        }
        if let Some(Some(symbol)) = function_symbols.get(&key) {
            let operations = indexed.by_symbol.entry(symbol.clone()).or_default();
            if !operations.contains(&requirement.operation) {
                operations.push(requirement.operation);
            }
        }
    }
    indexed
}

/// Adds exact owner requirements observed inside the exact exported function
/// to the generated package summary. The source project may report the
/// function as open-world/uncertain because its callers are not enumerable;
/// that is precisely the obligation a consumer contract must carry.
fn attach_generated_owner_requirements(
    facts: &solid_facts::ProjectFacts,
    aliases: &HashMap<String, String>,
    generated: &GeneratedOwnerRequirements,
    entry_file: &Path,
    export_name: &str,
    export_entity: Option<&typefacts::EntityFact>,
    mut summary: solid_reactive_ir::ContractExport,
) -> solid_reactive_ir::ContractExport {
    let operations = export_entity
        .map(|entity| canonical_symbol(&entity.symbol, aliases))
        .filter(|symbol| !symbol.is_empty())
        .and_then(|symbol| generated.by_symbol.get(&symbol))
        .or_else(|| {
            (export_name == "default").then(|| {
                let file = facts
                    .files
                    .iter()
                    .find(|file| same_canonical_path(Path::new(file.path.as_str()), entry_file))?;
                let default_span = file
                    .ast
                    .exports
                    .iter()
                    .filter(|export| {
                        !export.type_only && export.kind == solid_facts::ast::ExportKind::Default
                    })
                    .flat_map(|export| export.declarations.iter())
                    .find(|specifier| !specifier.type_only && specifier.exported == "default")?
                    .local
                    .span;
                let function = file
                    .ast
                    .functions
                    .iter()
                    .filter(|function| {
                        function.span.contains(default_span)
                            && !function.body.contains(default_span)
                    })
                    .min_by_key(|function| function.span.end - function.span.start)?;
                generated.by_function.get(&(
                    file.path.to_string(),
                    function.span.start,
                    function.span.end,
                ))
            })?
        });
    let Some(operations) = operations else {
        return summary;
    };
    let Some(owner_requirements) = summary.owner_requirements.known_mut() else {
        // An inherited/re-exported unknown remains unknown. Adding the local
        // positive rows would not prove that the list is complete.
        return summary;
    };
    for operation in operations {
        if !owner_requirements
            .iter()
            .any(|existing| existing.operation == *operation)
        {
            owner_requirements.push(solid_reactive_ir::ContractOwnerRequirement {
                operation: *operation,
            });
        }
    }
    owner_requirements.sort_by_key(|requirement| match requirement.operation {
        solid_reactive_ir::OwnerRequirementOperation::Effect => 0,
        solid_reactive_ir::OwnerRequirementOperation::Cleanup => 1,
        solid_reactive_ir::OwnerRequirementOperation::Boundary => 2,
        solid_reactive_ir::OwnerRequirementOperation::SettledCleanup => 3,
    });
    summary
}

/// Decides the `kind` of one entry-file export, or refuses the entrypoint.
///
/// A bare `kind: "value"` summary is the maximal certified negative claim —
/// `validate_export` bars it from carrying even an open function domain — so it is
/// publishable only against a proof that the export is not a function.
/// [`solid_reactive_ir::export_kind_proof`] holds the whole rule; only its two
/// closed answers publish anything here. `Unknown` (an `any`, `unknown`,
/// `never` or error type, which is what an untyped dependency leaves behind in
/// a published `.js` artifact), `Mixed`, and an absent fact are the absence of
/// that proof — on *either* of the two signature facts — and treating any of
/// them as `value` is how `@solid-devtools/locator@0.16.7` came to publish
/// "invokes no caller-supplied callback" for `addClickInterceptor(fn)`.
/// Refusing costs the entrypoint; publishing costs the claim, which is worse.
/// See docs/package-contracts.md "Refused entrypoints versus failed
/// generation".
///
/// No dependency proposal is consulted here. A proposal is not accepted proof
/// for another proposal, so an external boundary remains a local open claim or
/// an entrypoint refusal until an independently proof-checked document and
/// receipt are supplied to the analyzer.
fn promote_entry_callable(
    facts: &solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
    export_entity: Option<&typefacts::EntityFact>,
    summary: solid_reactive_ir::ContractExport,
) -> Result<solid_reactive_ir::ContractExport, Box<dyn std::error::Error>> {
    let Some(entity) = export_entity else {
        return Ok(summary);
    };
    let refuse = |reason: String| -> Result<_, Box<dyn std::error::Error>> {
        Err(format!(
            "emit package contract: entry file {} exports {name:?}, {reason}; publishing kind \"value\" would certify it invokes no caller-supplied callback",
            entry_file.display()
        )
        .into())
    };
    match solid_reactive_ir::export_kind_proof_from_entity(facts, &entity.location, Some(entity)) {
        // A call signature or a construct signature; either is
        // `typeof === "function"` at runtime, and the type system reads a
        // construct signature as *not* a call signature, so a class arrives
        // here through constructability alone. The raise leaves `callbacks`
        // unknown for both: a summary still saying `value` here is one whose
        // body was never analyzed, so its silence about callbacks is not a
        // claim either. See `solid_reactive_ir::raised_function_export`.
        solid_reactive_ir::ExportKindProof::Callable if summary.kind == "value" => {
            Ok(solid_reactive_ir::raised_function_export(summary))
        }
        solid_reactive_ir::ExportKindProof::Callable => Ok(summary),
        // The exact exported specifier has closed negative callability and
        // constructability facts. That proof outranks an inferred function
        // summary reached through an initializer expression: a call such as a
        // bundler's `__exportAll({...})` has the callee's symbol inside it, but
        // exports the call result. Keeping the callee summary here certified a
        // namespace object as callable (`@kobalte/core`'s `./menubar:t`).
        solid_reactive_ir::ExportKindProof::NonCallable if summary.kind == "function" => {
            Ok(solid_reactive_ir::ContractExport {
                kind: "value".into(),
                ..solid_reactive_ir::ContractExport::default()
            })
        }
        solid_reactive_ir::ExportKindProof::Unresolvable(callability, constructability) => {
            refuse(format!(
                "whose runtime kind no closed type answers ({callability:?}, {constructability:?})"
            ))
        }
        // Demanded and unanswered, not undemanded: `demand_plan` requests both
        // signature facts at every export specifier and every exported
        // declaration name, which are the only spans reaching this decision.
        // Absence is the producer finding no node to classify, so it is
        // missing evidence about this export rather than an answer about its
        // type — and `kind: "value"` is a claim, not a default.
        solid_reactive_ir::ExportKindProof::Unanswered => {
            refuse("whose runtime kind no fact covers at all".into())
        }
        solid_reactive_ir::ExportKindProof::NonCallable => Ok(summary),
    }
}

fn entry_export_entity<'a>(
    facts: &'a solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
) -> Option<&'a typefacts::EntityFact> {
    entry_export_entity_with_visiting(facts, entry_file, name, &mut HashSet::new())
}

fn entry_export_entity_indexed<'a>(
    facts: &'a solid_facts::ProjectFacts,
    entities_by_location: &HashMap<typefacts::Location, &'a typefacts::EntityFact>,
    entry_file: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> Option<&'a typefacts::EntityFact> {
    let entry_file = entry_file.canonicalize().ok()?;
    if !visiting.insert((entry_file.clone(), name.to_owned())) {
        return None;
    }
    let file = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &entry_file))?;
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        if let Some(specifier) = export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .find(|specifier| !specifier.type_only && specifier.exported == name)
        {
            let span = specifier.local.span;
            let location = typefacts::Location {
                path: file.path.to_string().into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            };
            if let Some(entity) = entities_by_location.get(&location) {
                return Some(*entity);
            }
            if let Some(module) = export.module.as_deref()
                && module.starts_with('.')
            {
                let target = resolve_relative_export(facts, &entry_file, module).ok()?;
                let local_name = file.source_text(specifier.local.span).unwrap_or(name);
                if let Some(entity) = entry_export_entity_indexed(
                    facts,
                    entities_by_location,
                    &target,
                    local_name,
                    visiting,
                ) {
                    return Some(entity);
                }
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && module.starts_with('.')
        {
            let target = resolve_relative_export(facts, &entry_file, module).ok()?;
            if let Some(entity) =
                entry_export_entity_indexed(facts, entities_by_location, &target, name, visiting)
            {
                return Some(entity);
            }
        }
    }
    None
}

fn entry_export_entity_with_visiting<'a>(
    facts: &'a solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> Option<&'a typefacts::EntityFact> {
    let entry_file = entry_file.canonicalize().ok()?;
    if !visiting.insert((entry_file.clone(), name.to_owned())) {
        return None;
    }
    let file = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &entry_file))?;
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace`, `declare module`, or `declare global` body binds a member
    // of that namespace object, not a name this module publishes. See
    // `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        if let Some(specifier) = export
            .specifiers
            .iter()
            .chain(&export.declarations)
            .find(|specifier| !specifier.type_only && specifier.exported == name)
        {
            let span = specifier.local.span;
            let location = typefacts::Location {
                path: file.path.to_string().into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            };
            if let Some(entity) = facts.typescript.entities().find(|entity| {
                entity.location.path == location.path
                    && entity.location.start_byte == location.start_byte
                    && entity.location.end_byte == location.end_byte
            }) {
                return Some(entity);
            }
            if let Some(module) = export.module.as_deref()
                && module.starts_with('.')
            {
                let target = resolve_relative_export(facts, &entry_file, module).ok()?;
                let local_name = file.source_text(specifier.local.span).unwrap_or(name);
                if let Some(entity) =
                    entry_export_entity_with_visiting(facts, &target, local_name, visiting)
                {
                    return Some(entity);
                }
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && module.starts_with('.')
        {
            let target = resolve_relative_export(facts, &entry_file, module).ok()?;
            if let Some(entity) = entry_export_entity_with_visiting(facts, &target, name, visiting)
            {
                return Some(entity);
            }
        }
    }
    None
}

fn unify_runtime_alias_summaries(
    entry_entities_by_name: &HashMap<String, &typefacts::EntityFact>,
    exports: &mut BTreeMap<String, solid_reactive_ir::ContractExport>,
) {
    let mut names_by_identity = BTreeMap::<String, Vec<String>>::new();
    for name in exports.keys() {
        let Some(identity) = entry_entities_by_name
            .get(name)
            .copied()
            .map(|entity| entity.runtime_identity.as_ref())
            .filter(|identity| !identity.is_empty())
        else {
            continue;
        };
        names_by_identity
            .entry(identity.to_owned())
            .or_default()
            .push(name.clone());
    }
    for names in names_by_identity.values().filter(|names| names.len() > 1) {
        let mut merged = solid_reactive_ir::ContractExport::default();
        for name in names {
            let Some(summary) = exports.get(name) else {
                continue;
            };
            if summary.kind == "function" {
                merged.kind = "function".into();
            } else if merged.kind.is_empty() {
                merged.kind = summary.kind.clone();
            }
            match (&mut merged.reactive_reads, &summary.reactive_reads) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_reads),
                    solid_reactive_ir::ContractClaim::Known(reads),
                ) => {
                    for read in reads {
                        if !merged_reads.contains(read) {
                            merged_reads.push(read.clone());
                        }
                    }
                }
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.reactive_reads = solid_reactive_ir::ContractClaim::Open;
                }
                (solid_reactive_ir::ContractClaim::Open, _) => {}
            }
            match (&mut merged.callbacks, &summary.callbacks) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_callbacks),
                    solid_reactive_ir::ContractClaim::Known(callbacks),
                ) => {
                    for callback in callbacks {
                        if !merged_callbacks.contains(callback) {
                            merged_callbacks.push(callback.clone());
                        }
                    }
                }
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.callbacks = solid_reactive_ir::ContractClaim::Open;
                }
                (solid_reactive_ir::ContractClaim::Open, _) => {}
            }
            match (&mut merged.returns, &summary.returns) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_return),
                    solid_reactive_ir::ContractClaim::Known(returned),
                ) if merged_return.is_none() => *merged_return = returned.clone(),
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.returns = solid_reactive_ir::ContractClaim::Open;
                }
                _ => {}
            }
            match (&mut merged.async_behavior, &summary.async_behavior) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_behavior),
                    solid_reactive_ir::ContractClaim::Known(behavior),
                ) if merged_behavior.is_empty() => *merged_behavior = behavior.clone(),
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.async_behavior = solid_reactive_ir::ContractClaim::Open;
                }
                _ => {}
            }
        }
        if let Some(callbacks) = merged.callbacks.known_mut() {
            callbacks.sort_by_key(|callback| (callback.parameter, callback.execution.clone()));
        }
        if let Some(reads) = merged.reactive_reads.known_mut() {
            reads.sort_by(|left, right| {
                (&left.kind, &left.parameter, &left.label).cmp(&(
                    &right.kind,
                    &right.parameter,
                    &right.label,
                ))
            });
        }
        for name in names {
            exports.insert(name.clone(), merged.clone());
        }
    }
}

/// The machine-readable half of a missing-dependency refusal.
///
/// Contract generation is fail-closed across package boundaries. When this
/// entrypoint re-exports a package with no exact accepted contract, this stable
/// stderr record identifies the unresolved module for the refusal report.
///
/// Parsing a specifier back out of human prose would couple automation to
/// wording, so the boundary emits one stable line in addition to the message.
const UNRESOLVED_DEPENDENCY_MODULE_MARKER: &str = "solid-checker:unresolved-dependency-module=";

/// Refuse emission at a package boundary this process cannot cross, naming the
/// dependency both machine-readably and in prose. A module specifier never
/// contains a newline, so the marker is exactly one line.
///
/// The marker is written here, on the propagation path, rather than while the
/// error value is built: a marker on stderr is a claim that this run *refused*,
/// so it must be a side effect of actually refusing. Constructing the error is
/// not refusing — a caller that recovered it would otherwise leave the marker
/// behind on a run that exits 0, and the generator would recurse on a
/// dependency this process never declined.
fn refuse_unresolved_dependency_module<T>(
    module: &str,
    from: &Path,
) -> Result<T, Box<dyn std::error::Error>> {
    eprintln!("{UNRESOLVED_DEPENDENCY_MODULE_MARKER}{module}");
    Err(format!(
        "emit package contract: cannot statically expand external export-all {module:?} from {}; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts",
        from.display()
    )
    .into())
}

fn exported_names_for_file(
    facts: &solid_facts::ProjectFacts,
    path: &Path,
    visiting: &mut HashSet<PathBuf>,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    if !visiting.insert(path.clone()) {
        return Ok(BTreeSet::new());
    }
    let file = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &path))
        .ok_or_else(|| {
            format!(
                "emit package contract: entry file {} is not part of the TypeScript project",
                path.display()
            )
        })?;
    let mut names = BTreeSet::new();
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace`, `declare module`, or `declare global` body binds a member of
    // that namespace object rather than a name this module publishes, so it is
    // not part of the entrypoint's surface. See
    // `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        if export.kind == solid_facts::ast::ExportKind::All {
            let module = export
                .module
                .as_deref()
                .ok_or("export-all declaration has no module")?;
            if !module.starts_with('.') {
                return refuse_unresolved_dependency_module(module, &path);
            }
            let target = resolve_relative_export(facts, &path, module)?;
            names.extend(exported_names_for_file(facts, &target, visiting)?);
            continue;
        }
        if export.kind == solid_facts::ast::ExportKind::Default {
            names.insert("default".into());
        }
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only)
        {
            let name = specifier.exported.to_string();
            // An unmarked re-export of a type is not a runtime export at all.
            // Omitting it is what `export type { T }` already does one line
            // above, through `type_only`; this proves the same thing for the
            // spelling that carries no marker. See `export_is_type_only`.
            if export_is_type_only(facts, &path, &name, &mut HashSet::new()) {
                continue;
            }
            names.insert(name);
        }
        for binding in file.ast.exported_bindings(export) {
            names.extend(binding.names.iter().filter_map(|name| {
                file.source_text(name.span)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
            }));
        }
    }
    visiting.remove(&path);
    Ok(names)
}

/// Whether every export of `name` from `path` exists only in type space.
///
/// `export type { T }` and `export interface T {}` say so in their own syntax
/// and are filtered by `type_only` before this is consulted. An **unmarked**
/// re-export of a type — `import { Options } from "./types.js"; export
/// { Options }`, or `export { Options } from "./types.js"`, both legal with no
/// `type` modifier — says nothing at the export site, and no fact the producer
/// offers at that span separates it from a value whose type is unresolvable:
/// callability is `Unknown`, `runtime_identity` is empty, `reference_space` is
/// structurally `Neither` (identifiers inside an import or export specifier are
/// excluded from the reference index) and the declaration kind is the catch-all
/// `"declaration"` for an interface and for a name whose module lies outside
/// the project alike. Left to the `kind` decision the whole entrypoint refuses,
/// costing every real export beside it, for a name that has no runtime
/// existence to describe.
///
/// This project's own syntax at the *declaring* file does separate them.
/// Walking the relative re-export and import chain, a name whose every export
/// is marked `type_only` somewhere along it binds nothing at runtime, so
/// omitting it is exactly right — and exactly what the marked spelling already
/// gets.
///
/// Fail-closed by construction: a chain that leaves this project (a bare
/// specifier, an unresolvable relative path) or that this walk cannot see (a
/// name declared locally without being exported as a declaration, so no
/// `type_only` specifier covers it) proves nothing and returns `false`, which
/// leaves the name to the `kind` decision and its refusal. Declaration merging
/// is handled by requiring *every* export of the name to be type-only: an
/// `export interface T {}` beside an `export const T` is a runtime export.
fn export_is_type_only(
    facts: &solid_facts::ProjectFacts,
    path: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    if !visiting.insert((path.clone(), name.to_owned())) {
        return false;
    }
    let Some(file) = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &path))
    else {
        return false;
    };
    let mut proven = false;
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace`, `declare module`, or `declare global` body binds a member
    // of that namespace object, not a name this module publishes. See
    // `AstFacts::module_level_exports`.
    for export in file.ast.module_level_exports() {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| specifier.exported.as_str() == name)
        {
            if export.type_only || specifier.type_only {
                proven = true;
                continue;
            }
            let local_name = file
                .source_text(specifier.local.span)
                .unwrap_or(specifier.exported.as_str());
            let type_only = match export.module.as_deref() {
                Some(module) if module.starts_with('.') => {
                    resolve_relative_export(facts, &path, module).is_ok_and(|target| {
                        export_is_type_only(facts, &target, local_name, visiting)
                    })
                }
                // A bare specifier leaves this project; the dependency's own
                // contract describes its runtime exports and says nothing
                // about its type-only ones, so nothing here is proof.
                Some(_) => false,
                None => local_import_is_type_only(facts, file, &path, local_name, visiting),
            };
            if !type_only {
                return false;
            }
            proven = true;
        }
    }
    proven
}

/// Whether the local name a bare `export { x }` specifier names is an import
/// binding this project can follow to a type-only declaration.
///
/// Only the import chain: a name declared in this file and exported by
/// specifier rather than as a declaration carries no `type_only` fact anywhere,
/// so `interface T {} export { T }` is not provable here and stays with the
/// refusal. Adding it needs a type-declaration fact in solid-facts.
fn local_import_is_type_only(
    facts: &solid_facts::ProjectFacts,
    file: &solid_facts::FileFacts,
    path: &Path,
    local_name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> bool {
    for import in &file.ast.imports {
        for binding in &import.bindings {
            if file.source_text(binding.local.span) != Some(local_name) {
                continue;
            }
            if import.type_only || binding.type_only {
                return true;
            }
            let Some(imported) = binding.imported.as_deref().or_else(|| {
                (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
            }) else {
                return false;
            };
            if !import.module.starts_with('.') {
                return false;
            }
            return resolve_relative_export(facts, path, &import.module)
                .is_ok_and(|target| export_is_type_only(facts, &target, imported, visiting));
        }
    }
    false
}

fn resolve_relative_export(
    facts: &solid_facts::ProjectFacts,
    source: &Path,
    module: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let project_paths = facts.files.iter().map(|file| file.path.as_str());
    let Some(target) =
        solid_facts::resolve_relative_module_path(&source.to_string_lossy(), module, project_paths)
    else {
        return Err(format!(
            "emit package contract: cannot resolve export-all {module:?} from {}",
            source.display()
        )
        .into());
    };
    Ok(PathBuf::from(target))
}

fn same_canonical_path(left: &Path, right: &Path) -> bool {
    left == right || left.canonicalize().is_ok_and(|left| left == right)
}

/// A per-project daemon holding the retained `NativeIncrementalSession` behind
/// a Unix socket, so repeat CLI checks reuse the warm session instead of
/// rebuilding the TypeScript program and demand closure from scratch.
///
/// Release clients use it by default and may opt out with
/// `SOLID_CHECKER_DAEMON=0`. The socket path is derived from the canonical
/// project id. Before every answer the daemon resynchronizes with the
/// filesystem: a changed tsconfig, a changed source directory (file created,
/// deleted, or renamed), or an unreadable known file rebuilds the whole
/// session; changed file contents become incremental overlay updates. The
/// response body is byte-identical to one-shot output.
#[cfg(unix)]
mod daemon;

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let program = std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.file_stem()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "solid-facts-backend".into());
            eprintln!("{program}: {error}");
            // An incompatible producer is its own exit code so callers can
            // tell "wrong build" from "analysis failed". The session reports
            // it through its own error type now.
            let exit_code = if error.downcast_ref::<BackendError>().is_some_and(|error| {
                matches!(
                    error,
                    BackendError::Handshake(_)
                        | BackendError::TypeFactsSession(typefacts::SessionError::Handshake(_))
                )
            }) {
                3
            } else {
                2
            };
            std::process::exit(exit_code);
        }
    }
}
