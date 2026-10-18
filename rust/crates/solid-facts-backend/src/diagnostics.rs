use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use solid_facts::ProjectFacts;
use solid_reactive_ir::{
    AnalysisInputs, CacheRetention, Finding, IncrementalBuilder, Program, RuleOptions,
    RuntimeEnvironment, suppress_findings_owned_by_enabled_rules,
};

use crate::dialect::{self, Dialect};
use crate::{BackendError, SemanticDemandOptions, SourceFile};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Conditional runtime assumptions used by this analysis (ADRs 0266,
    /// 0268, 0300). These are premises, not facts established by the census.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub premises: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback_facts: Vec<solid_reactive_ir::DevelopmentFile>,
    pub status: String,
    pub findings: Vec<SnapshotFinding>,
    pub metrics: Metrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFinding {
    pub id: String,
    pub rule: String,
    pub kind: String,
    pub severity: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_context: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject_kind: String,
    /// ADR 0205: the analysis-coverage family and subject this gap groups by.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coverage_family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coverage_subject: String,
    pub primary_location: SourceLocation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_locations: Vec<SourceLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<SnapshotEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<SnapshotFix>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEvidence {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFix {
    pub message: String,
    pub applicability: String,
    pub edits: Vec<SnapshotTextEdit>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotTextEdit {
    pub location: SourceLocation,
    pub new_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub path: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub files_analyzed: usize,
    pub functions_analyzed: usize,
    pub proof_obligations: usize,
    pub cached_summaries: usize,
    pub unresolved_obligations: usize,
}

pub struct DiagnosticAnalysis {
    pub program: Arc<Program>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DiagnosticTimings {
    pub reactive_ir: Duration,
    pub solve_and_snapshot: Duration,
    pub reused: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RequestedRuleEnablement<'a> {
    pub presets: &'a [String],
    pub rules: &'a [String],
    pub runtime: RuntimeEnvironment,
    pub feedback_facts: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticIdentity {
    runtime_configuration: solid_reactive_ir::RuntimeConfigurationPremise,
    /// Which dialect's catalog and compiler produced the retained analysis;
    /// a retained result never answers for a different dialect.
    dialect: &'static str,
    project_id: String,
    generation: u64,
    contracts: Vec<[u8; 32]>,
    /// Per-rule options are re-read from disk on every analysis, so an
    /// edited `.solid-checker/rule-options.json` invalidates a retained
    /// diagnostic even within one generation.
    rule_options: RuleOptions,
    /// The installed-release notice is re-read from disk on every analysis
    /// too: an install that moves `solid-js` between an audited and an
    /// unaudited release changes the result without changing a source file.
    release_notice: Option<dialect::ReleaseNotice>,
    /// ADR 0324: whether the router-parameters premise applies; it moves with
    /// the resolved router release as well as with the sources.
    router_params: solid_reactive_ir::router_params::RouterParamsStatus,
}

struct RetainedDiagnostic {
    identity: DiagnosticIdentity,
    analysis: Arc<DiagnosticAnalysis>,
}

/// Retains the complete diagnostic result for one coherent project
/// generation. The session owns IR cache policy, solving, and snapshot
/// construction so callers cannot accidentally select a slower fresh path.
pub struct DiagnosticSession {
    dialect: &'static Dialect,
    builder: IncrementalBuilder,
    retained: Option<RetainedDiagnostic>,
}

impl Default for DiagnosticSession {
    fn default() -> Self {
        Self::new(dialect::default_dialect())
    }
}

/// Preserve every baseline violation; a stronger host can replace its proof,
/// but a clean browser view cannot erase a defect of another execution.
fn merge_host_findings(
    baseline: &[SnapshotFinding],
    browser: &[SnapshotFinding],
    eligible: &impl Fn(&SnapshotFinding) -> bool,
    reason: &str,
) -> Vec<SnapshotFinding> {
    let selected = browser
        .iter()
        .filter(|finding| eligible(finding))
        .collect::<Vec<_>>();
    let same_claim = |left: &SnapshotFinding, right: &SnapshotFinding| {
        left.id == right.id
            && left.rule == right.rule
            && left.kind == right.kind
            && left.primary_location.path == right.primary_location.path
            && left.primary_location.start_byte == right.primary_location.start_byte
            && left.primary_location.end_byte == right.primary_location.end_byte
            && left.analysis_context == right.analysis_context
            && left.subject_kind == right.subject_kind
    };
    let mut result = baseline
        .iter()
        .filter(|finding| {
            !eligible(finding)
                || (finding.kind == "violation"
                    && !selected
                        .iter()
                        .any(|selected| same_claim(finding, selected)))
        })
        .cloned()
        .collect::<Vec<_>>();
    result.extend(selected.into_iter().cloned().map(|mut finding| {
        finding.evidence.push(SnapshotEvidence {
            message: reason.into(),
            location: None,
        });
        finding
    }));
    result
}

impl DiagnosticSession {
    #[must_use]
    pub fn new(dialect: &'static Dialect) -> Self {
        Self {
            dialect,
            builder: IncrementalBuilder::default(),
            retained: None,
        }
    }

    pub fn analyze(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
    ) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
        self.analyze_inputs_measured_with_enablement(
            project,
            sources,
            facts,
            AnalysisInputs::empty(),
            RequestedRuleEnablement::default(),
        )
        .map(|(analysis, _)| analysis)
    }

    /// Runs ordinary analysis from receipt-validated normalized semantics
    /// against the project's package boundary, which is part of the retained
    /// diagnostic's identity.
    pub fn analyze_inputs_measured_with_enablement(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        inputs: AnalysisInputs<'_>,
        enablement: RequestedRuleEnablement<'_>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        self.analyze_accepted_inner(project, sources, facts, inputs, enablement)
    }

    #[allow(clippy::too_many_arguments)]
    fn analyze_accepted_inner(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        inputs: AnalysisInputs<'_>,
        enablement: RequestedRuleEnablement<'_>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        if let Some(hosts) = inputs.hosts {
            crate::inferred_host::validate_inputs(hosts)?;
            // No package behaviour differs by host, so the browser execution
            // is answered by the ordinary analysis itself; the host only marks
            // the findings it covers and keeps the result from certifying.
            let (ordinary, mut timings) = self.analyze_accepted_inner(
                project,
                sources,
                facts,
                AnalysisInputs {
                    boundary: inputs.boundary,
                    hosts: None,
                },
                enablement,
            )?;
            let browser_site = |location: &SourceLocation| {
                hosts.browser_proof_site(&location.path, location.start_byte, location.end_byte)
            };
            let browser_proof = |finding: &SnapshotFinding| {
                browser_site(&finding.primary_location)
                    && finding
                        .related_locations
                        .iter()
                        .chain(
                            finding
                                .evidence
                                .iter()
                                .filter_map(|step| step.location.as_ref()),
                        )
                        .filter(|location| {
                            !Path::new(&location.path)
                                .components()
                                .any(|part| part.as_os_str() == "node_modules")
                                && (facts
                                    .files
                                    .iter()
                                    .any(|file| file.path.as_str() == location.path)
                                    || project.parent().is_some_and(|directory| {
                                        Path::new(&location.path).starts_with(directory)
                                    }))
                        })
                        .all(browser_site)
            };
            let projection = crate::inferred_host::HostStage::new("projection");
            let mut snapshot = ordinary.snapshot.clone();
            snapshot.findings = merge_host_findings(
                &ordinary.snapshot.findings,
                &ordinary.snapshot.findings,
                &browser_proof,
                &hosts.manifest.reason,
            );
            // Inference changes findings only. Even a clean browser execution
            // cannot certify the server execution or an explicit target premise.
            snapshot.status = if snapshot
                .findings
                .iter()
                .any(|finding| finding.kind == "violation")
            {
                "violation"
            } else {
                "uncertifiable"
            }
            .into();
            drop(projection);
            crate::inferred_host::validate_inputs(hosts)?;
            timings.reused = false;
            return Ok((
                Arc::new(DiagnosticAnalysis {
                    program: Arc::clone(&ordinary.program),
                    snapshot,
                }),
                timings,
            ));
        }
        let ir_started = Instant::now();
        let mut rule_options = discover_rule_options(project)?;
        rule_options.request_presets(enablement.presets.iter().cloned());
        rule_options.request_rules(enablement.rules.iter().cloned());
        enablement
            .runtime
            .validate()
            .map_err(BackendError::Contract)?;
        rule_options.runtime = enablement.runtime;
        if rule_options.runtime.program_boundary.is_none() {
            rule_options.runtime.program_boundary = Some(default_program_boundary(project));
        }
        rule_options.runtime.router_install =
            crate::installed_bytes::router_install_evidence(project, facts);
        rule_options.development_feedback = enablement.feedback_facts;
        // Scoped to this generation's facts: a gap about named exports is due
        // only where the project reaches one (`release_scope`), so the answer
        // moves with the sources as well as the install.
        let release_notice = dialect::release_notice(self.dialect, project)
            .and_then(|notice| notice.scoped_to(facts));
        let router_params = solid_reactive_ir::router_params::router_params_status(
            facts,
            self.dialect.vocabulary,
            &rule_options.runtime.router_install,
        );
        // Consult the builder before a diagnostic hit: its identity includes
        // ADR 0266's visible veto even when fact generation is unchanged.
        let (program, _) = self.builder.build_with_analysis_inputs_shared(
            facts,
            self.dialect.vocabulary,
            inputs,
            &rule_options,
        )?;
        let identity = DiagnosticIdentity {
            runtime_configuration: program.runtime_configuration.clone(),
            dialect: self.dialect.id,
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: inputs.cache_fingerprints(),
            rule_options: rule_options.clone(),
            release_notice: release_notice.clone(),
            router_params: router_params.clone(),
        };
        if let Some(retained) = &self.retained
            && retained.identity == identity
        {
            return Ok((
                Arc::clone(&retained.analysis),
                DiagnosticTimings {
                    reactive_ir: ir_started.elapsed(),
                    reused: true,
                    ..DiagnosticTimings::default()
                },
            ));
        }
        let reactive_ir = ir_started.elapsed();
        let solve_started = Instant::now();
        let mut findings = self.dialect.solve(&program);
        retain_enabled(self.dialect, &rule_options, &mut findings)?;
        // ADR 0284: a package analyzed as source is not the project's code.
        // Its own findings are not reported; what it does to project values
        // is reported at the project's call sites, through summaries and,
        // for owner requirements, through ADR 0290's attribution.
        if !facts.dependency_source_roots.is_empty() {
            findings.retain(|finding| {
                facts
                    .dependency_source_root(&finding.primary_location.path)
                    .is_none()
            });
        }
        suppress_findings_owned_by_enabled_rules(&mut findings, self.dialect.catalog_capabilities);
        // After enablement, like the refusal: the notice is about the installed
        // runtime, not a site, so there is nothing to suppress it at.
        if let Some(notice) = &release_notice {
            findings.push(unaudited_release_finding(notice));
        }
        // ADR 0324: a router release nobody audited gets its own notice, at
        // the import that reaches it.
        if let solid_reactive_ir::router_params::RouterParamsStatus::Unaudited {
            version,
            bytes_differ,
            path,
            span,
        } = &router_params
        {
            findings.push(unaudited_router_finding(
                self.dialect,
                version.as_deref(),
                *bytes_differ,
                path,
                *span,
            ));
        }
        let metrics = analysis_metrics(facts, &program);
        let mut snapshot = snapshot_of(sources, metrics, findings);
        snapshot.feedback_facts = program.development_feedback.clone();
        if program.runtime_configuration.permits_proof() {
            snapshot
                .premises
                .push("standard runtime configuration (ADRs 0266/0268)".into());
        }
        if let solid_reactive_ir::router_params::RouterParamsStatus::Applies { version } =
            &router_params
        {
            snapshot.premises.push(format!(
                "router parameters (ADR 0324): @solidjs/router {version}, as audited, hands out route parameters as URL path-segment strings, or undefined for an absent optional one"
            ));
        }
        if !facts.dependency_source_roots.is_empty() {
            snapshot.premises.push("package integrity (ADR 0300): code outside the analyzed program neither provides selected packages' exported contexts nor rewrites objects those packages create".into());
        }
        let analysis = Arc::new(DiagnosticAnalysis { program, snapshot });
        self.retained = Some(RetainedDiagnostic {
            identity,
            analysis: Arc::clone(&analysis),
        });
        Ok((
            analysis,
            DiagnosticTimings {
                reactive_ir,
                solve_and_snapshot: solve_started.elapsed(),
                reused: false,
            },
        ))
    }

    pub fn clear(&mut self) {
        self.builder.clear();
        self.retained = None;
    }

    /// Releases derived IR indexes according to the daemon's idle policy while
    /// preserving the current diagnostic and coherent program.
    pub fn retain_for_idle(&mut self, retention: CacheRetention) {
        self.builder.retain_for_idle(retention);
    }
}

/// Runs ordinary analysis of one project against its package boundary.
pub fn analyze_project_inputs_measured_with_enablement(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    inputs: AnalysisInputs<'_>,
    enablement: RequestedRuleEnablement<'_>,
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    DiagnosticSession::new(dialect)
        .analyze_inputs_measured_with_enablement(project, sources, facts, inputs, enablement)
}

fn retain_enabled(
    dialect: &Dialect,
    options: &RuleOptions,
    findings: &mut Vec<Finding>,
) -> Result<(), BackendError> {
    let mut unknown = Vec::new();
    findings.retain(|finding| match (dialect.rule_metadata)(&finding.rule) {
        Some(metadata) => {
            options.is_enabled(&finding.rule, metadata.default_enabled, metadata.presets)
        }
        None => {
            unknown.push(finding.rule.clone());
            false
        }
    });
    if unknown.is_empty() {
        return Ok(());
    }
    unknown.sort_unstable();
    unknown.dedup();
    Err(BackendError::UnknownRuleIdentity {
        dialect: dialect.id,
        rules: unknown,
    })
}

/// The `SC9014` notice: the analysis beside it ran, under a vocabulary that was
/// not audited on the installed release, so the result cannot certify.
///
/// Uncertifiable rather than a violation for SC9013's reason -- the claim is
/// about the installed runtime, never the project's source -- and located at
/// the deciding manifest, as a project-scoped finding, for the same reason.
/// Unlike SC9013 it *accompanies* the analysis: the vocabulary is the one the
/// review measured to hold for everything except the gaps it names.
#[must_use]
pub fn unaudited_release_finding(notice: &dialect::ReleaseNotice) -> Finding {
    let manifest: Arc<str> = notice.manifest.display().to_string().into();
    let location = typefacts::Location {
        path: Arc::clone(&manifest),
        start_byte: 0,
        end_byte: 0,
    };
    let found = english_list(
        &notice
            .releases
            .iter()
            .map(|release| match &release.version {
                Some(version) => format!("{} {version}", release.package),
                None => format!("no {}", release.package),
            })
            .collect::<Vec<_>>(),
    );
    let audited = notice
        .audited
        .iter()
        .map(|(package, _)| *package)
        .collect::<Vec<_>>();
    let audited_versions = notice
        .audited
        .iter()
        .map(|(_, version)| *version)
        .collect::<std::collections::BTreeSet<_>>();
    let audited_release = match audited_versions.iter().collect::<Vec<_>>().as_slice() {
        [single] => format!("{single}, the audited release of each"),
        _ => english_list(
            &notice
                .audited
                .iter()
                .map(|(package, version)| format!("{package} {version}"))
                .collect::<Vec<_>>(),
        ),
    };
    let mut reviews = notice
        .gaps
        .iter()
        .filter_map(|gap| gap.review)
        .collect::<Vec<_>>();
    reviews.sort_unstable();
    reviews.dedup();
    let message = format!(
        "{found} {} installed; this build's Solid 2 vocabulary was audited on {}, and {} in what it knows about this installation {} still open, so the analysis ran and cannot certify the project",
        if notice.releases.len() == 1 {
            "is"
        } else {
            "are"
        },
        if audited.is_empty() {
            "no installation".to_owned()
        } else {
            english_list(
                &notice
                    .audited
                    .iter()
                    .map(|(package, version)| format!("{package} {version}"))
                    .collect::<Vec<_>>(),
            )
        },
        match notice.gaps.len() {
            1 => "1 gap".to_owned(),
            count => format!("{count} gaps"),
        },
        if notice.gaps.len() == 1 { "is" } else { "are" },
    );
    let pin = if audited.is_empty() {
        "Use a checker release that has reviewed this installation to certify.".to_owned()
    } else {
        format!(
            "To certify, pin {} to {audited_release}, a transitive one with an overrides or resolutions entry, because a dependency's own range can admit later releases; this project resolves {found}.",
            english_list(
                &audited
                    .iter()
                    .map(|package| (*package).to_owned())
                    .collect::<Vec<_>>()
            )
        )
    };
    let hint = if reviews.is_empty() {
        format!("Findings beside this notice stand; certification waits on the gaps. {pin}")
    } else {
        format!(
            "Findings beside this notice stand; certification waits on the gaps. {pin} The {} {}.",
            if reviews.len() == 1 {
                "review is"
            } else {
                "reviews are"
            },
            english_list(
                &reviews
                    .iter()
                    .map(|review| (*review).to_owned())
                    .collect::<Vec<_>>()
            )
        )
    };
    let mut evidence = notice
        .releases
        .iter()
        .map(|release| solid_reactive_ir::EvidenceStep {
            message: match &release.version {
                Some(version) => format!(
                    "{} resolves here, and names version {version}",
                    release.package
                ),
                None => format!("{} does not resolve", release.package),
            },
            location: release
                .manifest
                .as_ref()
                .map(|manifest| typefacts::Location {
                    path: manifest.display().to_string().into(),
                    start_byte: 0,
                    end_byte: 0,
                }),
        })
        .collect::<Vec<_>>();
    evidence.extend(
        notice
            .gaps
            .iter()
            .map(|gap| solid_reactive_ir::EvidenceStep {
                message: format!("known gap {}", gap.gap),
                location: None,
            }),
    );
    evidence.extend(
        notice
            .reaches
            .iter()
            .map(|reach| solid_reactive_ir::EvidenceStep {
                message: reach.message.clone(),
                location: Some(typefacts::Location {
                    path: reach.path.as_str().into(),
                    start_byte: u64::from(reach.span.start),
                    end_byte: u64::from(reach.span.end),
                }),
            }),
    );
    let metadata = solid_reactive_ir::RuleMetadata {
        code: dialect::UNAUDITED_RELEASE_CODE,
        name: dialect::UNAUDITED_RELEASE_RULE,
        severity: "warning",
        uncertifiable: true,
        default_enabled: true,
        presets: &[],
    };
    let mut finding = Finding::new(metadata, message, location);
    finding.hint = hint;
    finding.analysis_context = "dialect-detection".into();
    finding.subject_kind = "project".into();
    finding.evidence = evidence;
    finding
}

/// The `SC9014` notice for a router release the dialect did not audit
/// (ADR 0324). The premise it gates is not applied, so the findings beside it
/// that it could have discharged stand; the notice names the release, where
/// the project reaches it, and the audited one.
#[must_use]
pub fn unaudited_router_finding(
    dialect: &Dialect,
    version: Option<&str>,
    bytes_differ: bool,
    path: &str,
    span: solid_facts::core::Span,
) -> Finding {
    let location = typefacts::Location {
        path: path.into(),
        start_byte: u64::from(span.start),
        end_byte: u64::from(span.end),
    };
    let audited = dialect.vocabulary.audited_router_releases();
    let found = version.map_or_else(
        || "an @solidjs/router whose release was not resolved".to_owned(),
        |version| format!("@solidjs/router {version}"),
    );
    let installed = if bytes_differ {
        format!("{found} is installed, but its installed bytes differ from the audited release")
    } else {
        format!("{found} is installed")
    };
    let message = format!(
        "{installed}; this build audited route parameters on {}, so the router-parameters premise (ADR 0324) was not applied and what it could have settled stays open",
        english_list(
            &audited
                .iter()
                .map(|version| format!("@solidjs/router {version}"))
                .collect::<Vec<_>>()
        ),
    );
    let metadata = solid_reactive_ir::RuleMetadata {
        code: dialect::UNAUDITED_RELEASE_CODE,
        name: dialect::UNAUDITED_RELEASE_RULE,
        severity: "warning",
        uncertifiable: true,
        default_enabled: true,
        presets: &[],
    };
    let mut finding = Finding::new(metadata, message, location.clone());
    finding.hint = format!(
        "Findings beside this notice stand. To have route parameters settled, install {}.",
        english_list(
            &audited
                .iter()
                .map(|version| format!("@solidjs/router {version}"))
                .collect::<Vec<_>>()
        ),
    );
    finding.analysis_context = "router-release".into();
    finding.subject_kind = "project".into();
    finding.evidence = vec![solid_reactive_ir::EvidenceStep {
        message: "this import reaches useParams from the router".into(),
        location: Some(location),
    }];
    finding
}

/// `a`, `a and b`, `a, b and c`.
fn english_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// An audited installation as one release, `2.0.0-rc.3`, when every package
/// shares it, and otherwise as `solid-js 2.0.0-rc.3 and …`. `None` for a
/// vocabulary audited on none.
fn audited_spelling(audited: &[(&str, &str)]) -> Option<String> {
    let versions = audited
        .iter()
        .map(|(_, version)| *version)
        .collect::<std::collections::BTreeSet<_>>();
    match versions.iter().collect::<Vec<_>>().as_slice() {
        [] => None,
        [single] => Some((**single).to_owned()),
        _ => Some(english_list(
            &audited
                .iter()
                .map(|(package, version)| format!("{package} {version}"))
                .collect::<Vec<_>>(),
        )),
    }
}

/// The whole result for a project whose installed Solid runtime this build has
/// no dialect for.
///
/// Not produced by the rules engine, and deliberately not reachable from it:
/// there is no analysis behind it, so there is nothing for a rule to run on.
/// Dialect detection builds this directly and hands it to the emission path.
///
/// **One finding, and never any others.** The refusal's entire claim is that
/// the checker cannot model this project; a second finding beside it would be
/// an assertion about source that was never analyzed under the language it
/// actually runs. `metrics` is all zeroes for the same reason -- nothing was
/// read, and reporting otherwise would overstate what happened.
///
/// `refusal` is `Some` when the major is carried and its vocabulary refuses the
/// release line (the pre-beta `2.0.0-experimental.x`): the message then states
/// the vocabulary's reason instead of "carries no dialect for it", which would
/// be false.
#[must_use]
pub fn unsupported_runtime_snapshot(
    installed: &str,
    manifest: &Path,
    refusal: Option<&solid_dialect::RefusedRelease>,
) -> Snapshot {
    let manifest = manifest.display().to_string();
    let (message, hint) = match refusal {
        None => (
            format!(
                "solid-js {installed} is installed, and this build of solid-checker carries no dialect for it; the project was not analyzed"
            ),
            "Upgrade the project to Solid 2.0, or use a checker release carrying the dialect for this runtime. Passing --dialect analyzes the project anyway, under a language it does not run."
                .to_owned(),
        ),
        Some(refusal) => (
            format!(
                "solid-js {installed} is installed, and this build of solid-checker refuses the {} line: {}; the project was not analyzed",
                refusal.line, refusal.reason
            ),
            format!(
                "Upgrade the project to a Solid 2.0 release candidate{}. Passing --dialect analyzes the project anyway, under a runtime it does not run. The measurement is in {}.",
                audited_spelling(refusal.audited)
                    .map(|audited| format!(" (the audited one is {audited})"))
                    .unwrap_or_default(),
                refusal.review
            ),
        ),
    };
    Snapshot {
        premises: Vec::new(),
        feedback_facts: Vec::new(),
        status: "uncertifiable".into(),
        findings: vec![SnapshotFinding {
            id: dialect::UNSUPPORTED_RUNTIME_CODE.into(),
            rule: dialect::UNSUPPORTED_RUNTIME_RULE.into(),
            kind: "uncertifiable".into(),
            severity: "error".into(),
            message,
            hint,
            analysis_context: "dialect-detection".into(),
            subject_kind: "project".into(),
            coverage_family: String::new(),
            coverage_subject: String::new(),
            primary_location: SourceLocation {
                path: manifest.clone(),
                start_byte: 0,
                end_byte: 0,
                line: 1,
                column: 1,
            },
            related_locations: Vec::new(),
            evidence: vec![SnapshotEvidence {
                message: format!(
                    "the nearest node_modules/solid-js above the project resolves here, and names version {installed}"
                ),
                location: Some(SourceLocation {
                    path: manifest,
                    start_byte: 0,
                    end_byte: 0,
                    line: 1,
                    column: 1,
                }),
            }],
            fixes: Vec::new(),
        }],
        metrics: Metrics {
            files_analyzed: 0,
            functions_analyzed: 0,
            proof_obligations: 0,
            cached_summaries: 0,
            unresolved_obligations: 0,
        },
    }
}

fn snapshot_of(sources: &[SourceFile], metrics: Metrics, findings: Vec<Finding>) -> Snapshot {
    let has_violation = findings.iter().any(|finding| finding.kind == "violation");
    let has_unresolved = findings
        .iter()
        .any(|finding| finding.kind == "uncertifiable");
    let status = if has_violation {
        "violation"
    } else if has_unresolved {
        "uncertifiable"
    } else {
        "certified"
    };
    let findings = findings
        .into_iter()
        .map(|finding| SnapshotFinding {
            kind: finding.kind,
            id: finding.id,
            rule: finding.rule,
            severity: finding.severity,
            message: finding.message,
            hint: finding.hint,
            analysis_context: finding.analysis_context,
            subject_kind: finding.subject_kind,
            coverage_family: finding.coverage_family,
            coverage_subject: finding.coverage_subject,
            primary_location: source_location(&finding.primary_location, sources),
            related_locations: finding
                .related_locations
                .iter()
                .map(|location| source_location(location, sources))
                .collect(),
            evidence: finding
                .evidence
                .into_iter()
                .map(|step| SnapshotEvidence {
                    message: step.message,
                    location: step
                        .location
                        .as_ref()
                        .map(|location| source_location(location, sources)),
                })
                .collect(),
            fixes: finding
                .fixes
                .into_iter()
                .map(|fix| SnapshotFix {
                    message: fix.message,
                    applicability: fix.applicability,
                    edits: fix
                        .edits
                        .into_iter()
                        .map(|edit| SnapshotTextEdit {
                            location: source_location(&edit.location, sources),
                            new_text: edit.new_text,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    Snapshot {
        premises: Vec::new(),
        feedback_facts: Vec::new(),
        status: status.into(),
        findings,
        metrics,
    }
}

pub fn analysis_metrics(facts: &ProjectFacts, program: &Program) -> Metrics {
    let functions_analyzed = facts
        .typescript
        .files()
        .map(|file| file.functions.len())
        .sum::<usize>()
        + program.obligation_counts.factory_instances;
    let unresolved_obligations = program
        .static_violations
        .iter()
        .filter(|violation| violation.id.starts_with("SC9"))
        .count()
        + program
            .static_defects
            .iter()
            .filter(|defect| defect.kind.is_unresolved_obligation())
            .count();
    Metrics {
        files_analyzed: facts
            .files
            .iter()
            .filter(|file| {
                matches!(
                    Path::new(file.path.as_str())
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("jsx" | "tsx")
                )
            })
            .count(),
        functions_analyzed,
        proof_obligations: program.obligation_counts.strict_reads
            + program.obligation_counts.writes_and_actions
            + program.leaf_operations.len()
            + program.missing_owners.len()
            + program.async_reads.len()
            + program.directive_creations.len()
            + program.static_violations.len()
            + program.static_defects.len(),
        cached_summaries: 0,
        unresolved_obligations,
    }
}

pub fn source_location(location: &typefacts::Location, sources: &[SourceFile]) -> SourceLocation {
    let (line, column) = sources
        .iter()
        .find(|source| *source.path == *location.path)
        .map_or((1, 1), |source| {
            let mut offset = usize::try_from(location.start_byte)
                .unwrap_or(usize::MAX)
                .min(source.source.len());
            while !source.source.is_char_boundary(offset) {
                offset = offset.saturating_sub(1);
            }
            let prefix = &source.source[..offset];
            let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
            (
                prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
                source.source[line_start..offset].encode_utf16().count() + 1,
            )
        });
    SourceLocation {
        path: location.path.to_string(),
        start_byte: location.start_byte,
        end_byte: location.end_byte,
        line,
        column,
    }
}

fn package_root(module: &str) -> &str {
    if module.starts_with('@') {
        module
            .match_indices('/')
            .nth(1)
            .map_or(module, |(index, _)| &module[..index])
    } else {
        module.split('/').next().unwrap_or(module)
    }
}

/// The sorted package roots of non-relative, non-builtin imports across the
/// project's facts — the module set contract discovery probes.
pub fn imported_package_roots(facts: &ProjectFacts) -> Vec<String> {
    let mut modules = facts
        .files
        .iter()
        .flat_map(|file| &file.ast.imports)
        .filter(|import| {
            !import.module.starts_with('.')
                && !import.module.starts_with('/')
                && !import.module.starts_with("node:")
        })
        .map(|import| package_root(&import.module).to_string())
        .collect::<Vec<_>>();
    modules.sort();
    modules.dedup();
    modules
}

/// The installed package manifests that influence first-party bundle selection
/// and accepted-contract coverage for the given imported modules. The retained
/// daemon hashes the accepted catalog and its members separately.
pub fn discovered_contract_paths(
    project_directory: &Path,
    modules: &[String],
) -> Result<Vec<PathBuf>, BackendError> {
    let mut paths = Vec::new();
    for module in modules {
        if let Some(directory) = discover_package_directory(project_directory, module)? {
            let manifest = directory.join("package.json");
            if manifest.is_file() {
                paths.push(manifest);
            }
        }
    }
    Ok(paths)
}

fn discover_package_directory(
    directory: &Path,
    module: &str,
) -> Result<Option<PathBuf>, BackendError> {
    Ok(discover_package_install(directory, module)?.map(|(_, package)| package))
}

/// [`discover_package_directory`] with the directory whose `node_modules`
/// holds the package: `(install directory, package directory)`. The install
/// directory is where the package manager put this copy, so the lockfile that
/// governs it is found by walking up from there.
fn discover_package_install(
    directory: &Path,
    module: &str,
) -> Result<Option<(PathBuf, PathBuf)>, BackendError> {
    for ancestor in directory.ancestors() {
        let candidate = ancestor.join("node_modules").join(module);
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() => {
                return Ok(Some((ancestor.to_path_buf(), candidate)));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}

/// The program boundary a check uses when the user selected none (ADR 0193).
///
/// An application is the whole program: every caller of its exported
/// components and hooks is one of its own call sites. A published library is
/// not. The nearest `package.json` at or above the project decides:
///
/// - `"private": true` is an application;
/// - otherwise, a manifest that publishes an entry (`exports`, `main`,
///   `module`, `types`, `typings` or `bin`) is a library;
/// - a manifest that publishes nothing is an application.
///
/// No manifest, or one that cannot be read, keeps the open boundary: a build
/// that cannot tell stays fail-closed.
pub(crate) fn default_program_boundary(project: &Path) -> solid_reactive_ir::ProgramBoundary {
    use solid_reactive_ir::ProgramBoundary;
    let start = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or(project)
    };
    let Some(manifest) = start
        .ancestors()
        .map(|directory| directory.join("package.json"))
        .find(|candidate| candidate.is_file())
    else {
        return ProgramBoundary::Open;
    };
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return ProgramBoundary::Open;
    };
    let Ok(serde_json::Value::Object(fields)) = serde_json::from_str::<serde_json::Value>(&text)
    else {
        return ProgramBoundary::Open;
    };
    if fields.get("private") == Some(&serde_json::Value::Bool(true)) {
        return ProgramBoundary::Closed;
    }
    let publishes = ["exports", "main", "module", "types", "typings", "bin"]
        .iter()
        .any(|field| fields.contains_key(*field));
    if publishes {
        ProgramBoundary::Open
    } else {
        ProgramBoundary::Closed
    }
}

/// Loads the project's per-rule options from the nearest
/// `.solid-checker/rule-options.json`, walking ancestors exactly as local
/// contract discovery does. A project without one gets upstream's defaults;
/// a file that fails to parse fails the analysis rather than silently
/// meaning "defaults".
pub fn discover_rule_options(project: &Path) -> Result<RuleOptions, BackendError> {
    discover_rule_options_with(
        project,
        // A retired identity is accepted so an existing rule-options document
        // does not hard-fail on a rule this checker itself deleted. The rule is
        // gone either way: no catalog declares it, so disabling it is a no-op.
        |rule| {
            dialect::ALL.iter().any(|dialect| (dialect.has_rule)(rule))
                || dialect::retired_rule(rule).is_some()
        },
    )
}

/// Facts needed before diagnostic rule execution, derived from the same
/// project options and request enablement that the diagnostic identity uses.
pub fn semantic_demand_options_for_enablement(
    dialect: &Dialect,
    project: &Path,
    enablement: RequestedRuleEnablement<'_>,
) -> Result<SemanticDemandOptions, BackendError> {
    let mut options = discover_rule_options(project)?;
    options.request_presets(enablement.presets.iter().cloned());
    options.request_rules(enablement.rules.iter().cloned());
    let rule = "prefer-for";
    let metadata = (dialect.rule_metadata)(rule);
    Ok(SemanticDemandOptions {
        array_map_receiver_types: metadata.is_some_and(|metadata| {
            options.is_enabled(rule, metadata.default_enabled, metadata.presets)
        }),
        contract_probe_parameters: false,
    })
}

fn discover_rule_options_with(
    project: &Path,
    has_rule: impl Fn(&str) -> bool,
) -> Result<RuleOptions, BackendError> {
    let directory = if project.is_dir() {
        project
    } else {
        project.parent().unwrap_or(project)
    };
    for ancestor in directory.ancestors() {
        let candidate = ancestor.join(".solid-checker").join("rule-options.json");
        match fs::read_to_string(&candidate) {
            Ok(encoded) => {
                return RuleOptions::parse_with_aliases(&encoded, &has_rule, dialect::rule_alias)
                    .map_err(|error| {
                        BackendError::RuleOptions(format!("{}: {error}", candidate.display()))
                    });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(RuleOptions::default())
}

/// The rule-options document [`discover_rule_options`] would load for this
/// project, if one exists on disk right now.
///
/// The retained check daemon folds this into its cached-snapshot input set:
/// per-rule options are part of every diagnostic identity, so editing,
/// creating, or deleting `.solid-checker/rule-options.json` must invalidate
/// a cached answer exactly like an edited contract does.
pub fn discovered_rule_options_path(project_directory: &Path) -> Option<PathBuf> {
    for ancestor in project_directory.ancestors() {
        let candidate = ancestor.join(".solid-checker").join("rule-options.json");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {

    #[test]
    fn inferred_projection_is_monotone_for_existing_violations() {
        use super::{SnapshotFinding, merge_host_findings};
        let finding = |kind: &str| {
            serde_json::from_value::<SnapshotFinding>(serde_json::json!({
            "id":"SC4001", "rule":"missing-owner", "kind":kind, "severity":"error", "message":"baseline",
            "primaryLocation":{"path":"app.ts", "startByte":0, "endByte":1, "line":1, "column":1}
        })).unwrap()
        };
        let baseline = [finding("violation"), finding("uncertifiable")];
        let kept = merge_host_findings(&baseline, &[], &|_| true, "inferred browser: root");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].message, "baseline");
        let selected = merge_host_findings(
            &baseline,
            &[finding("violation")],
            &|_| true,
            "inferred browser: root",
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].evidence.len(), 1);
        let refused = merge_host_findings(
            &baseline,
            &[finding("violation")],
            &|_| false,
            "inferred browser: root",
        );
        assert_eq!(refused.len(), 2);
        assert!(refused.iter().all(|finding| finding.evidence.is_empty()));
    }

    use std::{path::Path, sync::Arc};

    use solid_facts::core::Generation;
    use solid_facts::{ProjectFacts, TypeScriptTable};
    use solid_reactive_ir::RuntimeEnvironment;

    use super::{DiagnosticSession, retain_enabled};

    #[test]
    fn unknown_finding_identities_fail_closed() {
        let mut findings = vec![solid_reactive_ir::Finding::new(
            solid_reactive_ir::RuleMetadata {
                code: "TEST00",
                name: "not-in-the-catalog",
                severity: "error",
                uncertifiable: false,
                default_enabled: true,
                presets: &[],
            },
            "synthetic".into(),
            typefacts::Location {
                path: "synthetic.tsx".into(),
                start_byte: 0,
                end_byte: 1,
            },
        )];
        let error = retain_enabled(
            crate::dialect::default_dialect(),
            &solid_reactive_ir::RuleOptions::default(),
            &mut findings,
        )
        .unwrap_err();
        assert!(findings.is_empty());
        assert!(matches!(
            error,
            crate::BackendError::UnknownRuleIdentity { rules, .. }
                if rules == ["not-in-the-catalog"]
        ));
    }

    #[test]
    fn diagnostic_session_reuses_the_complete_result() {
        let facts = ProjectFacts {
            generation: Generation::new(1).unwrap(),
            project_id: "/virtual/tsconfig.json".into(),
            files: Vec::new(),
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "/virtual/tsconfig.json",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: Vec::new(),
        };
        let mut session = DiagnosticSession::default();

        let (initial, initial_timings) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement::default(),
            )
            .unwrap();
        let (reused, reused_timings) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement::default(),
            )
            .unwrap();

        assert!(Arc::ptr_eq(&initial, &reused));
        assert!(!initial_timings.reused);
        assert!(reused_timings.reused);
    }

    #[test]
    fn diagnostic_session_keys_retention_on_requested_enablement() {
        let facts = ProjectFacts {
            generation: Generation::new(1).unwrap(),
            project_id: "/virtual/tsconfig.json".into(),
            files: Vec::new(),
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "/virtual/tsconfig.json",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: Default::default(),
            dependency_source_roots: Vec::new(),
        };
        let mut session = DiagnosticSession::default();

        let (_, baseline) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement::default(),
            )
            .unwrap();
        let (preset_analysis, preset_miss) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement {
                    presets: &["preferences".into()],
                    rules: &[],
                    runtime: RuntimeEnvironment::default(),
                    feedback_facts: false,
                },
            )
            .unwrap();
        let (preset_reused, preset_hit) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement {
                    presets: &["preferences".into(), "preferences".into()],
                    rules: &[],
                    runtime: RuntimeEnvironment::default(),
                    feedback_facts: false,
                },
            )
            .unwrap();
        let (_, rule_miss) = session
            .analyze_inputs_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                solid_reactive_ir::AnalysisInputs::empty(),
                super::RequestedRuleEnablement {
                    presets: &[],
                    rules: &["prefer-show".into()],
                    runtime: RuntimeEnvironment::default(),
                    feedback_facts: false,
                },
            )
            .unwrap();

        assert!(!baseline.reused);
        assert!(
            !preset_miss.reused,
            "a preset change must miss retained analysis"
        );
        assert!(
            preset_hit.reused,
            "duplicate preset values must normalize to one identity"
        );
        assert!(Arc::ptr_eq(&preset_analysis, &preset_reused));
        assert!(
            !rule_miss.reused,
            "an enabled-rule change must miss retained analysis"
        );
    }
    /// A rule-options document naming a *removed* rule must load, and one
    /// naming a rule that never existed must still fail. The first half is the
    /// migration path for a project that had disabled a rule this checker went
    /// on to delete; the second is what keeps a typo from silently changing
    /// policy, which is the reason the validation exists.
    #[test]
    fn compatibility_rule_identities_are_tolerated_and_typos_are_not() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-retired-rules-{}",
            std::process::id()
        ));
        let options_directory = directory.join(".solid-checker");
        std::fs::create_dir_all(&options_directory).unwrap();
        let document = options_directory.join("rule-options.json");

        for retired in crate::dialect::RETIRED_RULES {
            std::fs::write(
                &document,
                format!(
                    r#"{{ "schemaVersion": 1, "rules": {{ {:?}: {{ "enabled": false }} }} }}"#,
                    retired.0
                ),
            )
            .unwrap();
            let loaded = super::discover_rule_options(&directory);
            assert!(
                loaded.is_ok(),
                "a document disabling the retired {:?} must still load: {loaded:?}",
                retired.0
            );
            // And the identity really is gone: nothing in either catalog
            // declares it, so the disable is a no-op rather than a demotion.
            assert!(
                !crate::dialect::ALL
                    .iter()
                    .any(|dialect| (dialect.has_rule)(retired.0)),
                "{:?} is retired but still declared by a catalog",
                retired.0
            );
        }

        for (old, current) in crate::dialect::RULE_ALIASES {
            // An alias names a rule in one dialect's catalog, and a
            // single-dialect build (`--no-default-features --features
            // dialect-v2`, which `scripts/verify.sh` checks and which retiring
            // 1.x makes the only build) compiles only one of them. An alias
            // whose target no compiled-in catalog declares is not loadable
            // here and is not this test's subject; the build that carries the
            // target is where it is asserted.
            if !crate::dialect::ALL
                .iter()
                .any(|dialect| (dialect.has_rule)(current))
            {
                continue;
            }
            std::fs::write(
                &document,
                format!(
                    r#"{{ "schemaVersion": 1, "rules": {{ {old:?}: {{ "enabled": false }} }} }}"#
                ),
            )
            .unwrap();
            let loaded = super::discover_rule_options(&directory)
                .unwrap_or_else(|error| panic!("alias {old:?} must load: {error}"));
            assert!(
                !loaded.is_enabled(current, true, &[]),
                "disabling alias {old:?} did not disable {current:?}"
            );
            assert!(
                crate::dialect::ALL
                    .iter()
                    .any(|dialect| (dialect.has_rule)(current)),
                "alias target {current:?} is absent from every catalog"
            );
            assert!(
                !crate::dialect::ALL
                    .iter()
                    .any(|dialect| (dialect.has_rule)(old)),
                "alias source {old:?} is still declared by a catalog"
            );
        }

        std::fs::write(
            &document,
            r#"{ "schemaVersion": 1, "rules": { "v1/no-such-rule": { "enabled": false } } }"#,
        )
        .unwrap();
        assert!(super::discover_rule_options(&directory).is_err());

        std::fs::remove_dir_all(&directory).ok();
    }

    /// ADR 0193: an application is closed-world, a published library is not,
    /// and a project with no readable manifest keeps the open boundary.
    #[test]
    fn the_default_program_boundary_follows_the_nearest_manifest() {
        use solid_reactive_ir::ProgramBoundary::{Closed, Open};
        let root = std::env::temp_dir().join(format!(
            "solid-checker-program-boundary-{}",
            std::process::id()
        ));
        let boundary = |manifest: Option<&str>| {
            let _ = std::fs::remove_dir_all(&root);
            let project = root.join("app/src");
            std::fs::create_dir_all(&project).unwrap();
            if let Some(manifest) = manifest {
                std::fs::write(root.join("app/package.json"), manifest).unwrap();
            }
            super::default_program_boundary(&project.join("tsconfig.json"))
        };
        assert_eq!(
            boundary(Some(r#"{"private": true, "exports": "./x.js"}"#)),
            Closed
        );
        assert_eq!(
            boundary(Some(r#"{"name": "app"}"#)),
            Closed,
            "publishes nothing"
        );
        assert_eq!(
            boundary(Some(r#"{"name": "lib", "exports": "./x.js"}"#)),
            Open
        );
        assert_eq!(boundary(Some(r#"{"name": "lib", "main": "x.js"}"#)), Open);
        assert_eq!(boundary(Some(r#"{"name": "lib", "bin": "x.js"}"#)), Open);
        assert_eq!(boundary(Some("not json")), Open, "an unreadable manifest");
        assert_eq!(boundary(None), Open, "no manifest above the project");
        let _ = std::fs::remove_dir_all(&root);
    }
}
