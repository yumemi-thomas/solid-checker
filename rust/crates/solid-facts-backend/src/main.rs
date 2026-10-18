mod daemon_cache;
mod daemon_resolution;
mod idle_memory;
mod json_output;
mod snapshot_emission;

use std::{
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use serde::Deserialize;
use solid_facts_backend::{BackendError, SourceFile, default_typefacts_executable};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeRendering, RuntimeTarget};
// Shared with the library's check operation (moved from this binary).
use solid_facts_backend::dependency_program;

#[derive(Clone, Deserialize)]
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
    presets: Vec<String>,
    #[serde(default)]
    enable_rules: Vec<String>,
    #[serde(default = "json_format")]
    format: String,
    #[serde(default)]
    certify: bool,
    #[serde(default)]
    feedback_facts: bool,
    /// Resolve every module load through the project's own bundler, which
    /// runs its config (ADR 0220).
    #[serde(default)]
    runtime_resolution: bool,
    /// ADR 0284, experimental: installed packages, by bare specifier, whose
    /// runtime source is analyzed as project code instead of being bound
    /// through a package contract.
    #[serde(default)]
    dependency_sources: Vec<String>,
    /// ADR 0295: the declared premise that the selected dependency-source
    /// modules run in an HTML Window realm (`--runtime-realm window`), which
    /// decides their `typeof window` and `typeof document` guards. Only with
    /// `--runtime-target browser` and `--dependency-source`.
    #[serde(default)]
    window_realm: bool,
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
    // ADR 0297: the effective dependency selection, decided once and before
    // the daemon branch reads it, so a retained check serves the selection a
    // one-shot check analyzes.
    let (dependency_sources, implicit_dependency_sources) =
        solid_facts_backend::check::effective_dependency_selection(
            Path::new(&request.project_id),
            &request.dependency_sources,
            defaults_to_dependency_sources(&request),
            request.window_realm,
            &mut |line| eprintln!("{line}"),
        )?;
    request.dependency_sources = dependency_sources;
    // `detect_detailed` rather than `detect`: an installed Solid runtime this
    // build has no dialect for is not a dialect choice, and collapsing it onto
    // the default would analyze the project under a language it does not run.
    //
    // An explicit `--dialect` overrides detection outright, refusal included.
    // That is the escape hatch for a resolved manifest that misreports what
    // will actually be installed, and the rule page says so.
    let (dialect, unsupported_runtime) = solid_facts_backend::check::select_dialect(
        Path::new(&request.project_id),
        request.dialect.as_deref(),
    )?;
    // Refuse here: **before** the daemon branch below, not after it.
    //
    // `daemon::enabled()` defaults to on whenever `debug_assertions` is off,
    // and `daemon::eligible` is satisfied by the ordinary project check, so a
    // release build takes `daemon::check` and returns from this function
    // without ever reaching the analysis below. A refusal placed there would
    // be correct in a debug build and silently absent in every shipped one.
    if let Some(solid_facts_backend::check::UnsupportedRuntime {
        installed,
        manifest,
        refusal,
    }) = unsupported_runtime
    {
        if request.runtime.target.is_none() {
            eprintln!(
                "solid-checker: note: browser host not inferred: {}: unsupported installed Solid runtime {installed}",
                manifest.display()
            );
        }
        let snapshot =
            solid_facts_backend::unsupported_runtime_snapshot(&installed, &manifest, refusal);
        let emission = snapshot_emission::emit(
            dialect,
            &request.format,
            &request.project_id,
            &snapshot,
            request.certify,
            started.elapsed(),
        )?;
        io::stdout().write_all(&emission.output)?;
        return Ok(emission.exit_code);
    }
    // ADR 0284: refused before the daemon branch, so a retained check
    // (ADR 0296) refuses exactly as a one-shot one does.
    if !request.dependency_sources.is_empty() {
        dependency_program::host(&request.runtime)?;
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
    run_ordinary_check(&request, dialect, implicit_dependency_sources, started)
}

fn check_options(
    request: &Request,
    dialect: &'static solid_facts_backend::dialect::Dialect,
    implicit_dependency_sources: bool,
) -> solid_facts_backend::check::Options {
    solid_facts_backend::check::Options {
        project_id: request.project_id.clone(),
        dialect,
        generation: request.generation,
        sources: request.sources.clone(),
        runtime: request.runtime.clone(),
        presets: request.presets.clone(),
        enable_rules: request.enable_rules.clone(),
        feedback_facts: request.feedback_facts,
        dependency_sources: request.dependency_sources.clone(),
        implicit_dependency_sources,
        window_realm: request.window_realm,
        runtime_resolution: request.runtime_resolution,
    }
}

