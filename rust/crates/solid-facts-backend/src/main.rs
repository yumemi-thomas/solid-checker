mod daemon_cache;
mod idle_memory;
mod json_output;
mod snapshot_emission;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use solid_facts_backend::{
    BackendError, RequestedRuleEnablement, SemanticDemandOptions, SourceFile, TypeFactsSession,
    analyze_project_measured_with_enablement, build_project_native_measured_with_demands,
    default_typefacts_executable, dialect, encode_package_contract, package_contract_statuses,
    read_package_contract, semantic_demand_options_for_enablement,
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
    #[serde(default)]
    contract_paths: Vec<String>,
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
    #[serde(default)]
    package_name: String,
    #[serde(default)]
    package_version: String,
    #[serde(default)]
    declaration_artifact: String,
    #[serde(default)]
    implementation_artifact: String,
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
            read_package_contract(Path::new(path))?;
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
    let sidecar_spawn_ns = started.elapsed().as_nanos();
    let mut sources_bytes = 0usize;
    let sources_wire_bytes = 0u64;
    let mut preloaded_bundled = None;
    if request.sources.is_empty() {
        // Decode the bundled solid-js contract first: it is the only cold work
        // that needs nothing from the producer, so it overlaps the program
        // build that the source fetch is about to wait on.
        if diagnostics {
            preloaded_bundled = (dialect.bundled_contract)("solid-js")?;
        }
        request.sources = typescript.configured_sources()?;
        sources_bytes = request.sources.iter().map(|s| s.source.len()).sum();
    }
    let source_setup_ns = started.elapsed().as_nanos();
    let requested_enablement = RequestedRuleEnablement {
        presets: &request.presets,
        rules: &request.enable_rules,
        runtime: request.runtime.clone(),
    };
    let semantic_demand_options = if diagnostics {
        semantic_demand_options_for_enablement(
            dialect,
            Path::new(&request.project_id),
            requested_enablement.clone(),
        )?
    } else {
        SemanticDemandOptions::NONE
    };
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
    let facts_complete_ns = started.elapsed().as_nanos();
    if diagnostics && request.check_contracts {
        let statuses = package_contract_statuses(
            dialect,
            Path::new(&facts.project_id),
            &facts,
            &request.contract_paths,
        )?;
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
        let (analysis, diagnostic_timings) = analyze_project_measured_with_enablement(
            dialect,
            Path::new(&facts.project_id),
            &request.sources,
            &facts,
            &request.contract_paths,
            preloaded_bundled,
            requested_enablement,
        )?;
        if !request.emit_contract.is_empty() {
            emit_package_contract(dialect, &request, &analysis.program, &facts)?;
            return Ok(0);
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
        if std::env::var_os("SOLID_CHECKER_TIMINGS").is_some() {
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
                    "irNs": diagnostic_timings.reactive_ir.as_nanos(),
                    "solveAndSnapshotNs": diagnostic_timings.solve_and_snapshot.as_nanos(),
                    "totalNs": started.elapsed().as_nanos(),
                })
            );
        }
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
    let mut contract_paths = Vec::new();
    let mut presets = Vec::new();
    let mut enable_rules = Vec::new();
    let mut format = "default".to_owned();
    let mut certify = false;
    let mut check_contracts = false;
    let mut validate_contract_paths = Vec::new();
    let mut emit_contract = String::new();
    let mut package_name = String::new();
    let mut package_version = String::new();
    let mut declaration_artifact = String::new();
    let mut implementation_artifact = String::new();
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
        if let Some(value) = argument.strip_prefix("--contract=") {
            contract_paths.push(value.into());
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
        if let Some(value) = argument.strip_prefix("--package-name=") {
            package_name = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--package-version=") {
            package_version = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--declaration-artifact=") {
            declaration_artifact = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--implementation-artifact=") {
            implementation_artifact = value.into();
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
            "--contract" => contract_paths.push(args.next().ok_or("--contract needs a path")?),
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
            "--package-name" => package_name = args.next().ok_or("--package-name needs a value")?,
            "--package-version" => {
                package_version = args.next().ok_or("--package-version needs a value")?
            }
            "--declaration-artifact" => {
                declaration_artifact = args.next().ok_or("--declaration-artifact needs a path")?
            }
            "--implementation-artifact" => {
                implementation_artifact = args
                    .next()
                    .ok_or("--implementation-artifact needs a path")?
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
    let project = if !help && validate_contract_paths.is_empty() {
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
        contract_paths,
        presets,
        enable_rules,
        format,
        certify,
        check_contracts,
        validate_contract_paths,
        emit_contract,
        package_name,
        package_version,
        declaration_artifact,
        implementation_artifact,
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
           --contract <PATH>            Override/discover a package contract (repeatable)\n\
           --preset <NAME>              Enable a catalog preset (repeatable)\n\
           --enable-rule <NAME>         Enable one default-disabled rule (repeatable)\n\
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
           --emit-contract <PATH>       Write a generated solid-reactivity.json contract\n\
           --package-name <NAME>        Package name used by --emit-contract\n\
           --package-version <VERSION>  Exact package version used by --emit-contract\n\
           --declaration-artifact <PATH> Hash a declaration artifact into the contract\n\
           --implementation-artifact <PATH> Hash an implementation artifact into the contract\n\
           --typefacts <PATH>           TypeFacts service executable\n\
           --serve                      Run the retained per-project check daemon (Unix only).\n\
                                        Release checks use it by default; set\n\
                                        SOLID_CHECKER_DAEMON=0 for one-shot analysis.\n\
           -h, --help                   Print help"
    );
}

fn emit_package_contract(
    dialect: &'static dialect::Dialect,
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
    if let Some(unresolved) = program
        .static_violations
        .iter()
        .find(|violation| violation.id.starts_with("SC9"))
    {
        return Err(format!(
            "emit package contract: unresolved effect at {}:{}: {}",
            unresolved.location.path, unresolved.location.start_byte, unresolved.message
        )
        .into());
    }
    // The same SC9 class arrives as structured defects (missing contract
    // exports, uncovered execution maps, uncaptured sources); a contract
    // must not be emitted over those either. Unknown callback execution is
    // excluded: it is refused below, from the obligation list that knows the
    // requested entrypoint's exported surface.
    if let Some(unresolved) = program.static_defects.iter().find(|defect| {
        defect.kind.is_unresolved_obligation()
            && !defect.kind.refused_through_generation_obligations()
    }) {
        return Err(format!(
            "emit package contract: unresolved obligation at {}:{}: {:?}",
            unresolved.location.path, unresolved.location.start_byte, unresolved.kind
        )
        .into());
    }
    // An unresolved cleanup value remains a project diagnostic, but it does
    // not change the exported reactive dependency/callback/return summary.
    // Contract generation must only fail on obligations that affect that
    // summary; otherwise untyped implementation details make valid library
    // surfaces impossible to describe.
    let output = Path::new(&request.emit_contract);
    let artifacts = solid_reactive_ir::ContractArtifacts {
        declaration: (!request.declaration_artifact.is_empty())
            .then(|| artifact_for_file(output, Path::new(&request.declaration_artifact)))
            .transpose()?,
        implementation: (!request.implementation_artifact.is_empty())
            .then(|| artifact_for_file(output, Path::new(&request.implementation_artifact)))
            .transpose()?,
    };
    let mut dependency_contracts = request
        .contract_paths
        .iter()
        .map(|path| read_package_contract(Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    for package in dialect.bundled_packages {
        if dependency_contracts
            .iter()
            .any(|contract| contract.package.name == *package)
        {
            continue;
        }
        if let Some(contract) = (dialect.bundled_contract)(package)? {
            dependency_contracts.push(contract);
        }
    }
    let exports = if request.contract_entry_file.is_empty() {
        (*program.contract_exports).clone()
    } else {
        contract_exports_for_entry_file(
            facts,
            program,
            Path::new(&request.contract_entry_file),
            &dependency_contracts,
        )?
    };
    let exported_identities = if request.contract_entry_file.is_empty() {
        HashSet::new()
    } else {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        exports
            .keys()
            .filter_map(|name| entry_export_entity(facts, &entry_file, name))
            .filter(|entity| !entity.runtime_identity.is_empty())
            .map(|entity| entity.runtime_identity.to_string())
            .collect()
    };
    if let Some(unresolved) = program
        .contract_generation_obligations
        .iter()
        .find(|unresolved| {
            request.contract_entry_file.is_empty()
                || if unresolved.function_identity.is_empty() {
                    exports.contains_key(&unresolved.function)
                } else {
                    exported_identities.contains(&unresolved.function_identity)
                }
        })
    {
        return Err(format!(
            "emit package contract: unresolved parameter behavior in {} parameter {} ({}) at {}:{}: {}; required behavior: {}; edit this schema-v1 stub and review its evidence: {}",
            unresolved.function,
            unresolved.parameter,
            unresolved.parameter_type,
            unresolved.location.path,
            unresolved.location.start_byte,
            unresolved.message,
            unresolved.required_execution,
            unresolved.contract_stub
        )
        .into());
    }
    let contract = solid_reactive_ir::PackageContract {
        schema_version: 1,
        package: solid_reactive_ir::ContractPackage {
            name: request.package_name.clone(),
            version: request.package_version.clone(),
            integrity: String::new(),
        },
        compiler_facts_protocol: 1,
        artifacts,
        entrypoints: [(
            ".".into(),
            solid_reactive_ir::ContractEntrypoint {
                exports,
                conditions: Vec::new(),
            },
        )]
        .into(),
        evidence: solid_reactive_ir::ContractEvidence {
            kind: "inferred".into(),
            generator: "solid-checker".into(),
        },
        contract_hash: String::new(),
        source_path: String::new(),
    };
    contract.validate().map_err(|error| error.to_string())?;
    let mut encoded = encode_package_contract(&contract, true)?;
    encoded.push(b'\n');
    fs::write(output, encoded)?;
    Ok(())
}

fn contract_exports_for_entry_file(
    facts: &solid_facts::ProjectFacts,
    program: &solid_reactive_ir::Program,
    entry_file: &Path,
    dependency_contracts: &[solid_reactive_ir::PackageContract],
) -> Result<BTreeMap<String, solid_reactive_ir::ContractExport>, Box<dyn std::error::Error>> {
    let entry_file = entry_file.canonicalize()?;
    let mut visiting = HashSet::new();
    let names = exported_names_for_file(facts, &entry_file, dependency_contracts, &mut visiting)?;
    let symbol_aliases = canonical_symbol_aliases(facts);
    let generated_owner_requirements =
        generated_owner_requirements_by_symbol(facts, program, &symbol_aliases);
    let mut exports = BTreeMap::new();
    for name in names {
        let summary = external_export_summary_for_file(
            facts,
            &entry_file,
            dependency_contracts,
            &name,
            &mut HashSet::new(),
        )
        .or_else(|| {
            program.contract_exports.get(&name).cloned()
        }).ok_or_else(|| {
            format!(
                "emit package contract: entry file {} exports {name:?}, but no semantic summary was produced",
                entry_file.display()
            )
        })?;
        let summary = promote_entry_callable(facts, &entry_file, &name, summary);
        let summary = attach_generated_owner_requirements(
            facts,
            &symbol_aliases,
            &generated_owner_requirements,
            &entry_file,
            &name,
            summary,
        );
        exports.insert(name, summary);
    }
    unify_runtime_alias_summaries(facts, &entry_file, &mut exports);
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
    mut summary: solid_reactive_ir::ContractExport,
) -> solid_reactive_ir::ContractExport {
    let operations = entry_export_entity(facts, entry_file, export_name)
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
    for operation in operations {
        if !summary
            .owner_requirements
            .iter()
            .any(|existing| existing.operation == *operation)
        {
            summary
                .owner_requirements
                .push(solid_reactive_ir::ContractOwnerRequirement {
                    operation: *operation,
                    evidence: None,
                });
        }
    }
    summary
        .owner_requirements
        .sort_by_key(|requirement| match requirement.operation {
            solid_reactive_ir::OwnerRequirementOperation::Effect => 0,
            solid_reactive_ir::OwnerRequirementOperation::Cleanup => 1,
            solid_reactive_ir::OwnerRequirementOperation::Boundary => 2,
            solid_reactive_ir::OwnerRequirementOperation::SettledCleanup => 3,
        });
    summary
}

fn promote_entry_callable(
    facts: &solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
    mut summary: solid_reactive_ir::ContractExport,
) -> solid_reactive_ir::ContractExport {
    if summary.kind != "value" {
        return summary;
    }
    let Some(entity) = entry_export_entity(facts, entry_file, name) else {
        return summary;
    };
    if entity.callability == Some(typefacts::Callability::Callable) {
        summary.kind = "function".into();
    }
    summary
}

fn entry_export_entity<'a>(
    facts: &'a solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
) -> Option<&'a typefacts::EntityFact> {
    entry_export_entity_with_visiting(facts, entry_file, name, &mut HashSet::new())
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
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
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
    facts: &solid_facts::ProjectFacts,
    entry_file: &Path,
    exports: &mut BTreeMap<String, solid_reactive_ir::ContractExport>,
) {
    let mut names_by_identity = BTreeMap::<String, Vec<String>>::new();
    for name in exports.keys() {
        let Some(identity) = entry_export_entity(facts, entry_file, name)
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
            for read in &summary.reactive_reads {
                if !merged.reactive_reads.contains(read) {
                    merged.reactive_reads.push(read.clone());
                }
            }
            for callback in &summary.callbacks {
                if !merged.callbacks.contains(callback) {
                    merged.callbacks.push(callback.clone());
                }
            }
            if merged.returns.is_none() {
                merged.returns = summary.returns.clone();
            }
            if merged.async_behavior.is_empty() {
                merged.async_behavior = summary.async_behavior.clone();
            }
        }
        merged
            .callbacks
            .sort_by_key(|callback| (callback.parameter, callback.execution.clone()));
        merged
            .reactive_reads
            .sort_by(|left, right| (&left.kind, &left.label).cmp(&(&right.kind, &right.label)));
        for name in names {
            exports.insert(name.clone(), merged.clone());
        }
    }
}

fn dependency_export_summary(
    dependency_contracts: &[solid_reactive_ir::PackageContract],
    module: &str,
    name: &str,
) -> Option<solid_reactive_ir::ContractExport> {
    solid_reactive_ir::PackageContract::for_module(dependency_contracts, module)
        .and_then(|contract| contract.exports_for_module(module))
        .and_then(|exports| exports.get(name))
        .cloned()
}

fn external_export_summary_for_file(
    facts: &solid_facts::ProjectFacts,
    path: &Path,
    dependency_contracts: &[solid_reactive_ir::PackageContract],
    name: &str,
    visiting: &mut HashSet<PathBuf>,
) -> Option<solid_reactive_ir::ContractExport> {
    let path = path.canonicalize().ok()?;
    if !visiting.insert(path.clone()) {
        return None;
    }
    let file = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &path))?;
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only && specifier.exported.as_str() == name)
        {
            let local_name = file
                .source_text(specifier.local.span)
                .unwrap_or(specifier.exported.as_str());
            if let Some(module) = export.module.as_deref() {
                if module.starts_with('.') {
                    let target = resolve_relative_export(facts, &path, module).ok()?;
                    if let Some(summary) = external_export_summary_for_file(
                        facts,
                        &target,
                        dependency_contracts,
                        local_name,
                        visiting,
                    ) {
                        return Some(summary);
                    }
                } else if let Some(summary) =
                    dependency_export_summary(dependency_contracts, module, local_name)
                {
                    return Some(summary);
                }
            } else {
                for import in file.ast.imports.iter().filter(|import| !import.type_only) {
                    for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
                        if file.source_text(binding.local.span) != Some(local_name) {
                            continue;
                        }
                        let imported = binding.imported.as_deref().or_else(|| {
                            (binding.kind == solid_facts::ast::ImportKind::Default)
                                .then_some("default")
                        })?;
                        if import.module.starts_with('.') {
                            let target =
                                resolve_relative_export(facts, &path, &import.module).ok()?;
                            if let Some(summary) = external_export_summary_for_file(
                                facts,
                                &target,
                                dependency_contracts,
                                imported,
                                visiting,
                            ) {
                                return Some(summary);
                            }
                        } else if let Some(summary) = dependency_export_summary(
                            dependency_contracts,
                            &import.module,
                            imported,
                        ) {
                            return Some(summary);
                        }
                    }
                }
            }
        }
    }
    for export in file
        .ast
        .exports
        .iter()
        .filter(|export| !export.type_only && export.kind == solid_facts::ast::ExportKind::All)
    {
        let module = export.module.as_deref()?;
        if module.starts_with('.') {
            let target = resolve_relative_export(facts, &path, module).ok()?;
            if let Some(summary) = external_export_summary_for_file(
                facts,
                &target,
                dependency_contracts,
                name,
                visiting,
            ) {
                return Some(summary);
            }
            continue;
        }
        if let Some(summary) = dependency_export_summary(dependency_contracts, module, name) {
            return Some(summary);
        }
    }
    None
}

