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
    BackendError, ImportIdentityMeasurement, RequestedRuleEnablement, SemanticDemandOptions,
    SourceFile, TypeFactsSession, analyze_project_measured_with_enablement,
    attest_import_identities, build_project_native_measured_with_demands, contract_identity_scope,
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
    /// The subset of [`Request::contract_paths`] that *this generation run*
    /// produced itself, from the dependency's own installed sources.
    ///
    /// Passed as `--generated-contract` by the package generator's
    /// `ensureGeneratedDependencyContract`. It is provenance the document
    /// cannot carry, and it is what
    /// `PackageContract::kind_claims_are_trusted` needs: a contract merely
    /// discovered at `node_modules/<dep>/solid-reactivity.json` may have been
    /// written by any earlier solid-checker, so its `kind` is re-decided here
    /// unless its evidence says a human or a verifier stood behind it.
    #[serde(default)]
    generated_contract_paths: BTreeSet<String>,
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
            // After the contract, not before: a run that cannot emit a
            // contract has no closure record to attest, and writing the
            // inventory first would leave an attestation of a generation that
            // produced nothing.
            if !request.emit_module_inventory.is_empty() {
                write_module_inventory(&mut typescript, &request)?;
            }
            // Emission normally produces no stdout, and the generator depends
            // on that. `--format json` is a caller explicitly asking for the
            // diagnostics of the same analysis, which is otherwise only
            // obtainable by running the whole project a second time -- and the
            // second run is the one that cannot see which obligations the
            // emitter attributed to which export. The default format is
            // untouched, so the generator's process contract is unchanged.
            if request.format != "json" {
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
                    "importIdentityNs": import_identity_ns,
                    "importIdentityFilesRequested": import_identity.requested,
                    "importIdentityFilesAttested": import_identity.attested,
                    "importIdentityFilesUnknown": import_identity.unknown,
                    "importIdentitySpecifiers": import_identity.specifiers,
                    "importIdentityModules": import_identity.modules,
                    // How binding actually answered. A refusal is silent in the
                    // findings by design, but a defect in the span join, the
                    // scope, or a host's offsets degrades contract coverage
                    // toward nothing, and these two counts are what make that
                    // visible rather than merely quiet.
                    "contractBindingsBound": analysis.program.contract_binding.bound,
                    "contractBindingsRefused": analysis.program.contract_binding.refused,
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
    let mut generated_contract_paths = BTreeSet::new();
    let mut presets = Vec::new();
    let mut enable_rules = Vec::new();
    let mut format = "default".to_owned();
    let mut certify = false;
    let mut check_contracts = false;
    let mut validate_contract_paths = Vec::new();
    let mut emit_contract = String::new();
    let mut emit_module_inventory = String::new();
    let mut runtime_module_resolutions = String::new();
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
        if let Some(value) = argument.strip_prefix("--generated-contract=") {
            contract_paths.push(value.into());
            generated_contract_paths.insert(value.into());
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
        if let Some(value) = argument.strip_prefix("--emit-module-inventory=") {
            emit_module_inventory = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-module-resolutions=") {
            runtime_module_resolutions = value.into();
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
            "--generated-contract" => {
                let path = args.next().ok_or("--generated-contract needs a path")?;
                contract_paths.push(path.clone());
                generated_contract_paths.insert(path);
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
            "--emit-module-inventory" => {
                emit_module_inventory = args.next().ok_or("--emit-module-inventory needs a path")?
            }
            "--runtime-module-resolutions" => {
                runtime_module_resolutions = args
                    .next()
                    .ok_or("--runtime-module-resolutions needs a path")?
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
        generated_contract_paths,
        presets,
        enable_rules,
        format,
        certify,
        check_contracts,
        validate_contract_paths,
        emit_contract,
        emit_module_inventory,
        runtime_module_resolutions,
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
           --generated-contract <PATH>  Same, for a contract this generation run produced\n\
                                        itself from the dependency's own sources. Only such\n\
                                        a contract, or one whose evidence records a review,\n\
                                        may carry an export `kind` across the boundary\n\
                                        without it being re-proved here (repeatable)\n\
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
    for variant in &mut summary.variants {
        marked |= mark_summary_claims_unknown(&mut variant.summary, domains);
    }
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
    solid_reactive_ir::ContractClaim::Unknown(solid_reactive_ir::ContractUnknownClaim::new())
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

/// How an unknown claim was attributed to the exports it was written onto.
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
                summary.reactive_reads.is_unknown()
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

/// The machine-readable half of an unknown-claim decision.
///
/// Schema v1's `unknownClaim` is `additionalProperties: false`, and a loader
/// that predates a new field hard-fails on the document rather than ignoring
/// it -- which is why RFC 0002 rejected recording attribution in the contract.
/// So the reason travels the same way the dependency-boundary refusal does: one
/// stable line of this process's stderr, addressed to
/// `generate-package-contract.mjs`, which records it on the matching
/// `unknown-sentinel` item of `<contract>.review.json` and strips the line from
/// anything a human reads.
///
/// One line per decision, JSON so the fields can grow without a parser change.
/// Both sides pin the pairing:
/// `unknown_claim_attribution_markers_reach_the_review_plan` in
/// rust/crates/solid-facts-backend/tests/contracts_process.rs feeds this
/// binary's real stderr to the generator's real parser.
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
    // `unknown-sentinel` item will carry it -- but "the ladder resolved this
    // obligation to no export at all" is a narrowing decision, and the review
    // plan is where a narrowing decision has to be visible. Leaving it silent
    // made the interesting case (reachability proving no export reaches the
    // obligation) indistinguishable from the analyzer never having seen the
    // obligation, and the reviewer had nothing to check the narrowing against.
    // `generate-package-contract.mjs` turns the empty-export notes into their
    // own review-plan notes rather than attaching them to an item.
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
        "unknownImportPaths": graph.unknown_import_paths,
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
    // SC9 findings are proof obligations, not permission to discard every
    // independently known export. After resolving the requested entrypoint we
    // attribute each one to the narrowest claim domain it can invalidate and
    // emit that claim as explicitly unknown. Consumers then fail closed only
    // when they demand that claim. Proven violations remain diagnostics, but
    // they do not alter the package's descriptive runtime contract.
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
        .map(|path| {
            let mut contract = read_package_contract(Path::new(path))?;
            // Provenance the document cannot carry, stamped where the argv
            // that carries it is still in scope. See `Request::
            // generated_contract_paths` and `kind_claims_are_trusted`.
            contract.run_generated = request.generated_contract_paths.contains(path);
            Ok(contract)
        })
        .collect::<Result<Vec<_>, BackendError>>()?;
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
    let mut exports = if request.contract_entry_file.is_empty() {
        (*program.contract_exports).clone()
    } else {
        contract_exports_for_entry_file(
            facts,
            program,
            Path::new(&request.contract_entry_file),
            &dependency_contracts,
        )?
    };
    let exported_names_by_identity = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(identity) = entry_export_entity(facts, &entry_file, name)
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
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(symbol) = entry_export_entity(facts, &entry_file, name)
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
            summary.callbacks = solid_reactive_ir::ContractClaim::Unknown(
                solid_reactive_ir::ContractUnknownClaim::new(),
            );
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
        run_generated: false,
        installed_root: None,
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
        // `trusted_kind` says the summary's `kind` came from a dependency
        // contract whose provenance licenses carrying it across the boundary,
        // rather than from this project's analysis of its own files. A
        // dependency contract with neither provenance carries every other
        // claim and has its `kind` re-decided here, exactly like a local one.
        // See `promote_entry_callable` and `kind_claims_are_trusted`.
        let (summary, trusted_kind) = external_export_summary_for_file(
            facts,
            &entry_file,
            dependency_contracts,
            &name,
            &mut HashSet::new(),
        )
        .or_else(|| {
            program.contract_exports.get(&name).cloned().map(|summary| (summary, false))
        }).ok_or_else(|| {
            format!(
                "emit package contract: entry file {} exports {name:?}, but no semantic summary was produced",
                entry_file.display()
            )
        })?;
        let summary = promote_entry_callable(facts, &entry_file, &name, summary, trusted_kind)?;
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
                evidence: None,
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
/// `validate_export` bars it from carrying even an unknown domain — so it is
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
/// `trusted_kind` marks a summary whose `kind` came from a *dependency
/// contract* this analysis is entitled to take that one claim from unproved.
/// The entitlement is about provenance and nothing else — see
/// [`solid_reactive_ir::PackageContract::kind_claims_are_trusted`]: either this
/// run generated the contract itself from the dependency's own sources under
/// this exact rule, or its evidence records that a human or a verifier stood
/// behind its claims. Re-deciding such a `kind` here — with the dependency's
/// implementation outside the project and its specifier therefore typed `any` —
/// would refuse exactly the entrypoints that already have the better answer.
///
/// A dependency contract with *neither* provenance is a document of unknown
/// origin found on disk; its `kind` goes through this decision like any local
/// claim, because it may have been generated by an earlier solid-checker whose
/// `Unknown ⇒ value` defect is the one this rule exists to close. Any carried
/// summary can still be *raised* by a local signature fact.
fn promote_entry_callable(
    facts: &solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
    summary: solid_reactive_ir::ContractExport,
    trusted_kind: bool,
) -> Result<solid_reactive_ir::ContractExport, Box<dyn std::error::Error>> {
    if summary.kind != "value" {
        return Ok(summary);
    }
    let Some(entity) = entry_export_entity(facts, entry_file, name) else {
        return Ok(summary);
    };
    let refuse = |reason: String| -> Result<_, Box<dyn std::error::Error>> {
        Err(format!(
            "emit package contract: entry file {} exports {name:?}, {reason}; publishing kind \"value\" would certify it invokes no caller-supplied callback",
            entry_file.display()
        )
        .into())
    };
    match solid_reactive_ir::export_kind_proof(facts, &entity.location) {
        // A call signature or a construct signature; either is
        // `typeof === "function"` at runtime, and the type system reads a
        // construct signature as *not* a call signature, so a class arrives
        // here through constructability alone. The raise leaves `callbacks`
        // unknown for both: a summary still saying `value` here is one whose
        // body was never analyzed, so its silence about callbacks is not a
        // claim either. See `solid_reactive_ir::raised_function_export`.
        solid_reactive_ir::ExportKindProof::Callable => {
            Ok(solid_reactive_ir::raised_function_export(summary))
        }
        solid_reactive_ir::ExportKindProof::Unresolvable(callability, constructability)
            if !trusted_kind =>
        {
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
        solid_reactive_ir::ExportKindProof::Unanswered if !trusted_kind => {
            refuse("whose runtime kind no fact covers at all".into())
        }
        solid_reactive_ir::ExportKindProof::NonCallable
        | solid_reactive_ir::ExportKindProof::Unresolvable(_, _)
        | solid_reactive_ir::ExportKindProof::Unanswered => Ok(summary),
    }
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
                (_, solid_reactive_ir::ContractClaim::Unknown(unknown)) => {
                    merged.reactive_reads =
                        solid_reactive_ir::ContractClaim::Unknown(unknown.clone());
                }
                (solid_reactive_ir::ContractClaim::Unknown(_), _) => {}
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
                (_, solid_reactive_ir::ContractClaim::Unknown(unknown)) => {
                    merged.callbacks = solid_reactive_ir::ContractClaim::Unknown(unknown.clone());
                }
                (solid_reactive_ir::ContractClaim::Unknown(_), _) => {}
            }
            match (&mut merged.returns, &summary.returns) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_return),
                    solid_reactive_ir::ContractClaim::Known(returned),
                ) if merged_return.is_none() => *merged_return = returned.clone(),
                (_, solid_reactive_ir::ContractClaim::Unknown(unknown)) => {
                    merged.returns = solid_reactive_ir::ContractClaim::Unknown(unknown.clone());
                }
                _ => {}
            }
            match (&mut merged.async_behavior, &summary.async_behavior) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_behavior),
                    solid_reactive_ir::ContractClaim::Known(behavior),
                ) if merged_behavior.is_empty() => *merged_behavior = behavior.clone(),
                (_, solid_reactive_ir::ContractClaim::Unknown(unknown)) => {
                    merged.async_behavior =
                        solid_reactive_ir::ContractClaim::Unknown(unknown.clone());
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

/// One dependency contract's summary for `name`, and whether its `kind` claim
/// may be carried across the boundary without being re-proved.
///
/// The second half is the contract's provenance, not the summary's content:
/// see [`solid_reactive_ir::PackageContract::kind_claims_are_trusted`]. Every
/// other claim in the summary is used regardless — a contract is the only
/// evidence there is about a package this project cannot see into.
///
/// **This lookup binds by module *name*, not by attested identity, and that is
/// deliberate — but it is a weaker rule than the one analysis uses.** The
/// obligations of the package under generation are computed through
/// `resolve_contract_imports`, which binds by identity; these emitted
/// *dependency* claims are computed here, by name. The two can only disagree
/// for a package whose own tsconfig shadows one of its declared dependencies
/// with a `paths` entry, which is a shape no corpus package has and which no
/// generator flow produces: `ensureGeneratedDependencyContract`
/// (packages/cli/scripts/generate-package-contract.mjs) generates these
/// contracts from the dependency's *installed* sources in this same run, and
/// the specifiers consulted here are the package's own imports of those
/// declared dependencies. Threading the declaration span through both call
/// sites would move generation-side answers, so the divergence is recorded in
/// docs/precision-backlog.md instead of narrowed here.
fn dependency_export_summary(
    dependency_contracts: &[solid_reactive_ir::PackageContract],
    module: &str,
    name: &str,
) -> Option<(solid_reactive_ir::ContractExport, bool)> {
    let contract = solid_reactive_ir::PackageContract::for_module(dependency_contracts, module)?;
    let summary = contract.exports_for_module(module)?.get(name)?.clone();
    Some((summary, contract.kind_claims_are_trusted()))
}

/// The summary a *dependency contract* supplies for the export `name` of
/// `path`, following this project's own re-export and import chains to reach
/// the boundary, and whether that contract's `kind` claim is carried unproved.
fn external_export_summary_for_file(
    facts: &solid_facts::ProjectFacts,
    path: &Path,
    dependency_contracts: &[solid_reactive_ir::PackageContract],
    name: &str,
    visiting: &mut HashSet<PathBuf>,
) -> Option<(solid_reactive_ir::ContractExport, bool)> {
    let path = path.canonicalize().ok()?;
    if !visiting.insert(path.clone()) {
        return None;
    }
    let file = facts
        .files
        .iter()
        .find(|file| same_canonical_path(Path::new(file.path.as_str()), &path))?;
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace`, `declare module`, or `declare global` body binds a member
    // of that namespace object, not a name this module publishes. See
    // `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
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

/// The machine-readable half of a missing-dependency refusal.
///
/// Contract generation is demand-driven across package boundaries: when this
/// entrypoint re-exports a package whose contract was not supplied, the
/// generator (`ensureGeneratedDependencyContract` in
/// packages/cli/scripts/generate-package-contract.mjs) generates exactly that
/// installed dependency and retries. It needs the module specifier, and the
/// only channel it has is this process's stderr.
///
/// Parsing that specifier back out of the human sentence couples the generator
/// to prose: reword the message and the recursion silently stops, which
/// surfaces only as an entrypoint the generator "refused" -- an outcome that
/// exits 0. So the boundary emits one stable line of its own, in addition to
/// the unchanged human message. Both sides pin the pairing:
/// `package_generator_dependency_boundary_marker_drives_recursion` in
/// rust/crates/solid-facts-backend/tests/contracts_process.rs feeds this
/// binary's real stderr to the generator's real parser.
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
        "emit package contract: cannot statically expand external export-all {module:?} from {}; generate and pass its dependency contract with --contract",
        from.display()
    )
    .into())
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
                // Name binding, for the same reason as
                // `dependency_export_summary`: a generated dependency contract
                // describes the dependency's installed sources as this run
                // generated them, and the specifier is the package's own
                // re-export of a dependency it declares. A `paths`-shadowed
                // dependency inside a package under generation would diverge
                // from the identity-bound answer analysis uses, which is the
                // residue recorded in docs/precision-backlog.md.
                let Some(contract) =
                    solid_reactive_ir::PackageContract::for_module(dependency_contracts, module)
                else {
                    return refuse_unresolved_dependency_module(module, &path);
                };
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
/// Walking the same relative re-export and import chain
/// `external_export_summary_for_file` walks, a name whose every export is
/// marked `type_only` somewhere along it binds nothing at runtime, so omitting
/// it is exactly right — and exactly what the marked spelling already gets.
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