/// Runs the shared ordinary check with producer child processes, then renders
/// its snapshot under the project's own identity.
fn run_ordinary_check(
    request: &Request,
    dialect: &'static solid_facts_backend::dialect::Dialect,
    implicit_dependency_sources: bool,
    started: Instant,
) -> Result<i32, Box<dyn std::error::Error>> {
    let mut sessions = solid_facts_backend::check::ProcessSessions::new(
        request.typefacts_executable.clone(),
        producer_arguments(&request.typefacts_args),
    );
    let prelude = started.elapsed();
    let outcome = solid_facts_backend::check::run(
        &check_options(request, dialect, implicit_dependency_sources),
        &mut sessions,
        &mut |line| eprintln!("{line}"),
    )?;
    let emission = snapshot_emission::emit(
        dialect,
        &request.format,
        &request.project_id,
        &outcome.analysis.snapshot,
        request.certify,
        started.elapsed(),
    )?;
    io::stdout().write_all(&emission.output)?;
    if std::env::var_os("SOLID_CHECKER_TIMINGS").is_some() {
        let at = |elapsed: std::time::Duration| (prelude + elapsed).as_nanos();
        let sidecar_spawn_ns = at(outcome.sidecar_spawn);
        let source_setup_ns = at(outcome.source_setup);
        let facts_complete_ns = at(outcome.facts_complete);
        let import_identity = outcome.import_identity;
        eprintln!(
            "{}",
            serde_json::json!({
                "sidecarSpawnNs": sidecar_spawn_ns,
                "sourcesFetchNs": source_setup_ns.saturating_sub(sidecar_spawn_ns),
                "sourcesBytes": outcome.sources_bytes,
                "sourcesWireBytes": 0,
                "sourceSetupNs": source_setup_ns,
                "sourceAnalysisNs": outcome.native_timings.source_analysis.as_nanos(),
                "typeFactsNs": outcome.native_timings.type_facts.as_nanos(),
                "factsTotalNs": facts_complete_ns.saturating_sub(source_setup_ns),
                "importIdentityNs": outcome.import_identity_elapsed.as_nanos(),
                "importIdentityFilesRequested": import_identity.requested,
                "importIdentityFilesAttested": import_identity.attested,
                "importIdentityFilesUnknown": import_identity.unknown,
                "importIdentitySpecifiers": import_identity.specifiers,
                "importIdentityModules": import_identity.modules,
                "irNs": outcome.diagnostic_timings.reactive_ir.as_nanos(),
                "solveAndSnapshotNs": outcome.diagnostic_timings.solve_and_snapshot.as_nanos(),
                "contractEmissionNs": 0,
                "moduleInventoryNs": 0,
                "totalNs": started.elapsed().as_nanos(),
            })
        );
    }
    Ok(emission.exit_code)
}

/// ADR 0297: an ordinary analysis whose host declares both a target and a
/// build analyzes every Solid package from source, as `--dependency-source
/// '*'` does, unless an explicit selection says otherwise. A package `*` does
/// not select or refuses stays uncertifiable.
///
/// Every other mode keeps the selection it was given. Contract emission and
/// its batch, the module inventory and resolutions it reads, probe and
/// certification planning, contract inspection and validation, feedback
/// facts, bundler runtime resolution, caller-supplied sources and the daemon
/// server (which serves its client's selection verbatim) are not ordinary
/// analyses, and none of them has been shown to mean the same thing under a
/// source selection.
fn defaults_to_dependency_sources(request: &Request) -> bool {
    let declared_host = request.runtime.target.is_some() && request.runtime.build.is_some();
    let ordinary = !request.serve
        && !request.help
        && request.sources.is_empty()
        && !request.feedback_facts
        && !request.runtime_resolution
        && env!("CARGO_BIN_NAME") == "solid-checker-rust";
    declared_host && ordinary && request.dependency_sources.is_empty()
}

#[cfg(test)]
mod default_dependency_source_tests {
    use super::{Request, defaults_to_dependency_sources};
    use solid_reactive_ir::{RuntimeBuild, RuntimeTarget};

    fn request(target: bool, build: bool) -> Request {
        let mut request: Request = serde_json::from_value(serde_json::json!({
            "projectId": "/project/tsconfig.json",
            "generation": 1,
            "sources": [],
            "typefactsExecutable": "solid-typefacts",
        }))
        .unwrap();
        request.runtime.target = target.then_some(RuntimeTarget::Browser);
        request.runtime.build = build.then_some(RuntimeBuild::Development);
        request
    }

    #[test]
    fn only_an_ordinary_analysis_under_a_declared_target_and_build_defaults_to_source() {
        assert!(defaults_to_dependency_sources(&request(true, true)));
        assert!(!defaults_to_dependency_sources(&request(true, false)));
        assert!(!defaults_to_dependency_sources(&request(false, true)));
        assert!(!defaults_to_dependency_sources(&request(false, false)));
        let mut named = request(true, true);
        named.dependency_sources = vec!["dep".into()];
        assert!(!defaults_to_dependency_sources(&named));
    }