fn exported_names_for_file(
    facts: &solid_facts::ProjectFacts,
    path: &Path,
    dependency_contracts: &[solid_reactive_ir::PackageContract],
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
    for export in file.ast.exports.iter().filter(|export| !export.type_only) {
        if export.kind == solid_facts::ast::ExportKind::All {
            let module = export
                .module
                .as_deref()
                .ok_or("export-all declaration has no module")?;
            if !module.starts_with('.') {
                let contract = solid_reactive_ir::PackageContract::for_module(
                    dependency_contracts,
                    module,
                )
                .ok_or_else(|| {
                        format!(
                            "emit package contract: cannot statically expand external export-all {module:?} from {}; generate and pass its dependency contract with --contract",
                            path.display()
                        )
                    })?;
                let exports = contract.exports_for_module(module).ok_or_else(|| {
                    format!(
                        "emit package contract: dependency contract for {} has no entrypoint matching {module:?}",
                        contract.package.name
                    )
                })?;
                names.extend(
                    exports
                        .keys()
                        .filter(|name| name.as_str() != "default")
                        .cloned(),
                );
                continue;
            }
            let target = resolve_relative_export(facts, &path, module)?;
            names.extend(exported_names_for_file(
                facts,
                &target,
                dependency_contracts,
                visiting,
            )?);
            continue;
        }
        if export.kind == solid_facts::ast::ExportKind::Default {
            names.insert("default".into());
        }
        names.extend(
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .filter(|specifier| !specifier.type_only)
                .map(|specifier| specifier.exported.to_string()),
        );
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

fn artifact_for_file(
    contract_path: &Path,
    artifact_path: &Path,
) -> Result<solid_reactive_ir::ContractArtifact, Box<dyn std::error::Error>> {
    let contract_directory = contract_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let artifact = artifact_path.canonicalize()?;
    let relative = artifact
        .strip_prefix(&contract_directory)
        .map_err(|_| "package contract artifact must be a file inside the contract directory")?;
    if relative.as_os_str().is_empty() || !artifact.is_file() {
        return Err(
            "package contract artifact must be a file inside the contract directory".into(),
        );
    }
    let data = fs::read(&artifact)?;
    Ok(solid_reactive_ir::ContractArtifact {
        path: relative.to_string_lossy().replace('\\', "/"),
        hash: format!("sha256:{:x}", Sha256::digest(data)),
    })
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
