//! The ordinary project check, as one library operation the native CLI and
//! the owned WASM host both run.
//!
//! It is exactly the native one-shot composition, in its order: the original
//! program's sources and inventory, dependency-source selection and
//! preparation, the combined program opened at its logical path, the selected
//! host fold against the original program's hashes, semantic facts, source-root
//! and runtime redirects, import attestation, the package boundary with its
//! refusals, the requirement-only package index with host inference, retained-export decisions, and analysis. Only the producer
//! programs differ between callers: a [`ProducerSessions`] opens them, as
//! child processes natively or as owned reactor programs in WASM.
//!
//! Formatting, stdout and exit codes, and the daemon's retained sessions stay
//! with their callers.

use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use solid_reactive_ir::RuntimeEnvironment;

use crate::{
    DiagnosticAnalysis, DiagnosticTimings, ImportIdentityMeasurement, NativeBuildTimings,
    RequestedRuleEnablement, SourceFile, TypeFactsSession, attest_import_identities,
    build_project_native_measured_with_program_hashes, dependency_program,
    dependency_sources::{PreparedDependencyProgram, SourceRefusal},
    dialect::Dialect,
    import_identity_scope,
};

/// Everything an ordinary check reads besides its producer programs.
#[derive(Clone, Debug)]
pub struct Options {
    /// The project's tsconfig, the identity findings are reported under.
    pub project_id: String,
    pub dialect: &'static Dialect,
    pub generation: u64,
    /// Analyzed sources; empty means the producer's configured sources.
    pub sources: Vec<SourceFile>,
    pub runtime: RuntimeEnvironment,
    pub presets: Vec<String>,
    pub enable_rules: Vec<String>,
    pub feedback_facts: bool,
    /// The effective dependency-source selection (names or `*`).
    pub dependency_sources: Vec<String>,
    /// The selection was the implicit default rather than a request: a
    /// generated program that cannot be made available falls back to no
    /// selection with a note instead of failing.
    pub implicit_dependency_sources: bool,
    pub window_realm: bool,
    pub runtime_resolution: bool,
}

/// Opens the producer programs a check reads. Two implementations exist: child
/// processes (native) and owned reactor programs (WASM).
pub trait ProducerSessions {
    /// The project's own program.
    fn open(&mut self, project_id: &str) -> Result<TypeFactsSession, Box<dyn Error>>;
    /// The generated combined program, overlaid at its logical path. `Ok(Err)`
    /// means the program could not be made available to the producer (a
    /// native scratch write); the check may then fall back to no selection.
    fn open_generated(
        &mut self,
        program: &PreparedDependencyProgram,
    ) -> Result<Result<(String, TypeFactsSession), String>, Box<dyn Error>>;
}

/// One check's answer and what it cost.
pub struct Outcome {
    pub analysis: Arc<DiagnosticAnalysis>,
    pub diagnostic_timings: DiagnosticTimings,
    pub native_timings: NativeBuildTimings,
    pub import_identity: ImportIdentityMeasurement,
    /// The analyzed program's identity: the project, or the combined program.
    pub analyzed_project_id: String,
    pub sidecar_spawn: Duration,
    pub source_setup: Duration,
    pub facts_complete: Duration,
    pub import_identity_elapsed: Duration,
    pub sources_bytes: usize,
}

/// The project's program, prepared: the session analysis reads, the selected
/// dependency program when any package was selected, and preparation's
/// refusals.
pub struct OpenedProgram {
    pub typescript: TypeFactsSession,
    pub dependency_sources: Option<PreparedDependencyProgram>,
    pub source_refusals: Vec<SourceRefusal>,
    /// The program the session opened: the project, or the combined program's
    /// logical path beside it.
    pub project_id: String,
}