    #[test]
    fn no_other_mode_defaults_to_source() {
        type Mode = (&'static str, fn(&mut Request));
        let modes: &[Mode] = &[
            ("serve", |r| r.serve = true),
            ("help", |r| r.help = true),
            ("feedback-facts", |r| r.feedback_facts = true),
            ("runtime-resolution", |r| r.runtime_resolution = true),
        ];
        for (mode, set) in modes {
            let mut request = request(true, true);
            set(&mut request);
            assert!(!defaults_to_dependency_sources(&request), "{mode}");
        }
        let mut supplied = request(true, true);
        supplied.sources = vec![
            serde_json::from_value(serde_json::json!({
                "path": "/project/App.tsx",
                "source": "",
            }))
            .unwrap(),
        ];
        assert!(
            !defaults_to_dependency_sources(&supplied),
            "caller-supplied sources"
        );
    }
}

/// The flags package contracts took, each refused with a message rather than
/// read as an unknown argument.
const REMOVED_CONTRACT_FLAGS: &[&str] = &[
    "--accepted-contracts",
    "--receipt-trust-configuration",
    "--proposal-dependencies",
    "--no-bundled-contracts",
    "--check-contracts",
    "--validate-contract",
    "--emit-contract",
    "--emit-contract-batch",
    "--contract-batch-results",
    "--emit-module-inventory",
    "--runtime-module-resolutions",
    "--contract-resolution",
    "--emit-proposal-plan",
    "--merge-contract",
    "--merge-contract-output",
    "--merge-proposal-plan",
    "--merge-proposal-plan-output",
    "--review-contract",
    "--review-output",
    "--declaration-probe-plan",
    "--runtime-probe-proposal",
    "--runtime-probe-proposal-plan",
    "--runtime-probe-request",
    "--runtime-probe-plan-output",
    "--runtime-probe-runs",
    "--runtime-probe-evaluation-output",
    "--plan-contract-certification",
    "--certification-plan-output",
    "--execute-contract-certification",
    "--verify-policy2-discovery",
    "--package-name",
    "--package-version",
    "--contract-entry-file",
    "--contract-package-root",
    "--no-dependency-source",
];

fn request_from_args() -> Result<Request, Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mut project = PathBuf::from("tsconfig.json");
    let mut typefacts = default_typefacts_executable();
    let mut dialect_id: Option<String> = None;
    // The export conditions this project resolves its imports under. Declared
    // by the host because the analyzer has no condition facts of its own, and
    // conditions select the artifact an acceptance was issued for.
    let mut export_conditions = std::collections::BTreeSet::<String>::new();
    let mut presets = Vec::new();
    let mut enable_rules = Vec::new();
    let mut format = "default".to_owned();
    let mut certify = false;
    let mut feedback_facts = false;
    let mut runtime_resolution = false;
    let mut dependency_sources = Vec::new();
    let mut runtime = RuntimeEnvironment::default();
    let mut window_realm = false;
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
        if let Some(value) = argument.strip_prefix("--runtime-target=") {
            runtime.target = Some(parse_runtime_target(value)?);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-build=") {
            runtime.build = Some(parse_runtime_build(value)?);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-realm=") {
            window_realm = parse_runtime_realm(value)?;
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
            "--conditions" => {
                let value = args
                    .next()
                    .ok_or("--conditions needs a comma-separated list")?;
                export_conditions.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|condition| !condition.is_empty())
                        .map(str::to_owned),
                );
            }
            "--preset" => presets.push(args.next().ok_or("--preset needs a name")?),
            "--enable-rule" => {
                enable_rules.push(args.next().ok_or("--enable-rule needs a rule name")?)
            }
            "--format" => format = args.next().ok_or("--format needs a value")?,
            "--certify" => certify = true,
            "--feedback-facts" => feedback_facts = true,
            "--runtime-resolution" => {
                runtime_resolution = match args.next().as_deref() {
                    Some("required") => true,
                    Some("off") => false,
                    _ => return Err("--runtime-resolution needs required or off".into()),
                }
            }
            "--serve" => serve = true,
            // Internal: the whole declared host a dependency-source daemon
            // serves (ADR 0296), as its client stated it.
            "--daemon-runtime" => {
                runtime = serde_json::from_str(
                    &args.next().ok_or("--daemon-runtime needs a JSON value")?,
                )?
            }
            "--help" | "-h" => help = true,
            "--dependency-source" => dependency_sources.push(
                args.next()
                    .ok_or("--dependency-source needs a package specifier")?,
            ),
            "--runtime-realm" => {
                window_realm =
                    parse_runtime_realm(&args.next().ok_or("--runtime-realm needs a value")?)?;
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
            // Accepted for compatibility: no check consumes a package contract.
            "--no-package-contracts" => {}
            removed
                if REMOVED_CONTRACT_FLAGS
                    .contains(&removed.split_once('=').map_or(removed, |(flag, _)| flag)) =>
            {
                return Err(format!(
                    "{removed} was removed with package contracts: a package is analyzed \
                     from source or its behaviour stays uncertifiable"
                )
                .into());
            }
            unknown => return Err(format!("unknown argument {unknown:?}").into()),
        }
    }
    // ADR 0295: the realm is a premise about the selected packages' runtime
    // in a browser; `--runtime-target browser` alone states no realm, and no
    // other analysis reads one.
    // Whether any package is selected is decided in `run`, after the default
    // selection of ADR 0297.
    if window_realm && runtime.target != Some(RuntimeTarget::Browser) {
        return Err("--runtime-realm window needs --runtime-target browser".into());
    }
    let project = if help {
        project
    } else {
        project.canonicalize()?
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
        presets,
        enable_rules,
        format,
        certify,
        feedback_facts,
        runtime_resolution,
        dependency_sources,
        window_realm,
        help,
        serve,
        // Both spellings, merged. `--conditions` takes a comma-separated list
        // and `--runtime-condition` one name at a time; overwriting here
        // discarded the latter entirely, and `packages/cli/eslint.cjs` emits
        // exactly that flag for every configured condition. Since conditions
        // select the artifact, dropping them silently disabled artifact
        // admission for every ESLint-driven run.
        runtime: RuntimeEnvironment {
            conditions: {
                let mut conditions = runtime.conditions.clone();
                conditions.extend(export_conditions);
                conditions
            },
            ..runtime
        },
    })
}

