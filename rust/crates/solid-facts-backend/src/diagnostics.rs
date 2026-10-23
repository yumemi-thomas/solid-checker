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
    boundary_fingerprints: Vec<[u8; 32]>,
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
            .map_err(BackendError::RuntimePremise)?;
        rule_options.runtime = enablement.runtime;
        if rule_options.runtime.program_boundary.is_none() {
            rule_options.runtime.program_boundary = Some(default_program_boundary(project));
        }
        rule_options.runtime.router_install =
            crate::installed_bytes::router_install_evidence(project, facts);
        rule_options.runtime.package_instances =
            crate::installed_bytes::package_instance_evidence(facts);
        rule_options.development_feedback = enablement.feedback_facts;
        // Scoped to this generation's facts: a gap about named exports is due
        // only where the project reaches one (`release_scope`), so the answer
        // moves with the sources as well as the install.
        // A gap only server rendering can reach is not due under declared CSR.
        let client_only_rendering = matches!(
            rule_options.runtime.rendering,
            Some(solid_reactive_ir::RuntimeRendering::Csr)
        );
        let release_notice = dialect::release_notice(self.dialect, project)
            .and_then(|notice| notice.scoped_to(facts, client_only_rendering));
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
            boundary_fingerprints: inputs.cache_fingerprints(),
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
        let mut withheld_coverage = retain_enabled(self.dialect, &rule_options, &mut findings)?;
        // ADR 0284: a package analyzed as source is not the project's code.
        // Its own findings are not reported; what it does to project values
        // is reported at the project's call sites, through summaries and,
        // for owner requirements, through ADR 0290's attribution.
        if !facts.dependency_source_roots.is_empty() {
            let in_project = |finding: &Finding| {
                facts
                    .dependency_source_root(&finding.primary_location.path)
                    .is_none()
            };
            findings.retain(in_project);
            withheld_coverage.retain(in_project);
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
        // An instance field initializer runs when an instance is constructed,
        // which no rule places; several rules decide "written directly in
        // this callback" on their own and would place it in the function the
        // class is written in (review rounds 2-3). No violation is proven
        // there, whichever rule claims one: it is an obligation.
        for finding in &mut findings {
            if finding.kind == "violation"
                && facts
                    .files
                    .iter()
                    .find(|file| file.path.as_str() == finding.primary_location.path.as_ref())
                    .is_some_and(|file| {
                        solid_reactive_ir::site_in_instance_field(
                            file,
                            solid_facts::core::Span::new(
                                u32::try_from(finding.primary_location.start_byte)
                                    .unwrap_or(u32::MAX),
                                u32::try_from(finding.primary_location.end_byte)
                                    .unwrap_or(u32::MAX),
                            ),
                        )
                    })
            {
                finding.kind = "uncertifiable".into();
                soften_strict_read_hint(&mut finding.hint);
                finding.message.push_str(
                    "; this code is in an instance field initializer, which runs when an instance is constructed, so where it runs is not proven",
                );
            }
        }
        // A function analyzed as a component whose identity is not proven
        // (`solid_reactive_ir::UndecidedRegion`): Solid makes it a component
        // only where `createComponent` calls it, so no violation in its own
        // body is proven. Style rules claim nothing about execution.
        for finding in &mut findings {
            if finding.kind == "violation"
                && !finding.id.starts_with("SC80")
                && solid_reactive_ir::site_in_undecided_component(
                    &program,
                    finding.primary_location.path.as_ref(),
                    finding.primary_location.start_byte,
                    finding.primary_location.end_byte,
                )
            {
                finding.kind = "uncertifiable".into();
                soften_strict_read_hint(&mut finding.hint);
                finding.message.push_str(
                    "; this code is in a function that returns JSX but is also handed to code that may call it as a plain function, so whether it runs as a component is not proven",
                );
            }
        }
        // ADR 0138: a proven component only `Dynamic` renders runs with no
        // strict-read window, so a strict-read claim (SC1001, SC5001) in its
        // own body is not proven. A read in a helper the body calls is
        // reported at that call, in the body; a callback a primitive places
        // (`<For>`'s row, which `mapArray` labels) keeps its own window.
        for finding in &mut findings {
            if finding.kind == "violation"
                && matches!(finding.id.as_str(), "SC1001" | "SC5001")
                && solid_reactive_ir::site_in_windowless_component(
                    &program,
                    finding.primary_location.path.as_ref(),
                    finding.primary_location.start_byte,
                    finding.primary_location.end_byte,
                )
            {
                finding.kind = "uncertifiable".into();
                soften_strict_read_hint(&mut finding.hint);
                finding.message.push_str(
                    "; this code runs in a component only `Dynamic` renders, which calls it with no strict-read window, so the claim is not proven",
                );
            }
        }
        // ADR 0360: a file the compiler refused carries no compiler facts, so
        // nothing about it is proven, and neither is anything a finding
        // elsewhere reaches through it. Its sites are unplaced in the IR; a
        // violation located in it, or whose evidence passes through it, is an
        // obligation here. Each refused file gets a project-level notice.
        let refused = facts
            .files
            .iter()
            .filter_map(|file| {
                file.compiler
                    .refusal()
                    .map(|message| (file.path.as_str(), message))
            })
            .collect::<Vec<_>>();
        if !refused.is_empty() {
            let in_refused = |location: &typefacts::Location| {
                refused
                    .iter()
                    .any(|(path, _)| *path == location.path.as_ref())
            };
            for finding in &mut findings {
                if finding.kind == "violation"
                    && (in_refused(&finding.primary_location)
                        || finding.related_locations.iter().any(in_refused)
                        || finding
                            .evidence
                            .iter()
                            .filter_map(|step| step.location.as_ref())
                            .any(in_refused))
                {
                    finding.kind = "uncertifiable".into();
                    finding.message.push_str(
                        "; this depends on a file the Solid compiler refused, which has no compiler facts, so it is not proven",
                    );
                }
            }
            for (path, message) in &refused {
                findings.push(compiler_refused_finding(path, message));
            }
        }
        let metrics = analysis_metrics(facts, &program);
        let mut snapshot = snapshot_of(sources, metrics, findings);
        // A disabled coverage rule hides its obligations, not the gap they
        // name: the analysis is still incomplete there, so it cannot certify.
        if snapshot.status == "certified" && !withheld_coverage.is_empty() {
            snapshot.status = "uncertifiable".into();
        }
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

/// Drops the findings of disabled rules. Returns the dropped findings of
/// coverage rules (`RuleMetadata::uncertifiable`: SC9005, SC9011, SC9012),
/// which report where the analysis is incomplete rather than a defect.

/// A finding demoted to uncertifiable keeps its rule's hint, whose last
/// sentence asserts the dev warning fires. The may-warn wording the rule
/// uses for its own uncertifiable findings replaces it, so a demoted hint
/// does not contradict the demotion (`strict_read_hint` in
/// `rust/dialects/solid-v2/rules/src/lib.rs` owns both sentences).
fn soften_strict_read_hint(hint: &mut String) {
    const WARNS: &str = "Solid warns STRICT_READ_UNTRACKED here in dev.";
    const MAY_WARN: &str = "If this read executes untracked in dev, Solid warns STRICT_READ_UNTRACKED for it; this finding does not establish that it does, so confirm the read's actual execution and tracking status before relying on that warning.";
    if hint.contains(WARNS) {
        *hint = hint.replace(WARNS, MAY_WARN);
    }
}

fn retain_enabled(
    dialect: &Dialect,
    options: &RuleOptions,
    findings: &mut Vec<Finding>,
) -> Result<Vec<Finding>, BackendError> {
    let mut unknown = Vec::new();
    let mut withheld_coverage = Vec::new();
    for finding in std::mem::take(findings) {
        match (dialect.rule_metadata)(&finding.rule) {
            Some(metadata)
                if options.is_enabled(
                    &finding.rule,
                    metadata.default_enabled,
                    metadata.presets,
                ) =>
            {
                findings.push(finding);
            }
            Some(metadata) => {
                if metadata.uncertifiable && finding.kind == "uncertifiable" {
                    withheld_coverage.push(finding);
                }
            }
            None => unknown.push(finding.rule.clone()),
        }
    }
    if unknown.is_empty() {
        return Ok(withheld_coverage);
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

/// The `SC9015` notice for a file the dialect compiler refused (ADR 0360). It
/// is about the file, not a site in it, and states the compiler's own message:
/// the file has no compiler facts, so the analysis claims nothing about it and
/// withholds what depends on it. Project-level like SC9014, so a run that
/// would otherwise look clean shows it and `--certify` fails.
#[must_use]
pub fn compiler_refused_finding(path: &str, message: &str) -> Finding {
    let location = typefacts::Location {
        path: path.into(),
        start_byte: 0,
        end_byte: 0,
    };
    let reason = message
        .strip_prefix(path)
        .and_then(|rest| rest.strip_prefix(": "))
        .unwrap_or(message);
    // The compiler's first line names the refusal; the rest (the malformed
    // markup, say) goes in the evidence.
    let summary = reason.lines().next().unwrap_or(reason).trim_end();
    let metadata = solid_reactive_ir::RuleMetadata {
        code: dialect::COMPILER_REFUSED_CODE,
        name: dialect::COMPILER_REFUSED_RULE,
        severity: "warning",
        uncertifiable: true,
        default_enabled: true,
        presets: &[],
    };
    let mut finding = Finding::new(
        metadata,
        format!(
            "the Solid compiler refused {path}: {summary}; the file has no compiler facts, so nothing in it is proven and findings that depend on it are not proven either"
        ),
        location.clone(),
    );
    finding.hint = "Findings beside this notice that do not depend on the file stand. To have the file analyzed, rewrite the construct the compiler names in a form the pinned Solid 2 compiler accepts.".into();
    finding.analysis_context = "compiler-facts".into();
    finding.subject_kind = "project".into();
    finding.evidence = vec![solid_reactive_ir::EvidenceStep {
        message: format!("the compiler's message: {reason}"),
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
/// project's facts — the module set package discovery reads.
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

/// The installed package manifests of the given imported modules, which the
/// package boundary reads.
pub fn imported_package_manifest_paths(
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
/// `.solid-checker/rule-options.json`, walking ancestors. A project without one gets upstream's defaults;
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
/// a cached answer exactly like an edited package manifest does.
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
mod tests;