/// Opens the program a check analyzes (ADR 0284): the project's own, or, when
/// packages are selected, the combined program after the original program's
/// sources and inventory decided the selection. Notes go to `note` as they
/// arise, so a caller that prints them sees them even when this fails.
pub fn open_program(
    options: &Options,
    sessions: &mut dyn ProducerSessions,
    note: &mut dyn FnMut(&str),
) -> Result<OpenedProgram, Box<dyn Error>> {
    let host = if options.dependency_sources.is_empty() {
        None
    } else {
        Some(dependency_program::host(&options.runtime)?)
    };
    let Some(host) = host else {
        return Ok(OpenedProgram {
            typescript: sessions.open(&options.project_id)?,
            dependency_sources: None,
            source_refusals: Vec::new(),
            project_id: options.project_id.clone(),
        });
    };
    let mut configured = sessions.open(&options.project_id)?;
    let (notes, prepared) = dependency_program::prepare(
        Path::new(&options.project_id),
        &options.dependency_sources,
        &host,
        options.dialect,
        &mut configured,
    )?;
    drop(configured);
    for line in &notes.lines {
        note(line);
    }
    let source_refusals = notes.refusals;
    let Some(program) = prepared? else {
        return Ok(OpenedProgram {
            typescript: sessions.open(&options.project_id)?,
            dependency_sources: None,
            source_refusals,
            project_id: options.project_id.clone(),
        });
    };
    match sessions.open_generated(&program)? {
        Ok((project_id, typescript)) => Ok(OpenedProgram {
            typescript,
            dependency_sources: Some(program),
            source_refusals,
            project_id,
        }),
        // Review round 1 of ADR 0297: the default adds no write requirement an
        // analysis without dependency sources did not have.
        Err(error) if options.implicit_dependency_sources => {
            note(&format!(
                "solid-checker: note: packages are not analyzed from source by \
                 default: {error}; their behaviour stays uncertifiable"
            ));
            Ok(OpenedProgram {
                typescript: sessions.open(&options.project_id)?,
                dependency_sources: None,
                source_refusals,
                project_id: options.project_id.clone(),
            })
        }
        Err(error) => Err(error.into()),
    }
}

/// Runs one ordinary check.
pub fn run(
    options: &Options,
    sessions: &mut dyn ProducerSessions,
    note: &mut dyn FnMut(&str),
) -> Result<Outcome, Box<dyn Error>> {
    let started = Instant::now();
    let OpenedProgram {
        mut typescript,
        dependency_sources,
        source_refusals,
        project_id,
    } = open_program(options, sessions, note)?;
    let sidecar_spawn = started.elapsed();
    let mut sources_bytes = 0;
    let sources = if options.sources.is_empty() {
        let sources = typescript.configured_sources()?;
        sources_bytes = sources.iter().map(|source| source.source.len()).sum();
        sources
    } else {
        options.sources.clone()
    };
    let source_setup = started.elapsed();
    let requested_enablement = || RequestedRuleEnablement {
        presets: &options.presets,
        rules: &options.enable_rules,
        runtime: options.runtime.clone(),
        feedback_facts: options.feedback_facts,
    };
    let semantic_demand_options = crate::semantic_demand_options_for_enablement(
        options.dialect,
        Path::new(&project_id),
        requested_enablement(),
    )?;
    let (folded, program_hashes) = match &dependency_sources {
        Some(dependencies) => {
            let folded = dependency_program::fold_sources(
                &sources,
                dependencies,
                &options.runtime,
                options.window_realm,
            );
            (folded.sources, folded.program_hashes)
        }
        None => (sources.clone(), HashMap::new()),
    };
    let (mut facts, native_timings) = build_project_native_measured_with_program_hashes(
        options.dialect,
        project_id.clone(),
        options.generation,
        folded,
        &program_hashes,
        &mut typescript,
        semantic_demand_options,
    )?;
    // ADR 0284: bind the selected packages' imports to their runtime files.
    if let Some(program) = &dependency_sources {
        dependency_program::bind(&mut facts, &mut typescript, program)?;
    }
    let facts_complete = started.elapsed();
    let mut import_identity = ImportIdentityMeasurement::default();
    let scope = import_identity_scope(&facts);
    if !scope.is_empty() {
        let (index, measurement) = attest_import_identities(&mut typescript, &scope)?;
        facts.resolved_imports = Some(index);
        import_identity = measurement;
    }
    if options.runtime_resolution {
        let directory = project_directory(&facts.project_id);
        let (index, measurement) =
            crate::runtime_resolution::resolve_runtime_imports(&facts, &directory);
        if std::env::var_os("SOLID_CHECKER_TIMINGS").is_some() {
            note(&format!(
                "runtime resolution: {} of {} loads answered ({})",
                measurement.answered, measurement.loads, measurement.status
            ));
        }
        facts.runtime_resolutions = Some(index);
    }
    let import_identity_elapsed = started.elapsed().saturating_sub(facts_complete);

    let directory = project_directory(&facts.project_id);
    let (boundary, inferred) = package_inputs(
        &directory,
        &facts,
        &source_refusals,
        &options.runtime,
        options.dialect,
    )?;
    let crate::InferredHost {
        hosts,
        note: inference_note,
    } = inferred;
    if let Some(inference_note) = inference_note {
        note(&inference_note);
    }
    let (analysis, diagnostic_timings) = crate::analyze_project_inputs_measured_with_enablement(
        options.dialect,
        Path::new(&facts.project_id),
        &sources,
        &facts,
        solid_reactive_ir::AnalysisInputs {
            boundary: &boundary,
            hosts: hosts.as_ref(),
        },
        requested_enablement(),
    )?;
    Ok(Outcome {
        analysis,
        diagnostic_timings,
        native_timings,
        import_identity,
        analyzed_project_id: project_id,
        sidecar_spawn,
        source_setup,
        facts_complete,
        import_identity_elapsed,
        sources_bytes,
    })
}