fn parse_runtime_target(value: &str) -> Result<RuntimeTarget, Box<dyn std::error::Error>> {
    match value {
        "browser" => Ok(RuntimeTarget::Browser),
        "node" => Ok(RuntimeTarget::Node),
        _ => Err(format!("unknown runtime target {value:?}; expected browser or node").into()),
    }
}

/// ADR 0295: `window` is the one realm a premise is defined for.
fn parse_runtime_realm(value: &str) -> Result<bool, Box<dyn std::error::Error>> {
    match value {
        "window" => Ok(true),
        _ => Err(format!("unknown runtime realm {value:?}; expected window").into()),
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
           --format <FORMAT>            default: violations, then findings to review, then\n\
                                analysis-coverage gaps grouped by cause; full: every\n\
                                finding in place; text; json (default: default)\n\
           --dialect <ID>               Solid dialect (default: detect from solid-js; fallback: solid-v2)\n\
           --certify                    Exit 1 unless the project is certified\n\
           --runtime-resolution <MODE>  required: resolve module loads through the\n\
                                        project's own Vite, which runs its config\n\
                                        (SOLID_CHECKER_RUNTIME_RESOLVER names the\n\
                                        worker script); off (default)\n\
           --preset <NAME>              Enable a catalog preset (repeatable)\n\
           --enable-rule <NAME>         Explicitly enable one rule (repeatable)\n\
           --runtime-target <browser|node>\n\
                                        Select the runtime target explicitly\n\
           --runtime-build <development|production>\n\
                                        Select the build mode explicitly\n\
           --dependency-source <SPECIFIER>\n\
                                        Analyze an installed package from its runtime source\n\
                                        (repeatable; `*` selects every\n\
                                        Solid package). Requires --runtime-target and\n\
                                        --runtime-build; with both declared, `*` is the default\n\
           --runtime-realm <window>     Declare that the dependency-source packages run\n\
                                        in an HTML Window realm, which decides their\n\
                                        `typeof window`/`typeof document` guards.\n\
                                        Requires --runtime-target browser\n\
           --rendering <csr|string-ssr|streaming-ssr>\n\
                                        Select the rendering mode explicitly\n\
           --runtime-condition <NAME>   Select an exact package/runtime condition\n\
           --framework-transform <NAME> Select an explicit framework/compiler transform\n\
           --program-boundary <open|closed>\n\
                                        Assert whether code outside this project may import\n\
                                        from it. `closed` lets an exported symbol's caller set\n\
                                        be enumerated; it never licenses guessing one\n\
           --typefacts <PATH>           TypeFacts service executable\n\
           --serve                      Run the retained per-project check daemon (Unix only).\n\
                                        Release checks use it by default; set\n\
                                        SOLID_CHECKER_DAEMON=0 for one-shot analysis.\n\
           -h, --help                   Print help"
    );
}

fn render_program_error(error: &(dyn std::error::Error + 'static)) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "solid-facts-backend".into());
    format!("{program}: {error}")
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
            eprintln!("{}", render_program_error(error.as_ref()));
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