/// The package inputs of an ordinary check, one-shot or retained: the package
/// boundary and the requirement-only index host inference runs over.
///
/// Every import the boundary refuses
/// keeps an open requirement (`SC9005`).
pub fn package_inputs(
    directory: &Path,
    facts: &solid_facts::ProjectFacts,
    source_refusals: &[SourceRefusal],
    runtime: &RuntimeEnvironment,
    dialect: &'static Dialect,
) -> Result<
    (
        solid_reactive_ir::package_boundary::PackageBoundary,
        crate::InferredHost,
    ),
    Box<dyn Error>,
> {
    let boundary = crate::external_package_boundary(facts, source_refusals);
    let inferred =
        crate::inferred_project_host_with_note(directory, runtime, dialect.vocabulary, facts)?;
    Ok((boundary, inferred))
}

fn project_directory(project_id: &str) -> PathBuf {
    let project = Path::new(project_id);
    if project.is_dir() {
        project.to_path_buf()
    } else {
        project
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
}

/// The effective dependency-source selection (ADR 0297), decided once for a
/// check. `default_applies` is the caller's verdict that this is an ordinary
/// analysis whose host declares a target and a build and that asked for no
/// selection of its own; the default then still holds only where a Solid Vite
/// plugin resolves from the project, and says why otherwise. Returns the
/// selection and whether it is the implicit default.
pub fn effective_dependency_selection(
    project: &Path,
    explicit: &[String],
    default_applies: bool,
    window_realm: bool,
    note: &mut dyn FnMut(&str),
) -> Result<(Vec<String>, bool), String> {
    // Review round 1: the default holds only where the project establishes
    // the resolver source selection reads and can take the sibling tsconfig;
    // otherwise no package is analyzed from source, and it says why. An explicit selection
    // states the premise itself and is never second-guessed.
    let implicit = default_applies
        && match implicit_dependency_source_refusal(project) {
            None => true,
            Some(reason) => {
                note(&format!(
                    "solid-checker: note: packages are not analyzed from source by default: \
                     {reason}; their behaviour stays uncertifiable \
                     (--dependency-source '*' selects source anyway)"
                ));
                false
            }
        };
    let selection = if implicit {
        vec!["*".into()]
    } else {
        explicit.to_vec()
    };
    if window_realm && selection.is_empty() {
        return Err(
            "--runtime-realm window applies only to dependency-source packages, \
             and this run analyzes none"
                .into(),
        );
    }
    Ok((selection, implicit))
}

/// Review round 1 of ADR 0297: why the default selection does not apply to
/// `project`, or `None` when it does.
///
/// Source selection reads `exports` as Vite with the Solid plugin does
/// (`solid`, `import` and `module` always set), and a target and build do
/// not say that is how the project is built: a Node host never sets
/// `solid`. The default therefore needs that plugin resolvable from the
/// project (`dependency_sources::solid_vite_plugin`). The generated program
/// is written to a scratch directory, never the project, so a read-only
/// project needs nothing more.
#[must_use]
pub fn implicit_dependency_source_refusal(project: &Path) -> Option<String> {
    if crate::dependency_sources::solid_vite_plugin(project).is_none() {
        return Some(
            "no Solid Vite plugin (@solidjs/vite-plugin, vite-plugin-solid, or one \
             @solidjs/start ships) resolves from the project, so nothing establishes the \
             resolver source selection reads `exports` with"
                .into(),
        );
    }
    None
}

/// An installed Solid runtime this build has no dialect for (ADR 0110): the
/// check is refused with `SC9013` rather than analyzed under the default.
#[derive(Clone, Debug)]
pub struct UnsupportedRuntime {
    pub installed: String,
    pub manifest: PathBuf,
    pub refusal: Option<&'static solid_dialect::RefusedRelease>,
}

/// The dialect a project is analyzed under: the explicit `--dialect`, or the
/// one its nearest installed `solid-js` selects. An unsupported installation
/// still answers the default dialect, with the refusal beside it for the
/// caller to report instead of analyzing.
pub fn select_dialect(
    project: &Path,
    explicit: Option<&str>,
) -> Result<(&'static Dialect, Option<UnsupportedRuntime>), String> {
    if let Some(id) = explicit {
        let dialect = crate::dialect::by_id(id).ok_or_else(|| {
            format!(
                "unknown dialect {id:?}; known dialects: {}",
                crate::dialect::ALL
                    .iter()
                    .map(|dialect| dialect.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
        return Ok((dialect, None));
    }
    Ok(match crate::dialect::detect_detailed(project) {
        crate::dialect::Detection::Installed { dialect, .. } => (dialect, None),
        crate::dialect::Detection::Unsupported {
            installed,
            manifest,
            refusal,
            ..
        } => (
            crate::dialect::default_dialect(),
            Some(UnsupportedRuntime {
                installed,
                manifest,
                refusal,
            }),
        ),
        crate::dialect::Detection::Defaulted { .. } => (crate::dialect::default_dialect(), None),
    })
}

/// Producer programs as child processes, the native factory. A generated
/// program is written to scratch beside nothing in the project and opened at
/// its logical path; the scratch file lives as long as this factory.
pub struct ProcessSessions {
    executable: String,
    arguments: Vec<String>,
    generated: Vec<crate::dependency_sources::GeneratedProgram>,
}

impl ProcessSessions {
    #[must_use]
    pub fn new(executable: impl Into<String>, arguments: Vec<String>) -> Self {
        Self {
            executable: executable.into(),
            arguments,
            generated: Vec::new(),
        }
    }
}

impl ProducerSessions for ProcessSessions {
    fn open(&mut self, project_id: &str) -> Result<TypeFactsSession, Box<dyn Error>> {
        Ok(TypeFactsSession::open(
            &self.executable,
            project_id,
            &self.arguments,
        )?)
    }

    fn open_generated(
        &mut self,
        program: &PreparedDependencyProgram,
    ) -> Result<Result<(String, TypeFactsSession), String>, Box<dyn Error>> {
        let generated = match program.write_for_process() {
            Ok(generated) => generated,
            Err(error) => return Ok(Err(error)),
        };
        let project_id = generated.path().to_string_lossy().into_owned();
        let arguments = self
            .arguments
            .iter()
            .cloned()
            .chain(std::iter::once(format!(
                "-project-config={}",
                generated.config().display()
            )))
            .collect::<Vec<_>>();
        let session = TypeFactsSession::open(&self.executable, &project_id, &arguments)?;
        self.generated.push(generated);
        Ok(Ok((project_id, session)))
    }
}
