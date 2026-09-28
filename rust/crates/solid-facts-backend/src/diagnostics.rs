use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use solid_facts::ProjectFacts;
use solid_reactive_ir::{
    CacheRetention, Finding, IncrementalBuilder, Program, RuleOptions, RuntimeEnvironment,
    contract_semantics::{AcceptedContractIndex, ClaimDomain, ValueShape},
    suppress_findings_owned_by_enabled_rules,
};

use crate::dialect::{self, Dialect};
use crate::{BackendError, SemanticDemandOptions, SourceFile};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: String,
    pub findings: Vec<SnapshotFinding>,
    pub package_summaries: Vec<PackageSummary>,
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
pub struct PackageSummary {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contract_hash: String,
    /// `accepted` when an import this analysis read binds the contract;
    /// `refused` when a project catalog holds an acceptance for the package
    /// that no import was admitted to, with `detail` saying why. Presence in a
    /// catalog is never enough for `accepted`.
    pub evidence: String,
    /// Why a `refused` acceptance was not admitted. Empty for `accepted`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    pub exports_analyzed: usize,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticIdentity {
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
        contracts: &AcceptedContractIndex,
    ) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
        self.analyze_accepted_measured_with_enablement(
            project,
            sources,
            facts,
            contracts,
            RequestedRuleEnablement::default(),
        )
        .map(|(analysis, _)| analysis)
    }

    /// Runs ordinary analysis from receipt-validated normalized semantics.
    /// Wire decoding, artifact selection, and receipt validation must finish
    /// before this entry point is called.
    pub fn analyze_accepted_measured_with_enablement(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        contracts: &AcceptedContractIndex,
        enablement: RequestedRuleEnablement<'_>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        let external_contracts = contracts.external_packages();
        let contracts = external_contracts.as_ref();
        let ir_started = Instant::now();
        let mut rule_options = discover_rule_options(project)?;
        rule_options.request_presets(enablement.presets.iter().cloned());
        rule_options.request_rules(enablement.rules.iter().cloned());
        enablement
            .runtime
            .validate()
            .map_err(BackendError::Contract)?;
        rule_options.runtime = enablement.runtime;
        // Scoped to this generation's facts: a gap about named exports is due
        // only where the project reaches one (`release_scope`), so the answer
        // moves with the sources as well as the install.
        let release_notice = dialect::release_notice(self.dialect, project)
            .and_then(|notice| notice.scoped_to(facts));
        let identity = DiagnosticIdentity {
            dialect: self.dialect.id,
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: vec![contracts.cache_fingerprint()],
            rule_options: rule_options.clone(),
            release_notice: release_notice.clone(),
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
        let (program, _) = self.builder.build_with_accepted_contracts_shared(
            facts,
            self.dialect.vocabulary,
            contracts,
            &rule_options,
        )?;
        let reactive_ir = ir_started.elapsed();
        let solve_started = Instant::now();
        let mut findings = self.dialect.solve(&program);
        retain_enabled(self.dialect, &rule_options, &mut findings)?;
        suppress_findings_owned_by_enabled_rules(&mut findings, self.dialect.catalog_capabilities);
        // After enablement, like the refusal: the notice is about the installed
        // runtime, not a site, so there is nothing to suppress it at.
        if let Some(notice) = &release_notice {
            findings.push(unaudited_release_finding(notice));
        }
        let metrics = analysis_metrics(facts, &program, contracts);
        let snapshot = snapshot_with_package_summaries(
            sources,
            accepted_package_summaries(facts, contracts),
            metrics,
            findings,
        );
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

pub fn analyze_project_accepted_measured_with_enablement(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    enablement: RequestedRuleEnablement<'_>,
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    DiagnosticSession::new(dialect)
        .analyze_accepted_measured_with_enablement(project, sources, facts, contracts, enablement)
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
        package_summaries: Vec::new(),
        metrics: Metrics {
            files_analyzed: 0,
            functions_analyzed: 0,
            proof_obligations: 0,
            cached_summaries: 0,
            unresolved_obligations: 0,
        },
    }
}

fn snapshot_with_package_summaries(
    sources: &[SourceFile],
    package_summaries: Vec<PackageSummary>,
    metrics: Metrics,
    findings: Vec<Finding>,
) -> Snapshot {
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
        status: status.into(),
        findings,
        package_summaries,
        metrics,
    }
}

/// The value of [`PackageSummary::evidence`] for a contract an import binds.
pub const PACKAGE_EVIDENCE_ACCEPTED: &str = "accepted";
/// The value of [`PackageSummary::evidence`] for a project-catalog acceptance
/// no import of this analysis was admitted to.
pub const PACKAGE_EVIDENCE_REFUSED: &str = "refused";

/// Said of a refused acceptance whose package the admission rule's steps 1-3
/// did not refuse: the receipt, artifact and environment hold, and still no
/// import this analysis read selected it.
const NOT_SELECTED_BY_ANY_IMPORT: &str = "a project catalog entry exists for this package and \
     was not admitted: no import this analysis read selected it (resolved file, export \
     conditions or case agreement)";

/// What the analysis read, and what it was offered and refused.
///
/// `accepted` is decided by binding, the way the analysis decides it: an
/// import (or `export … from`) in a project file whose exact importer and
/// specifier, or whose specifier admitted by artifact, selects a contract.
/// Presence in the index is not that. A project catalog's entries are keyed on
/// the certification importer `contract certify` writes beside the package,
/// which is never a project file, so every one of them sits in the index
/// whether or not this tree admitted it -- and listing the index said
/// `accepted` of an acceptance whose receipt states no environment, and of one
/// carried into a tree whose installs differ. Measured on kobalte core
/// (phase22, 2026-09-26): `vite-plugin-solid`, and `@solid-primitives/form`
/// under signals rc.6.
///
/// Every such catalog entry that nothing bound is reported `refused`, with
/// the admission rule's own sentence when it names the package
/// ([`admission_refusal_details`], carried on the index by specifier), and a
/// plain statement that no import selected it otherwise.
fn accepted_package_summaries(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
) -> Vec<PackageSummary> {
    let summary = |package: &solid_reactive_ir::contract_semantics::PackageIdentity,
                   digest: &str,
                   evidence: &str,
                   detail: &str| PackageSummary {
        name: package.name.clone(),
        version: package.version.clone(),
        contract_hash: digest.into(),
        evidence: evidence.into(),
        detail: detail.into(),
        exports_analyzed: 0,
    };
    let mut summaries = Vec::new();
    for file in &facts.files {
        let modules = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str())
            .chain(
                file.ast
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| export.module.as_deref()),
            );
        for module in modules {
            if let Ok(contract) = contracts.contract(file.path.as_str(), module) {
                summaries.push(summary(
                    contract.package(),
                    contract.semantic_identity().semantic_digest.as_str(),
                    PACKAGE_EVIDENCE_ACCEPTED,
                    "",
                ));
            }
        }
    }
    let accepted = summaries
        .iter()
        .map(|row| {
            (
                row.name.clone(),
                row.version.clone(),
                row.contract_hash.clone(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    for (binding, refusal) in contracts.all_semantic_identities() {
        let semantics = &binding.semantics;
        let key = (
            semantics.package.name.clone(),
            semantics.package.version.clone(),
            semantics.semantic_digest.as_str().to_owned(),
        );
        if accepted.contains(&key) {
            continue;
        }
        summaries.push(summary(
            &semantics.package,
            semantics.semantic_digest.as_str(),
            PACKAGE_EVIDENCE_REFUSED,
            refusal.unwrap_or(NOT_SELECTED_BY_ANY_IMPORT),
        ));
    }
    summaries.sort_by(|left, right| {
        (
            &left.name,
            &left.version,
            &left.contract_hash,
            &left.evidence,
            &left.detail,
        )
            .cmp(&(
                &right.name,
                &right.version,
                &right.contract_hash,
                &right.evidence,
                &right.detail,
            ))
    });
    summaries.dedup_by(|left, right| {
        left.name == right.name
            && left.version == right.version
            && left.contract_hash == right.contract_hash
            && left.evidence == right.evidence
            && left.detail == right.detail
    });
    summaries
}

pub fn analysis_metrics(
    facts: &ProjectFacts,
    program: &Program,
    contracts: &AcceptedContractIndex,
) -> Metrics {
    let mut aliases = facts
        .typescript
        .symbols()
        .filter(|symbol| !symbol.alias_target().is_empty())
        .map(|symbol| (symbol.id().into(), symbol.alias_target().into()))
        .collect::<HashMap<String, String>>();
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
    let canonical = |symbol: &str| {
        aliases
            .get(symbol)
            .map_or_else(|| symbol.to_owned(), Clone::clone)
    };
    let entities = facts
        .typescript
        .entities()
        .filter(|entity| !entity.symbol.is_empty())
        .map(|entity| {
            (
                (
                    entity.location.path.as_ref(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ),
                canonical(&entity.symbol),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut contracted_functions = HashMap::<String, Option<String>>::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            // The same identity gate contract resolution applies: a metric that
            // counted a contract this analysis refused to bind would report
            // certified summaries the analysis never used.
            let Ok(contract) = contracts.contract(file.path.as_str(), &import.module) else {
                continue;
            };
            for binding in &import.bindings {
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    continue;
                }
                let exported = binding.imported.as_deref().unwrap_or("default");
                let Some(summary) = contract.export(exported) else {
                    continue;
                };
                let reads = summary
                    .operation_claim(ClaimDomain::Reads)
                    .into_iter()
                    .flat_map(|claim| claim.items());
                let returns = summary
                    .operation_claim(ClaimDomain::Returns)
                    .into_iter()
                    .flat_map(|claim| claim.items())
                    .collect::<Vec<_>>();
                if summary.callbacks().items().is_empty()
                    && reads.count() == 0
                    && returns.is_empty()
                {
                    continue;
                }
                let Some(symbol) = entities.get(&(
                    file.path.as_str(),
                    u64::from(binding.local.span.start),
                    u64::from(binding.local.span.end),
                )) else {
                    continue;
                };
                contracted_functions.insert(
                    symbol.to_string(),
                    returns
                        .iter()
                        .filter_map(|operation| summary.operation(&operation.0))
                        .filter_map(|operation| operation.output.as_ref())
                        .any(|shape| matches!(shape, ValueShape::Reactive { .. }))
                        .then(|| "accessor".to_string()),
                );
            }
        }
    }
    let factory_instances = facts
        .typescript
        .files()
        .flat_map(|file| file.bindings.iter())
        .filter(|binding| {
            !binding.array
                && !binding.names.is_empty()
                && contracted_functions
                    .get(&canonical(&binding.initializer.target))
                    .is_some_and(|returned| returned.as_deref() == Some("accessor"))
        })
        .count();
    let functions_analyzed = facts
        .typescript
        .files()
        .map(|file| file.functions.len())
        .sum::<usize>()
        + contracted_functions.len()
        + factory_instances
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContractStatus {
    pub name: String,
    pub status: String,
    /// Exact registry integrity recovered from the active package-manager
    /// lock. Proposal generation requires this identity and refuses local,
    /// linked, or otherwise unattested packages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_integrity: Option<String>,
    /// Why the status is what it is, when the status alone does not say it —
    /// the two disagreeing versions behind `stale`, for instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// What the user should do about this status, or `None` when the contract
    /// already certifies. Built by [`contract_remedy`] so the report and the
    /// analysis error cannot print divergent instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    pub contract_path: String,
    /// The analysed files whose imports of this package resolve to this row's
    /// installed artifact, relative to the project directory. Present only
    /// when the name alone does not identify the artifact: the package is
    /// installed as more than one artifact across the importing files (one row
    /// each), or its importers find it somewhere other than the project
    /// directory's own lookup -- a sub-package's `node_modules` in a monorepo
    /// analysed from its root. A single-package project never carries it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub importers: Vec<String>,
}

impl PackageContractStatus {
    /// Whether this status blocks contract-backed certification. These are the
    /// statuses `--check-contracts` counts and exits non-zero on.
    ///
    /// `unbound` is one of them and is produced only by
    /// [`package_contract_statuses`], the `--check-contracts` path: a contract
    /// no import binds certifies nothing, so the report must not count it as
    /// coverage. The analysis path
    /// ([`package_contract_statuses_with`]) never produces it, because a
    /// refusal is deliberately silent in the findings.
    pub fn needs_action(&self) -> bool {
        matches!(
            self.status.as_str(),
            "missing" | "unverified" | "stale" | "unbound" | "unsupported-runtime"
        )
    }
}

/// Why acceptances for each package were not admitted, replayed from one
/// directory; see [`accepted_package_contract_statuses`].
pub type RefusalReplay<'a> = dyn Fn(&Path) -> Result<BTreeMap<String, String>, BackendError> + 'a;

/// Reports external receipt coverage and the separately identified built-in
/// runtime foundation. Built-in model selection is not artifact certification.
/// Coverage is complete only when every exact imported specifier binds in the
/// already receipt-validated normalized index.
///
/// An external package has one row per installed artifact its importers
/// resolve to, each looked up from the importing files ([`imported_artifacts`])
/// and counted over those files' import rows only.
///
/// `refusals` names, per installed package, why an acceptance that exists for
/// it in a project catalog or the compiled-in tier was not admitted
/// ([`admission_refusal_details`]), replayed from the directory it is handed:
/// the project directory for a row whose importers reach the project
/// directory's own install, and the install directory of the row's artifact
/// otherwise, because that is where its admission was replayed from. It only
/// adds to the detail of a status that is not `certified`, and never changes
/// a status.
pub fn accepted_package_contract_statuses(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    refusals: &RefusalReplay,
) -> Result<Vec<PackageContractStatus>, BackendError> {
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    let mut replayed = BTreeMap::<PathBuf, BTreeMap<String, String>>::new();
    let mut refusals_from = |directory: &Path| -> Result<BTreeMap<String, String>, BackendError> {
        if let Some(refusals) = replayed.get(directory) {
            return Ok(refusals.clone());
        }
        let answer = refusals(directory)?;
        replayed.insert(directory.to_path_buf(), answer.clone());
        Ok(answer)
    };
    // Asking the vocabulary rather than matching the id. The list was the same
    // three names written out again, and the `_ => &[]` arm a third dialect
    // would have landed in reports "no package needs a contract" for exactly
    // the packages that dialect ships -- a silent wrong answer, not a gap.
    let first_party = dialect.vocabulary.primitive_defining_packages();
    let resolved = facts.resolved_imports.as_ref();
    let mut statuses = Vec::new();
    for module in imported_package_roots(facts) {
        if let Some(core_package) = core_runtime_package_for_status(&module, facts) {
            let modeled = dialect
                .vocabulary
                .primitive_defining_packages()
                .contains(&core_package);
            statuses.push(PackageContractStatus {
                name: module,
                // `unsupported-runtime` is retained deliberately although a
                // build carrying only the Solid 2 vocabulary cannot produce
                // it: v2's `primitive_defining_packages` is the whole union
                // across dialects, so every core package it can see is
                // modelled. It is kept for the same reason `Version::V1` is --
                // the seam has to exist before the next dialect needs it, and
                // a status silently dropped from `--check-contracts` output
                // and from the certification-blocking set is harder to
                // reinstate than one that was never removed. See ADR 0110 and
                // the retirement plan's step-3 notes.
                status: if modeled { "builtin" } else { "unsupported-runtime" }.into(),
                installed_integrity: None,
                detail: Some(if modeled {
                    format!("built-in runtime model {}; no package receipt is required; model selection is not installed-artifact authentication", dialect.vocabulary.runtime_model_identity())
                } else {
                    format!("this core package is outside the {} runtime model", dialect.id)
                }),
                remedy: (!modeled).then(|| "select a compatible Solid dialect and runtime; a core package contract cannot extend the built-in model".into()),
                contract_path: String::new(),
                importers: Vec::new(),
            });
            continue;
        }
        let artifacts = imported_artifacts(project_directory, facts, &module)?;
        // The artifact the project directory's own lookup finds. A row whose
        // importers find exactly that one is the row this report always
        // printed, so it names no importers and a single-package project reads
        // byte-identically.
        let project_artifact = discover_package_directory(project_directory, &module)?
            .map(|directory| artifact_key(&directory));
        let split = artifacts.len() > 1;
        for artifact in artifacts {
            let installed = artifact
                .directory
                .as_deref()
                .map(read_installed_manifest)
                .transpose()?
                .flatten();
            let manifest = installed.as_ref().map(|(_, manifest)| manifest);
            if !first_party.contains(&module.as_str()) && !manifest.is_some_and(manifest_uses_solid)
            {
                continue;
            }
            let importers = if split || artifact.key != project_artifact {
                artifact
                    .importers
                    .iter()
                    .map(|importer| {
                        Path::new(importer)
                            .strip_prefix(project_directory)
                            .map_or_else(|_| importer.clone(), |path| path.display().to_string())
                    })
                    .collect()
            } else {
                Vec::new()
            };
            // Where this artifact's admission was evaluated from: the project
            // directory when its importers reach the project directory's own
            // install, which keeps a single-package project byte-identical,
            // and otherwise the directory whose `node_modules` holds it, where
            // the lockfile governing that copy is found.
            let admission_directory = match &artifact.base {
                Some(base) if artifact.key != project_artifact => base.as_path(),
                _ => project_directory,
            };
            let refusals = refusals_from(admission_directory)?;
            let mut status = artifact_contract_status(
                project_directory,
                admission_directory,
                &module,
                installed.as_ref(),
                resolved,
                &artifact.importers,
                contracts,
                &refusals,
            )?;
            status.importers = importers;
            statuses.push(status);
        }
    }
    // Stable: the rows of one name keep their artifact order.
    statuses.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(statuses)
}

/// One installed artifact of a package name, with the analysed files whose
/// imports find it.
struct ImportedArtifact {
    /// [`artifact_key`] of `directory`; `None` when no importer finds the
    /// package installed at all.
    key: Option<PathBuf>,
    /// The package directory as the first importer's lookup spells it.
    directory: Option<PathBuf>,
    /// The directory whose `node_modules` holds `directory`.
    base: Option<PathBuf>,
    importers: std::collections::BTreeSet<String>,
}

/// What identifies an installed artifact across lookup spellings: two
/// sub-packages' `node_modules/<name>` links into one package-manager store
/// directory are one artifact.
fn artifact_key(directory: &Path) -> PathBuf {
    fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf())
}

/// The installed artifacts `module` resolves to from the analysed files that
/// import it, each looked up from the importing file's own location -- the
/// `node_modules` walk the analysis's resolution performs per file, not one
/// lookup from the project directory. A monorepo root analyses sub-packages
/// whose dependencies are installed under their own `node_modules` and nowhere
/// at the root; looking only from the project directory left those packages
/// out of `contract check` while the analysis raised obligations for them.
///
/// The importers are the files that name the package in an import and the
/// files an attested import row resolves into the package (an aliased
/// specifier), which is every file whose rows the status counts. Sorted by
/// artifact key, so the output is deterministic.
fn imported_artifacts(
    project_directory: &Path,
    facts: &ProjectFacts,
    module: &str,
) -> Result<Vec<ImportedArtifact>, BackendError> {
    let mut importers = std::collections::BTreeSet::new();
    for file in &facts.files {
        if file
            .ast
            .imports
            .iter()
            .any(|import| package_root(&import.module) == module)
        {
            importers.insert(file.path.to_string());
        }
    }
    if let Some(resolved) = &facts.resolved_imports {
        for (importer, import) in resolved.iter() {
            if resolved_package_name(import) == Some(module) {
                importers.insert(importer.to_owned());
            }
        }
    }
    let mut lookups = HashMap::<PathBuf, Option<(PathBuf, PathBuf)>>::new();
    let mut artifacts = BTreeMap::<Option<PathBuf>, ImportedArtifact>::new();
    for importer in importers {
        let from = Path::new(&importer)
            .parent()
            .unwrap_or(project_directory)
            .to_path_buf();
        let install = match lookups.get(&from) {
            Some(install) => install.clone(),
            None => {
                let install = discover_package_install(&from, module)?;
                lookups.insert(from, install.clone());
                install
            }
        };
        let (base, directory) = install.unzip();
        let key = directory.as_deref().map(artifact_key);
        artifacts
            .entry(key.clone())
            .or_insert_with(|| ImportedArtifact {
                key,
                directory,
                base,
                importers: std::collections::BTreeSet::new(),
            })
            .importers
            .insert(importer);
    }
    Ok(artifacts.into_values().collect())
}

/// The package an attested import resolved into, as `contract check` and the
/// obligation discovery both name it: the resolver's own identity first.
fn resolved_package_name(import: &solid_facts::AttestedImport) -> Option<&str> {
    import
        .resolver_package_name
        .as_deref()
        .or(import.package_name.as_deref())
}

/// The status of one installed artifact of `module`, counted over the
/// attested import rows of `importers` only. Its lockfile integrity is read
/// from `admission_directory`, where its admission was evaluated from.
#[allow(clippy::too_many_arguments)]
fn artifact_contract_status(
    project_directory: &Path,
    admission_directory: &Path,
    module: &str,
    installed: Option<&(PathBuf, PackageManifest)>,
    resolved: Option<&solid_facts::AttestedImportIndex>,
    importers: &std::collections::BTreeSet<String>,
    contracts: &AcceptedContractIndex,
    refusals: &BTreeMap<String, String>,
) -> Result<PackageContractStatus, BackendError> {
    let module = module.to_owned();
    let package_directory = installed.map(|(directory, _)| directory.as_path());
    let installed_integrity = package_directory
        .map(|directory| installed_package_integrity(admission_directory, directory))
        .transpose()?
        .flatten();
    let imports = resolved
        .into_iter()
        .flat_map(|imports| imports.iter())
        .filter(|(importer, import)| {
            resolved_package_name(import) == Some(module.as_str()) && importers.contains(*importer)
        })
        .collect::<Vec<_>>();
    let bound = imports
        .iter()
        .filter(|(importer, import)| contracts.contract(importer, &import.text).is_ok())
        .count();
    let (status, detail, remedy, contract_path) = if !imports.is_empty() && bound == imports.len() {
        (
            "certified".into(),
            None,
            None,
            "receipt-issued stable-v1 index".into(),
        )
    } else {
        let status = if bound == 0 { "missing" } else { "unbound" };
        let detail = if imports.is_empty() {
            "exact import identity facts are unavailable".to_owned()
        } else if bound == 0 {
            format!(
                "none of the {} exact imported artifact case(s) has a matching receipt",
                imports.len()
            )
        } else {
            format!(
                "{bound} of {} exact imported artifact case(s) have matching receipts",
                imports.len()
            )
        };
        let detail = Some(match refusals.get(&module) {
            Some(refusal) => format!("{detail}; {refusal}"),
            None => detail,
        });
        let root = package_directory.map_or_else(
            || format!("node_modules/{module}"),
            |path| {
                path.strip_prefix(project_directory)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            },
        );
        let remedy = installed_integrity.as_ref().map_or_else(
            || {
                Some(
                    "the package manager supplied no exact registry integrity; linked or local packages remain uncertifiable"
                        .into(),
                )
            },
            |integrity| {
                Some(format!(
                    "solid-checker contract certify --package-root {root} --integrity {integrity} --catalog .solid-checker/accepted-contracts.json --issuer-configuration <issuer.json> --trust-configuration-output <trust.json>, run from this project; it adds the package to the project catalog, which `solid-checker contract check --receipt-trust-configuration <trust.json>` then reads"
                ))
            },
        );
        (status.into(), detail, remedy, String::new())
    };
    Ok(PackageContractStatus {
        name: module,
        status,
        installed_integrity,
        detail,
        remedy,
        contract_path,
        importers: Vec::new(),
    })
}

/// Why an acceptance that exists for an installed package was not admitted by
/// artifact, one sentence per package, for `contract check` to append to a
/// status that is not `certified`.
///
/// Every acceptance is replayed through steps 1-3 of the one admission rule
/// ([`crate::accepted_bundles::admission_refusals`]); a package is named only
/// when *every* acceptance for it failed one of those steps, because otherwise
/// the refusal lies in the resolved file or case selection, which this does not
/// explain. Project catalogs are consulted before the compiled-in tier, and
/// within a tier the most specific refusal wins: an environment that differs
/// (the artifact matched) over a receipt that states none, over an artifact
/// that is not the certified one.
pub fn admission_refusal_details(
    project_directory: &Path,
    catalogs: &[PathBuf],
    bundled: bool,
) -> Result<BTreeMap<String, String>, BackendError> {
    use crate::accepted_bundles::AdmissionRefusal;
    let patches = crate::installed_patches::InstalledPatches::read(project_directory);
    let snapshots = InstalledSnapshots::default();
    let installed = |specifier: &str| installed_artifact_identity(project_directory, specifier);
    let bytes = |specifier: &str| {
        installed_artifact_bytes(project_directory, specifier, &patches, &snapshots)
    };
    let difference = |specifier: &str, environment: &[crate::DependencyEnvironmentEntry]| {
        installed_environment_difference(project_directory, specifier, environment, &patches)
    };
    let contract = |error: crate::ContractFailure| BackendError::Contract(error.to_string());
    let mut tiers = vec![(
        "a project catalog entry",
        crate::contract_interface::project_admission_refusals(
            catalogs,
            &installed,
            &bytes,
            &difference,
        )
        .map_err(contract)?,
    )];
    if bundled {
        tiers.push((
            "a compiled-in contract",
            crate::accepted_bundles::bundle_admission_refusals(&installed, &bytes, &difference)
                .map_err(contract)?,
        ));
    }
    let rank = |refusal: &AdmissionRefusal| match refusal {
        AdmissionRefusal::EnvironmentDiffers(_) => 0,
        AdmissionRefusal::InstalledBytesDiffer(_) => 1,
        AdmissionRefusal::NoEnvironmentStated => 2,
        AdmissionRefusal::AcceptanceRootNotReproduced { .. } => 3,
        AdmissionRefusal::CitationWithdrawn(_) => 4,
    };
    let mut details = BTreeMap::new();
    for (tier, refusals) in tiers {
        let mut by_package = BTreeMap::<String, Vec<Option<AdmissionRefusal>>>::new();
        for (specifier, refusal) in refusals {
            if let Some(module) = package_name_of_specifier(&specifier) {
                by_package.entry(module).or_default().push(refusal);
            }
        }
        for (module, refusals) in by_package {
            if details.contains_key(&module) || refusals.iter().any(Option::is_none) {
                continue;
            }
            if let Some(refusal) = refusals
                .iter()
                .flatten()
                .min_by_key(|refusal| rank(refusal))
            {
                details.insert(
                    module,
                    format!("{tier} exists for this package and was not admitted: {refusal}"),
                );
            }
        }
    }
    Ok(details)
}

/// Reporting-only classification. An alias counts as core only when every
/// exact import occurrence resolves to the same defining package. This does
/// not authenticate its bytes or grant semantics to the source program.
fn core_runtime_package_for_status<'a>(
    module: &'a str,
    facts: &'a ProjectFacts,
) -> Option<&'a str> {
    if solid_dialect::primitive_defining_package(module) {
        return Some(module);
    }
    let resolved = facts.resolved_imports.as_ref()?;
    let mut selected = None;
    for file in &facts.files {
        for import in file
            .ast
            .imports
            .iter()
            .filter(|import| package_root(&import.module) == module)
        {
            let solid_facts::SpecifierAttestation::Attested(answer) =
                resolved.specifier(file.path.as_str(), import.span, &import.module)
            else {
                return None;
            };
            if answer.resolution != solid_facts::ImportResolution::NodeModules {
                return None;
            }
            let package = answer
                .resolver_package_name
                .as_deref()
                .or(answer.package_name.as_deref())?;
            if !solid_dialect::primitive_defining_package(package)
                || selected.is_some_and(|other| other != package)
            {
                return None;
            }
            selected = Some(package);
        }
    }
    selected
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageManifest {
    #[serde(default)]
    version: String,
    #[serde(default)]
    dependencies: HashMap<String, serde_json::Value>,
    #[serde(default)]
    peer_dependencies: HashMap<String, serde_json::Value>,
    #[serde(default)]
    optional_dependencies: HashMap<String, serde_json::Value>,
}

/// The installed package directory and its parsed manifest, read once.
///
/// Ambient-module and virtual test projects have no installed package and no
/// manifest; both cases yield `None`, which every caller reads as "there is no
/// installed version to disagree with".
/// The specifiers this project may import under an existing acceptance,
/// because the installed artifact their importers reach is the one that
/// acceptance names.
///
/// Lives here rather than beside the catalog reader because the answer depends
/// on the installed tree, which is this module's business: the package the
/// specifier resolves to, its version, and the registry integrity its lockfile
/// selected. A package whose installs disagree yields no integrity at all
/// (`installed_package_integrity` returns `None`), so a project with two
/// versions of one dependency admits neither — which is the nested-install case
/// the importer key used to guard.
///
/// Evaluated from each importer's own install ([`ImporterInstalls`]): a
/// specifier every importer reaches at the project directory's own install is
/// admitted project-wide, exactly as before; one that some importer reaches
/// elsewhere is admitted per install, for the importers reaching it.
pub fn admitted_project_artifacts(
    catalogs: &[PathBuf],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
) -> Result<ArtifactAdmissions, BackendError> {
    let installs = importer_installs(project_directory, facts, None)?;
    admitted_catalog_artifacts(
        catalogs,
        trust,
        project_directory,
        conditions,
        facts,
        None,
        &installs,
    )
}

/// [`admitted_project_artifacts`] with the resolved-file fact taken only from
/// the importers `within` a directory -- a nested catalog's, whose admission
/// speaks for those files alone. `None` is every importer.
fn admitted_catalog_artifacts(
    catalogs: &[PathBuf],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
    within: Option<&Path>,
    installs: &ImporterInstalls,
) -> Result<ArtifactAdmissions, BackendError> {
    // The same environment check as the compiled-in tier (ADR 0123): a project
    // catalog certified in this tree reproduces it by construction, and one
    // carried into a tree whose installs differ does not.
    admitted_by_install(
        project_directory,
        facts,
        within,
        installs,
        &|installed, bytes, resolved_target, environment| {
            crate::contract_interface::admitted_project_artifacts(
                catalogs,
                trust,
                project_directory,
                conditions,
                installed,
                bytes,
                resolved_target,
                environment,
            )
        },
    )
}

/// Where the analysed files' imports reach an installed package other than
/// the one the admission directory's own lookup finds.
///
/// Artifact admission asks whether *the installed copy an import resolves to*
/// is the artifact an acceptance was proven about. It used to ask that of the
/// copy the project directory's own `node_modules` walk finds, which is the
/// right copy only when every importer finds that one too. A monorepo root
/// analysing `packages/*`, whose dependencies are installed under each
/// sub-package's `node_modules` and nowhere at the root, found none, and every
/// acceptance was refused with "the installed package has no exact lockfile
/// integrity" -- while the same files analysed from their sub-package were
/// admitted (measured on kobalte, 2026-09-26).
///
/// A file's install is found from its own directory with the same walk
/// [`imported_artifacts`] performs for `contract check`, and is identified by
/// its canonical directory. A specifier none of whose importers reaches an
/// install other than the directory's own is not recorded, so a
/// single-package project has nothing here and is admitted byte-identically.
/// A specifier some importer reaches elsewhere is *split*: nothing admits it
/// project-wide, and each install context below admits it for the importers
/// that reach it.
#[derive(Debug, Default)]
struct ImporterInstalls {
    /// The bare specifiers some attested import reaches at an install other
    /// than the admission directory's own.
    split: std::collections::BTreeSet<String>,
    /// Per install directory -- the directory whose `node_modules` holds the
    /// copy, where its lookup, its lockfile walk and its environment replay
    /// start -- each split specifier and the importers reaching it there.
    /// Importers of a split specifier that reach the admission directory's own
    /// copy (or none at all) are recorded under the admission directory.
    contexts: BTreeMap<PathBuf, BTreeMap<String, std::collections::BTreeSet<String>>>,
}

impl ImporterInstalls {
    /// The install directories other than `directory`, each with the modules
    /// looked up there. A host caching an admission verdict hashes their
    /// manifests and lockfiles.
    fn elsewhere<'a>(&'a self, directory: &'a Path) -> impl Iterator<Item = (&'a Path, String)> {
        self.contexts
            .iter()
            .filter(move |(base, _)| base.as_path() != directory)
            .flat_map(|(base, specifiers)| {
                specifiers
                    .keys()
                    .filter_map(|specifier| package_name_of_specifier(specifier))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .map(move |module| (base.as_path(), module))
            })
    }
}

/// The installs artifact admission reads for the analysed files besides the
/// ones `directory`'s own lookup finds: `(install directory, module)` for
/// every import some file reaches at another install ([`ImporterInstalls`]),
/// the project's and each nested catalog scope's, sorted. A host caching an
/// admission verdict hashes the manifest `module` resolves to from there and
/// [`admission_input_paths`] of the install directory, which is where that
/// copy's identity, lockfile integrity and environment replay are read.
/// Empty for a project whose importers all reach its own installs.
pub fn importer_admission_inputs(
    directory: &Path,
    nested: &[NestedCatalogs],
    facts: &solid_facts::ProjectFacts,
) -> Result<Vec<(PathBuf, String)>, BackendError> {
    let mut inputs = std::collections::BTreeSet::new();
    let installs = importer_installs(directory, facts, None)?;
    inputs.extend(
        installs
            .elsewhere(directory)
            .map(|(base, module)| (base.to_path_buf(), module)),
    );
    for scope in nested {
        let scope = scope.directory.as_path();
        let installs = importer_installs(scope, facts, Some(scope))?;
        inputs.extend(
            installs
                .elsewhere(scope)
                .map(|(base, module)| (base.to_path_buf(), module)),
        );
    }
    Ok(inputs.into_iter().collect())
}

/// A specifier naming an installed package, as opposed to a relative,
/// absolute or `node:` one, which no `node_modules` walk answers.
fn names_installed_package(specifier: &str) -> bool {
    !specifier.starts_with('.') && !specifier.starts_with('/') && !specifier.starts_with("node:")
}

/// [`ImporterInstalls`] for the attested imports of the files `within` a
/// directory (every file when `None`), relative to `directory`'s own lookup.
fn importer_installs(
    directory: &Path,
    facts: &solid_facts::ProjectFacts,
    within: Option<&Path>,
) -> Result<ImporterInstalls, BackendError> {
    use std::collections::hash_map::Entry;
    type Reached = Option<(PathBuf, PathBuf)>;
    let mut installs = ImporterInstalls::default();
    let Some(attested) = facts.resolved_imports.as_ref() else {
        return Ok(installs);
    };
    let mut lookups = HashMap::<(PathBuf, String), Reached>::new();
    let mut rows = BTreeMap::<String, Vec<(String, Reached)>>::new();
    for (importer, import) in attested.iter() {
        let specifier = import.text.as_str();
        if import.resolution == solid_facts::ImportResolution::Unresolved
            || !names_installed_package(specifier)
            || within.is_some_and(|within| !Path::new(importer).starts_with(within))
        {
            continue;
        }
        let Some(module) = package_name_of_specifier(specifier) else {
            continue;
        };
        let from = Path::new(importer)
            .parent()
            .unwrap_or(directory)
            .to_path_buf();
        let reached = match lookups.entry((from, module)) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let (from, module) = entry.key();
                let reached = discover_package_install(from, module)?
                    .map(|(base, package)| (base, artifact_key(&package)));
                entry.insert(reached).clone()
            }
        };
        rows.entry(specifier.to_owned())
            .or_default()
            .push((importer.to_owned(), reached));
    }
    let mut own = HashMap::<String, Option<PathBuf>>::new();
    for (specifier, rows) in rows {
        let Some(module) = package_name_of_specifier(&specifier) else {
            continue;
        };
        let own_key = match own.entry(module) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let key = discover_package_directory(directory, entry.key())?
                    .map(|package| artifact_key(&package));
                entry.insert(key).clone()
            }
        };
        let elsewhere = |reached: &Reached| {
            reached
                .as_ref()
                .filter(|(_, key)| own_key.as_ref() != Some(key))
                .map(|(base, _)| base.clone())
        };
        if !rows.iter().any(|(_, reached)| elsewhere(reached).is_some()) {
            continue;
        }
        installs.split.insert(specifier.clone());
        for (importer, reached) in rows {
            let base = elsewhere(&reached).unwrap_or_else(|| directory.to_path_buf());
            installs
                .contexts
                .entry(base)
                .or_default()
                .entry(specifier.clone())
                .or_default()
                .insert(importer);
        }
    }
    Ok(installs)
}

/// One tier's artifact admission (ADR 0123) over the installed-tree facts it
/// is handed: [`crate::accepted_bundles::admitted_bundle_artifacts`] or
/// [`crate::contract_interface::admitted_project_artifacts`].
type TierAdmission<'a> = dyn Fn(
        &crate::contract_interface::InstalledArtifactIdentity,
        &crate::accepted_bundles::InstalledArtifactBytes,
        &crate::contract_interface::ResolvedTargetIdentity,
        &crate::accepted_bundles::InstalledEnvironment,
    ) -> Result<Vec<(String, String)>, crate::ContractFailure>
    + 'a;

/// The importers of each specifier in one install context.
type ContextImporters = BTreeMap<String, std::collections::BTreeSet<String>>;

/// What one tier admits by artifact: `(specifier, artifact identity)` pairs
/// for the whole project, and per install context the pairs it admitted there
/// with the importers they apply to ([`ImporterInstalls`]).
#[derive(Debug, Default)]
pub struct ArtifactAdmissions {
    project_wide: Vec<(String, String)>,
    installs: Vec<(Vec<(String, String)>, ContextImporters)>,
}

impl ArtifactAdmissions {
    /// Folds these admissions into `contracts`: project-wide by specifier,
    /// the rest by importer. Admitting nothing leaves the index unchanged.
    #[must_use]
    pub fn admit_into(self, contracts: AcceptedContractIndex) -> AcceptedContractIndex {
        let mut contracts = contracts;
        if !self.project_wide.is_empty() {
            contracts = contracts.with_admitted_artifacts(self.project_wide);
        }
        let mut by_importer = Vec::new();
        for (admitted, importers) in self.installs {
            for (specifier, identity) in admitted {
                for importer in importers.get(&specifier).into_iter().flatten() {
                    by_importer.push((importer.clone(), specifier.clone(), identity.clone()));
                }
            }
        }
        if !by_importer.is_empty() {
            contracts = contracts.with_admitted_artifacts_for(by_importer);
        }
        contracts
    }

    /// Whether nothing was admitted anywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.project_wide.is_empty() && self.installs.is_empty()
    }

    /// [`agreed_admissions`] within each context: candidates for one
    /// specifier are compared only with candidates from the same install.
    fn agreed(self, contracts: &AcceptedContractIndex) -> Self {
        Self {
            project_wide: agreed_admissions(contracts, self.project_wide),
            installs: self
                .installs
                .into_iter()
                .map(|(admitted, importers)| (agreed_admissions(contracts, admitted), importers))
                .collect(),
        }
    }
}

/// Runs one tier's admission from every install its importers reach: once
/// from `directory` for the specifiers no importer reaches elsewhere, and once
/// per install context for the split ones, each context's facts -- identity,
/// lockfile integrity, environment replay and resolved file -- read from its
/// own install directory and its own importers only.
fn admitted_by_install(
    directory: &Path,
    facts: &solid_facts::ProjectFacts,
    within: Option<&Path>,
    installs: &ImporterInstalls,
    tier: &TierAdmission,
) -> Result<ArtifactAdmissions, BackendError> {
    let contract = |error: crate::ContractFailure| BackendError::Contract(error.to_string());
    let counted =
        |importer: &str| within.is_none_or(|within| Path::new(importer).starts_with(within));
    let installed = |specifier: &str| {
        (!installs.split.contains(specifier))
            .then(|| installed_artifact_identity(directory, specifier))
            .flatten()
    };
    // Read once per install directory, not once per acceptance: the tier
    // offers many acceptances of one package, and each would otherwise hash
    // its files and re-read every lockfile above it again.
    let snapshots = InstalledSnapshots::default();
    let patches = crate::installed_patches::InstalledPatches::read(directory);
    let bytes =
        |specifier: &str| installed_artifact_bytes(directory, specifier, &patches, &snapshots);
    let resolved_target =
        |specifier: &str| resolved_target_identity(directory, facts, specifier, &counted);
    let environment = |specifier: &str, environment: &[crate::DependencyEnvironmentEntry]| {
        installed_environment_matches_in(directory, specifier, environment, &patches)
    };
    let mut admissions = ArtifactAdmissions {
        project_wide: tier(&installed, &bytes, &resolved_target, &environment).map_err(contract)?,
        installs: Vec::new(),
    };
    for (base, specifiers) in &installs.contexts {
        let installed = |specifier: &str| {
            specifiers
                .contains_key(specifier)
                .then(|| installed_artifact_identity(base, specifier))
                .flatten()
        };
        let patches = crate::installed_patches::InstalledPatches::read(base);
        let bytes =
            |specifier: &str| installed_artifact_bytes(base, specifier, &patches, &snapshots);
        let resolved_target = |specifier: &str| {
            let importers = specifiers.get(specifier)?;
            resolved_target_identity(base, facts, specifier, &|importer| {
                importers.contains(importer)
            })
        };
        let environment = |specifier: &str, environment: &[crate::DependencyEnvironmentEntry]| {
            installed_environment_matches_in(base, specifier, environment, &patches)
        };
        let admitted =
            tier(&installed, &bytes, &resolved_target, &environment).map_err(contract)?;
        if !admitted.is_empty() {
            admissions.installs.push((admitted, specifiers.clone()));
        }
    }
    Ok(admissions)
}

/// The project catalogs one analysis may read, and the discovered ones it
/// withheld because nothing was configured to authenticate them.
#[derive(Clone, Debug, Default)]
pub struct ProjectCatalogSelection {
    /// The catalogs to read, in discovery order.
    pub admitted: Vec<PathBuf>,
    /// Discovered catalogs holding policy-2 entries, withheld because no
    /// `--receipt-trust-configuration` was supplied -- the project's own and
    /// every nested one.
    pub unauthenticated: Vec<UnauthenticatedCatalog>,
    /// The catalogs of directories inside the project, each applying to that
    /// directory's files only. Always empty for an explicit
    /// `--accepted-contracts`, which names the whole local tier.
    pub nested: Vec<NestedCatalogs>,
}

/// The catalogs found in one directory strictly inside the analysed project:
/// its `.solid-checker/`, read for the analysed files below that directory.
///
/// A file's applicable project catalogs are those of its ancestor directories
/// up to and including the analysed project's: a monorepo's root
/// `tsconfig.json` analyses `packages/core/src/**` exactly as
/// `packages/core/tsconfig.json` does, and until this it never saw
/// `packages/core/.solid-checker/` (measured on kobalte:
/// docs/package-contract-v2/phase22/2026-09-26-project-side-certification-on-kobalte-core.md,
/// defect 5). The nearest catalog answers per specifier and a farther one is
/// the fallback, so the project's own catalog -- the farthest -- answers what
/// no nested one does, and a project with no nested catalog is analysed
/// exactly as before.
///
/// Nothing is trusted for being found: a nested catalog's acceptances go
/// through the same authentication and the same admission as the project's,
/// replayed from this directory.
#[derive(Clone, Debug)]
pub struct NestedCatalogs {
    pub directory: PathBuf,
    /// The catalogs to read, in discovery order.
    pub admitted: Vec<PathBuf>,
}

/// A discovered project catalog that was not read for want of trust.
#[derive(Clone, Debug)]
pub struct UnauthenticatedCatalog {
    pub path: PathBuf,
    /// The packages its policy-2 entries accept.
    pub packages: Vec<String>,
}

impl UnauthenticatedCatalog {
    /// The sentence `contract check` appends to such a package's status.
    fn refusal(&self) -> String {
        format!(
            "a project catalog entry exists for this package and was not read: {} needs --receipt-trust-configuration <trust.json> to authenticate its policy-2 receipt",
            self.path.display()
        )
    }
}

impl ProjectCatalogSelection {
    /// Whether any catalog was found or named at all, admitted or not.
    pub fn is_empty(&self) -> bool {
        self.admitted.is_empty() && self.unauthenticated.is_empty() && self.nested.is_empty()
    }

    /// The one non-fatal notice an analysis prints when it withheld catalogs,
    /// naming every withheld path, the packages they accept, and the remedy.
    pub fn notice(&self) -> Option<String> {
        if self.unauthenticated.is_empty() {
            return None;
        }
        let paths = self
            .unauthenticated
            .iter()
            .map(|catalog| catalog.path.display().to_string())
            .collect::<Vec<_>>();
        let mut packages = self
            .unauthenticated
            .iter()
            .flat_map(|catalog| catalog.packages.iter().cloned())
            .collect::<Vec<_>>();
        packages.sort();
        packages.dedup();
        let (noun, verb) = if paths.len() == 1 {
            ("catalog", "was")
        } else {
            ("catalogs", "were")
        };
        Some(format!(
            "solid-checker: note: project {noun} {} {verb} not read: policy-2 receipts need a trusted issuer and no trust configuration was supplied, so the contracts for {} are not admitted and the analysis proceeds as if the {noun} {verb} absent; pass --receipt-trust-configuration <trust.json> (the file `contract certify --trust-configuration-output` wrote) to admit them",
            paths.join(", "),
            packages.join(", ")
        ))
    }

    /// `contract check`'s explanation for each package a withheld catalog
    /// accepts. It replaces a compiled-in tier's refusal for the same package:
    /// project catalogs take precedence over that tier, so the withheld one is
    /// what would have answered.
    pub fn extend_refusals(&self, refusals: &mut BTreeMap<String, String>) {
        for catalog in &self.unauthenticated {
            for package in &catalog.packages {
                refusals.insert(package.clone(), catalog.refusal());
            }
        }
    }
}

/// Selects the project catalogs an analysis reads.
///
/// `explicit` is `--accepted-contracts`; empty means discovery under
/// `directory`. `trust_supplied` is whether `--receipt-trust-configuration`
/// was given; the configuration itself is read and validated by the caller,
/// so a named but missing or undecodable trust file still fails there.
///
/// A policy-2 entry can only be admitted by authenticating its receipt against
/// separately configured trust, and a project cannot nominate its own issuer.
/// So a discovered catalog holding one is unreadable without that
/// configuration, and it used to take the whole analysis down with it: exit 2
/// and no findings at all, on every run after the first `contract certify`
/// that forgot `--receipt-trust-configuration`. Measured on `@kobalte/core`
/// (docs/package-contract-v2/phase22/2026-09-26-project-side-certification-on-kobalte-core.md).
///
/// Such a catalog is now withheld whole, exactly as if it were absent: every
/// import it would have covered falls back to the compiled-in tier or to the
/// acceptance-gate obligation, and the caller reports
/// [`ProjectCatalogSelection::notice`]. Nothing is admitted on the way, so this
/// can only lose acceptances, never add one.
///
/// A catalog the user *named* is different. `--accepted-contracts <path>` is a
/// request for that catalog's contracts, and answering it silently without
/// them would report a run that did not do what was asked; it stays a hard
/// error, now naming the path and the remedy.
pub fn select_project_catalogs(
    directory: &Path,
    explicit: &str,
    trust_supplied: bool,
) -> Result<ProjectCatalogSelection, BackendError> {
    let contract = |error: crate::ContractFailure| BackendError::Contract(error.to_string());
    if !explicit.is_empty() {
        let path = PathBuf::from(explicit);
        if !trust_supplied {
            let packages = crate::contract_interface::catalog_packages_needing_receipt_trust(&path)
                .map_err(contract)?;
            if !packages.is_empty() {
                return Err(BackendError::Contract(format!(
                    "--accepted-contracts {}: {}; pass --receipt-trust-configuration <trust.json> naming the issuer that certified it",
                    path.display(),
                    crate::ContractFailure::ReceiptAuthenticationRequired
                )));
            }
        }
        return Ok(ProjectCatalogSelection {
            admitted: vec![path],
            unauthenticated: Vec::new(),
            nested: Vec::new(),
        });
    }
    let mut selection = ProjectCatalogSelection::default();
    selection.admitted = selected_in(directory, trust_supplied, &mut selection.unauthenticated)?;
    Ok(selection)
}

/// [`select_project_catalogs`], and the catalogs of every directory in
/// `candidates` -- from [`nested_catalog_candidates`] -- each selected by the
/// same trust rule. A withheld nested catalog is named in the one notice with
/// the project's own. An explicit `--accepted-contracts` still names the whole
/// local tier: no nested catalog is read beside it.
pub fn select_project_catalogs_in(
    directory: &Path,
    candidates: &[PathBuf],
    explicit: &str,
    trust_supplied: bool,
) -> Result<ProjectCatalogSelection, BackendError> {
    let mut selection = select_project_catalogs(directory, explicit, trust_supplied)?;
    if !explicit.is_empty() {
        return Ok(selection);
    }
    for candidate in candidates {
        if !candidate.join(".solid-checker").is_dir() {
            continue;
        }
        let admitted = selected_in(candidate, trust_supplied, &mut selection.unauthenticated)?;
        if !admitted.is_empty() {
            selection.nested.push(NestedCatalogs {
                directory: candidate.clone(),
                admitted,
            });
        }
    }
    Ok(selection)
}

/// The catalogs discovered in `directory`'s `.solid-checker/` that this run can
/// read, pushing each one it cannot onto `withheld`.
fn selected_in(
    directory: &Path,
    trust_supplied: bool,
    withheld: &mut Vec<UnauthenticatedCatalog>,
) -> Result<Vec<PathBuf>, BackendError> {
    let contract = |error: crate::ContractFailure| BackendError::Contract(error.to_string());
    let discovered =
        crate::contract_interface::discovered_catalog_paths(directory).map_err(contract)?;
    if trust_supplied {
        return Ok(discovered);
    }
    let mut admitted = Vec::new();
    for path in discovered {
        let packages = crate::contract_interface::catalog_packages_needing_receipt_trust(&path)
            .map_err(contract)?;
        if packages.is_empty() {
            admitted.push(path);
        } else {
            withheld.push(UnauthenticatedCatalog { path, packages });
        }
    }
    Ok(admitted)
}

/// The directories whose `.solid-checker/` may hold a catalog for one of
/// `files`: every ancestor directory of an analysed file strictly inside
/// `project_directory`, sorted, whether or not it holds a catalog. The project
/// directory itself is not one -- its catalog is the project's own -- and
/// neither is a directory at or below a `node_modules`, which holds installed
/// packages rather than this project's sources. A file outside the project
/// directory contributes nothing, so the project's own catalog is the only one
/// that applies to it.
///
/// Every candidate is returned, not only the ones that hold a catalog today: a
/// host that caches an answer has to treat a catalog appearing in any of them
/// as a change.
#[must_use]
pub fn nested_catalog_candidates<'a>(
    project_directory: &Path,
    files: impl IntoIterator<Item = &'a str>,
) -> Vec<PathBuf> {
    // The project directory as the host spelled it, and as an absolute and a
    // real path: file paths come from the compiler, which may have resolved
    // either. A candidate keeps the file's own spelling, because that is what
    // an importer is compared with.
    let mut spellings = vec![project_directory.to_path_buf()];
    spellings.extend(std::path::absolute(project_directory).ok());
    spellings.extend(fs::canonicalize(project_directory).ok());
    let mut candidates = std::collections::BTreeSet::new();
    for file in files {
        let Some(parent) = Path::new(file).parent() else {
            continue;
        };
        let Some(project) = spellings
            .iter()
            .find(|spelling| parent.starts_with(spelling))
        else {
            continue;
        };
        for ancestor in parent.ancestors() {
            if ancestor == project {
                break;
            }
            let Ok(relative) = ancestor.strip_prefix(project) else {
                break;
            };
            if relative
                .components()
                .any(|component| component.as_os_str() == "node_modules")
            {
                continue;
            }
            if !candidates.insert(ancestor.to_path_buf()) {
                // Every ancestor above this one was inserted with it.
                break;
            }
        }
    }
    candidates.into_iter().collect()
}

/// The accepted-contract index ordinary analysis reads, from every tier this
/// project can reach.
///
/// One function because the *order* is the rule, and three callers had to agree
/// on it: the analysis, `contract check`, and the daemon. Project catalogs
/// first, the compiled-in tier below them, and the missing-evidence markers
/// last -- a project that certified a package itself keeps its own answer, a
/// package this build carries a contract for stops raising an obligation the
/// user cannot discharge, and everything else still raises one. Artifact
/// admission runs project-first for the same reason.
///
/// Callers still resolve `catalogs` and `trust` themselves, because how a
/// catalog is *selected* genuinely differs between them (an explicit `--catalog`
/// overrides discovery; the emission path supplies neither). What must not
/// differ is what happens afterwards. `nested` is the selection's
/// [`NestedCatalogs`], each folded in above every project-wide tier for its
/// own directory's files.
#[allow(clippy::too_many_arguments)]
pub fn project_accepted_contracts(
    directory: &Path,
    catalogs: &[PathBuf],
    nested: &[NestedCatalogs],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    bundled: bool,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
    requirements: AcceptedContractIndex,
) -> Result<AcceptedContractIndex, BackendError> {
    let mut contracts = read_catalogs(catalogs, trust)?;
    if bundled {
        contracts = contracts.with_fallback(
            crate::accepted_bundles::compiled_in_accepted_contracts()
                .map_err(|error| BackendError::Contract(error.to_string()))?,
        );
    }
    let mut contracts = contracts.with_fallback(requirements);
    // An acceptance is issued for the file that imported the package during
    // certification. Admit the specifier project-wide when *this* project's
    // installed artifact is the one that acceptance names -- same integrity,
    // entrypoint and declared conditions. With no declared conditions this
    // admits nothing, because conditions select the artifact and the analyzer
    // has no facts of its own about them.
    //
    // One call per tier, in precedence order. `with_admitted_artifacts` keeps
    // the first tier to claim a specifier, and it requires the identities
    // *within* one call to agree -- which a project's own catalog and a
    // contract compiled into this build have no reason to do, and no reason to
    // be asked to.
    //
    // "This project's installed artifact" is the one each importer's own
    // resolution reaches ([`ImporterInstalls`]); both tiers are asked from the
    // same installs.
    let installs = importer_installs(directory, facts, None)?;
    let project = admitted_catalog_artifacts(
        catalogs, trust, directory, conditions, facts, None, &installs,
    )?
    .agreed(&contracts);
    contracts = project.admit_into(contracts);
    if bundled {
        let bundles =
            bundled_admissions(directory, conditions, facts, &installs)?.agreed(&contracts);
        contracts = bundles.admit_into(contracts);
    }
    // Why a project catalog's acceptance was not admitted, for the acceptance
    // gate to say at the imports it leaves unanswered and for the package
    // summary to report instead of `accepted`. These are the sentences
    // `contract check` appends to a `missing` status, from the same replay of
    // admission steps 1-3, for the project tier only: a compiled-in contract is
    // not something this project asked for, and its refusals are `contract
    // check`'s to explain. Explanation only -- nothing here binds or withholds.
    if !catalogs.is_empty() {
        contracts = refusal_notes(directory, catalogs, facts, contracts, None, &installs)?;
    }
    // A catalog in a directory inside the project applies to that directory's
    // files, above every project-wide tier; see [`NestedCatalogs`]. Each is its
    // own tier with its own admission, replayed from its own directory: that is
    // where Node starts looking for the package when one of those files
    // imports it, so it is the installed tree the acceptance has to reproduce.
    for scope in nested {
        contracts = contracts.with_scoped(
            scope.directory.to_string_lossy().into_owned(),
            scoped_accepted_contracts(scope, trust, conditions, facts)?,
        );
    }
    Ok(contracts)
}

/// Every catalog read and folded in the one order the project tier uses.
fn read_catalogs(
    catalogs: &[PathBuf],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
) -> Result<AcceptedContractIndex, BackendError> {
    let mut contracts = AcceptedContractIndex::default();
    for path in catalogs {
        contracts =
            crate::contract_interface::read_external_contract_catalog_with_trust(path, trust)
                .map_err(|error| BackendError::Contract(error.to_string()))?
                .with_fallback(contracts);
    }
    Ok(contracts)
}

/// One nested directory's tier: its catalogs, what they admit by artifact for
/// the files below it, and why they refused the rest. It holds no compiled-in
/// contract and no missing-evidence marker -- those are project-wide, and the
/// index consults them after every scope.
fn scoped_accepted_contracts(
    scope: &NestedCatalogs,
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
) -> Result<AcceptedContractIndex, BackendError> {
    let directory = scope.directory.as_path();
    let mut contracts = read_catalogs(&scope.admitted, trust)?;
    let installs = importer_installs(directory, facts, Some(directory))?;
    let admitted = admitted_catalog_artifacts(
        &scope.admitted,
        trust,
        directory,
        conditions,
        facts,
        Some(directory),
        &installs,
    )?
    .agreed(&contracts);
    contracts = admitted.admit_into(contracts);
    refusal_notes(
        directory,
        &scope.admitted,
        facts,
        contracts,
        Some(directory),
        &installs,
    )
}

/// Adds to `contracts` the refusal sentence for each specifier the files
/// `within` (every file when `None`) import or re-export, or that `contracts`
/// binds by importer, whose package has an acceptance in `catalogs` that steps
/// 1-3 of admission refused from `directory`.
///
/// A split specifier ([`ImporterInstalls`]) is also explained per importer,
/// from the install that importer reaches, and a context whose replay refused
/// nothing says so -- so the project directory's sentence, which is about
/// another copy, is never the one an importer elsewhere is told.
fn refusal_notes(
    directory: &Path,
    catalogs: &[PathBuf],
    facts: &solid_facts::ProjectFacts,
    contracts: AcceptedContractIndex,
    within: Option<&Path>,
    installs: &ImporterInstalls,
) -> Result<AcceptedContractIndex, BackendError> {
    let refusals = admission_refusal_details(directory, catalogs, false)?;
    let mut by_importer = Vec::new();
    for (base, specifiers) in &installs.contexts {
        let replayed;
        let refusals = if base == directory {
            &refusals
        } else {
            replayed = admission_refusal_details(base, catalogs, false)?;
            &replayed
        };
        for (specifier, importers) in specifiers {
            let refusal = package_name_of_specifier(specifier)
                .and_then(|module| refusals.get(&module))
                .cloned();
            for importer in importers {
                by_importer.push(((importer.clone(), specifier.clone()), refusal.clone()));
            }
        }
    }
    // Nothing to say anywhere: the index is left exactly as it was.
    if by_importer.iter().all(|(_, refusal)| refusal.is_none()) {
        by_importer.clear();
    }
    let mut contracts = contracts;
    if !by_importer.is_empty() {
        contracts = contracts.with_admission_refusals_for(by_importer);
    }
    if refusals.is_empty() {
        return Ok(contracts);
    }
    let specifiers = facts
        .files
        .iter()
        .filter(|file| {
            within.is_none_or(|within| Path::new(file.path.as_str()).starts_with(within))
        })
        .flat_map(|file| {
            file.ast
                .imports
                .iter()
                .map(|import| import.module.as_str())
                .chain(
                    file.ast
                        .exports
                        .iter()
                        .filter_map(|export| export.module.as_deref()),
                )
        })
        .chain(
            contracts
                .semantic_identity()
                .iter()
                .map(|binding| binding.specifier.as_str()),
        )
        .collect::<std::collections::BTreeSet<_>>();
    let notes = specifiers
        .into_iter()
        .filter_map(|specifier| {
            let refusal = refusals.get(&package_name_of_specifier(specifier)?)?;
            Some((specifier.to_owned(), refusal.clone()))
        })
        .collect::<Vec<_>>();
    Ok(if notes.is_empty() {
        contracts
    } else {
        contracts.with_admission_refusals(notes)
    })
}

/// Keeps one acceptance per specifier, and only where every candidate for it
/// claims the same thing.
///
/// `admissible_cases` narrows to a single case whenever the host declared its
/// export conditions. It cannot when the host declared none — the ESLint and
/// Oxlint case — and then it hands over every case that reaches the file this
/// project resolved. That used to be decided by admitting when all candidates
/// named the same *runtime file*, on the ground that they therefore describe
/// the same bytes; a contract describes an entry file's export surface while
/// its semantics depend on the whole module closure, and conditions select that
/// closure, so agreeing on a path is not agreeing on a claim.
///
/// The comparison is the document's own content address for each export's
/// claims ([`contract_document::export_claims_address`]), because the in-memory
/// form cannot be compared directly: decoding qualifies every operation id with
/// the artifact-case id, so two contracts that agree completely still differ in
/// every `OperationId`. Measured on `@kobalte/utils@0.9.2`, whose two `.` cases
/// -- `["import"]` and `["import","solid"]` -- differ in exactly that and in
/// nothing else, for 13 of its 59 exports.
///
/// A candidate whose address cannot be computed answers nothing, so the
/// specifier is dropped: this must add acceptances on proof, never on the
/// absence of a comparison.
fn agreed_admissions(
    contracts: &AcceptedContractIndex,
    candidates: Vec<(String, String)>,
) -> Vec<(String, String)> {
    let claims = |identity: &str| -> Option<BTreeMap<String, String>> {
        let contract = contracts.contract_for_artifact(identity)?;
        let case = contract.artifact_case();
        case.exports
            .iter()
            .map(|(name, export)| {
                crate::contract_document::export_claims_address(case, name, export)
                    .ok()
                    .map(|address| (name.clone(), address))
            })
            .collect()
    };
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for (specifier, identity) in candidates {
        match grouped.iter_mut().find(|(name, _)| *name == specifier) {
            Some((_, identities)) => identities.push(identity),
            None => grouped.push((specifier, vec![identity])),
        }
    }
    let mut admitted = Vec::new();
    for (specifier, identities) in grouped {
        let [first, rest @ ..] = identities.as_slice() else {
            continue;
        };
        if rest.is_empty() {
            // One candidate needs no comparison, but it still has to be an
            // acceptance this index can serve, so that what comes back is
            // exactly what will be admitted.
            if contracts.contract_for_artifact(first).is_some() {
                admitted.push((specifier, first.clone()));
            }
            continue;
        }
        let Some(expected) = claims(first) else {
            continue;
        };
        if rest
            .iter()
            .all(|identity| claims(identity).is_some_and(|other| other == expected))
        {
            admitted.push((specifier, first.clone()));
        }
    }
    admitted
}

/// Every lockfile [`project_accepted_contracts`] can consult when it decides
/// whether an acceptance applies here.
///
/// Artifact admission recomputes the acceptance root from the *installed*
/// tarball integrity, and the integrity comes from whichever lockfile the
/// project's package manager wrote. A host that caches an answer across runs
/// has to treat these as inputs, or an install that repacks a dependency at the
/// same version keeps serving the previous verdict. Paths are returned whether
/// or not they exist, so that a lockfile appearing is a change too.
#[must_use]
pub fn admission_input_paths(project_directory: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for ancestor in project_directory.ancestors() {
        paths.push(ancestor.join("package-lock.json"));
        paths.push(ancestor.join("node_modules").join(".package-lock.json"));
        paths.push(ancestor.join("bun.lock"));
        paths.push(ancestor.join("pnpm-lock.yaml"));
        paths.push(ancestor.join("yarn.lock"));
        // Where a patch is declared (ADR 0131): pnpm and Bun declare theirs
        // here, and a manifest's scripts name `patch-package`'s directory.
        paths.push(ancestor.join("package.json"));
        paths.push(ancestor.join("pnpm-workspace.yaml"));
    }
    // And the patch files themselves, which no lockfile mentions. A patch
    // file added or removed changes this list, which is a change too.
    paths.extend(crate::installed_patches::patch_input_paths(
        project_directory,
    ));
    paths
}

/// The specifiers this project may import under a contract compiled into the
/// checker.
///
/// The same two installed-tree facts answer both tiers, because the question is
/// the same one: did *this* project resolve the artifact that acceptance was
/// proven about. Only the source of the acceptances differs.
pub fn admitted_bundled_artifacts(
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
) -> Result<ArtifactAdmissions, BackendError> {
    let installs = importer_installs(project_directory, facts, None)?;
    bundled_admissions(project_directory, conditions, facts, &installs)
}

/// [`admitted_bundled_artifacts`] over installs already grouped.
fn bundled_admissions(
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
    installs: &ImporterInstalls,
) -> Result<ArtifactAdmissions, BackendError> {
    admitted_by_install(
        project_directory,
        facts,
        None,
        installs,
        &|installed, bytes, resolved_target, environment| {
            crate::accepted_bundles::admitted_bundle_artifacts(
                conditions,
                installed,
                bytes,
                resolved_target,
                environment,
            )
        },
    )
}

/// Whether the catalog `contract certify` just published under `catalog_root`
/// is admitted in the tree it was certified in: steps 1-3 of the one admission
/// rule ([`crate::accepted_bundles::admission_refusals`]) for every entry of
/// `package_name`, one `(specifier, refusal)` per entry, `None` when admitted.
///
/// A certification that its own tree would refuse is a certification no
/// project can use, and until this check it exited 0 without a word: measured,
/// `vite-plugin-solid@3.0.0-next.5` certified in kobalte core and was refused in
/// the same tree. Asking the admission rule itself, rather than re-deriving
/// what it would say, is what keeps the two from drifting apart again.
///
/// The tree is the catalog's project -- the parent of a `.solid-checker/`
/// catalog root -- when that project installs the package, and otherwise the
/// package's own installed location, from which Node finds the package itself.
/// Step 4 (the file a project resolved, and case selection) needs a project's
/// resolved imports, which a certification has none of; it is not replayed.
///
/// `issued` names the receipts this certification issued, by their signed
/// `(artifactAcceptanceRoot, dependencyEnvironmentRoot)`: a catalog merges, and
/// an entry an earlier certification left for the same package is not this
/// certification's to answer for.
pub fn certified_catalog_self_admission(
    catalog_root: &Path,
    package_name: &str,
    package_root: &Path,
    issued: &std::collections::BTreeSet<(String, String)>,
) -> Result<Vec<(String, Option<crate::AdmissionRefusal>)>, BackendError> {
    let catalog_project = catalog_root
        .file_name()
        .is_some_and(|name| name == ".solid-checker")
        .then(|| catalog_root.parent())
        .flatten();
    let project = catalog_project
        .filter(|project| installed_artifact_identity(project, package_name).is_some())
        .unwrap_or(package_root);
    let patches = crate::installed_patches::InstalledPatches::read(project);
    let snapshots = InstalledSnapshots::default();
    let installed = |specifier: &str| installed_artifact_identity(project, specifier);
    let bytes =
        |specifier: &str| installed_artifact_bytes(project, specifier, &patches, &snapshots);
    let difference = |specifier: &str, environment: &[crate::DependencyEnvironmentEntry]| {
        installed_environment_difference(project, specifier, environment, &patches)
    };
    let catalogs = crate::contract_interface::catalog_paths_in(catalog_root)
        .map_err(|error| BackendError::Contract(error.to_string()))?;
    Ok(crate::contract_interface::project_admission_refusals_where(
        &catalogs,
        &installed,
        &bytes,
        &difference,
        |bindings| {
            issued.contains(&(
                bindings.artifact_acceptance_root.clone(),
                bindings.dependency_environment_root.clone(),
            ))
        },
    )
    .map_err(|error| BackendError::Contract(error.to_string()))?
    .into_iter()
    .filter(|(specifier, _)| package_name_of_specifier(specifier).as_deref() == Some(package_name))
    .collect())
}

/// One entry of a cited environment (its edge removed), the real paths it was
/// found at, and the lookups that found it: `(importer's installed real path,
/// bare name)`.
pub(crate) type LocatedCitedEntry = (
    crate::DependencyEnvironmentEntry,
    Vec<PathBuf>,
    Vec<(PathBuf, String)>,
);

/// One compiled-in acceptance a certification cites (ADR 0151), with where
/// its dependency and every package of its environment are installed in the
/// dependent's tree.
pub(crate) struct LocatedCitation {
    pub(crate) cited: crate::accepted_bundles::CompiledInCitation,
    /// The dependency itself, as installed: name, manifest version, lockfile
    /// integrity, no edge.
    pub(crate) installed: crate::DependencyEnvironmentEntry,
    /// The dependency's installed real path.
    pub(crate) root: PathBuf,
    /// Each environment entry of the cited receipt (its edge removed), the
    /// real paths it was found at, and the lookups that found it:
    /// `(importer's installed real path, bare name)`. An environment the tier
    /// states without edges records no lookups, and is then carried without
    /// edges.
    pub(crate) located: Vec<LocatedCitedEntry>,
}

/// The compiled-in acceptance the package installed at `dependent_root` may
/// cite for `query` (ADR 0151), replayed against that package's own tree, or
/// why it may not.
///
/// The tree is the dependent's installed location, from which Node finds the
/// dependency exactly as the dependent's own imports do; the admission steps
/// are the ones `contract check` and self-admission replay
/// ([`crate::accepted_bundles::admission_refusals`]), so a citation is
/// admitted here exactly when a project importing the dependency from this
/// location would admit the same acceptance.
pub(crate) fn compiled_in_citation(
    dependent_root: &Path,
    query: &crate::accepted_bundles::CitationQuery<'_>,
) -> Result<LocatedCitation, String> {
    let project = fs::canonicalize(dependent_root).map_err(|error| {
        format!(
            "the dependent's installed root {} cannot be read: {error}",
            dependent_root.display()
        )
    })?;
    let patches = crate::installed_patches::InstalledPatches::read(&project);
    let snapshots = InstalledSnapshots::default();
    let installed = |specifier: &str| installed_artifact_identity(&project, specifier);
    let bytes =
        |specifier: &str| installed_artifact_bytes(&project, specifier, &patches, &snapshots);
    let difference = |specifier: &str, environment: &[crate::DependencyEnvironmentEntry]| {
        installed_environment_difference(&project, specifier, environment, &patches)
    };
    let cited = crate::accepted_bundles::cite_compiled_in(query, &installed, &bytes, &difference)
        .map_err(|refusal| refusal.to_string())?;
    let unlocatable = || {
        format!(
            "{} is not installed where this tree can say",
            query.specifier
        )
    };
    let module = package_name_of_specifier(query.specifier).ok_or_else(unlocatable)?;
    let directory = discover_package_directory(&project, &module)
        .ok()
        .flatten()
        .ok_or_else(unlocatable)?;
    let root = fs::canonicalize(&directory).map_err(|_| unlocatable())?;
    let installed = installed_environment_identity(&project, &root).ok_or_else(unlocatable)?;
    let located = locate_cited_environment(&project, &root, &cited.environment);
    Ok(LocatedCitation {
        cited,
        installed,
        root,
        located,
    })
}

/// One answer of [`compiled_in_citation_candidates`]: the acceptance a
/// certification of the package would cite for one specifier, or why none.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationCandidate {
    pub specifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_contract_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// For each specifier the package installed at `dependent_root` imports, the
/// compiled-in acceptance a certification under `conditions` would cite
/// (ADR 0151), or why none.
///
/// The generation adapter's question, answered by the same function
/// certification uses, so the edge it writes into a proposal is the edge
/// certification can discharge. An answer is a hint and never authority:
/// certification replays the whole citation from the tree itself.
pub fn compiled_in_citation_candidates(
    dependent_root: &Path,
    conditions: &[String],
    specifiers: &[String],
) -> Vec<CitationCandidate> {
    specifiers
        .iter()
        .map(|specifier| {
            let query = crate::accepted_bundles::CitationQuery {
                specifier,
                export_conditions: conditions,
                artifact_case: None,
                accepted_contract_digest: None,
            };
            match compiled_in_citation(dependent_root, &query) {
                Ok(located) => CitationCandidate {
                    specifier: specifier.clone(),
                    package_name: Some(located.cited.citation.package_name.clone()),
                    package_version: Some(located.cited.citation.package_version.clone()),
                    artifact_case: located
                        .cited
                        .contract
                        .artifact_cases()
                        .first()
                        .map(|case| case.id.clone()),
                    accepted_contract_digest: Some(
                        located.cited.contract.semantic_digest().as_str().to_owned(),
                    ),
                    receipt_digest: Some(located.cited.citation.receipt_digest.clone()),
                    refusal: None,
                },
                Err(refusal) => CitationCandidate {
                    specifier: specifier.clone(),
                    package_name: None,
                    package_version: None,
                    artifact_case: None,
                    accepted_contract_digest: None,
                    receipt_digest: None,
                    refusal: Some(refusal),
                },
            }
        })
        .collect()
}

/// Where each entry of a cited environment is installed, found by replaying
/// its recorded lookup from every location its importer was found at, the
/// dependency itself standing for `certified`.
///
/// Admission has already replayed the same lookups and compared every
/// identity, so this only recovers the locations, which the dependent's own
/// environment needs to state edges of its own. An environment stated without
/// edges records none; its entries are located from the dependency and from
/// each other, by the strict rule's own search, and carried without lookups.
fn locate_cited_environment(
    project: &Path,
    dependency_root: &Path,
    environment: &[crate::DependencyEnvironmentEntry],
) -> Vec<LocatedCitedEntry> {
    use crate::contract_certification::EnvironmentImporter;
    let identity_at = |at: &Path| installed_environment_identity(project, at);
    let mut located: Vec<LocatedCitedEntry> = Vec::new();
    let locations_of =
        |located: &[LocatedCitedEntry], importer: &EnvironmentImporter| -> Vec<PathBuf> {
            match importer {
                EnvironmentImporter::Certified => vec![dependency_root.to_path_buf()],
                EnvironmentImporter::Package(package) => located
                    .iter()
                    .filter(|(entry, _, _)| package.is(entry))
                    .flat_map(|(_, roots, _)| roots.iter().cloned())
                    .collect(),
            }
        };
    if crate::contract_certification::dependency_environment_states_edges(environment) {
        // Edges are rooted, so repeated passes reach every entry whose
        // importer is reachable; the bound is the entry count.
        for _ in 0..=environment.len() {
            let mut moved = false;
            for entry in environment {
                let Some(edge) = entry.resolved_from.as_ref() else {
                    continue;
                };
                for from in locations_of(&located, &edge.importer) {
                    let Ok(Some(at)) = node_package_lookup(&from, &edge.specifier) else {
                        continue;
                    };
                    if !identity_at(&at).is_some_and(|found| found.same_package(entry)) {
                        continue;
                    }
                    let bare = entry.without_edge();
                    let lookup = (from.clone(), edge.specifier.clone());
                    match located
                        .iter_mut()
                        .find(|(known, _, _)| known.same_package(&bare))
                    {
                        Some((_, roots, lookups)) => {
                            if !roots.contains(&at) {
                                roots.push(at.clone());
                                moved = true;
                            }
                            if !lookups.contains(&lookup) {
                                lookups.push(lookup);
                                moved = true;
                            }
                        }
                        None => {
                            located.push((bare, vec![at.clone()], vec![lookup]));
                            moved = true;
                        }
                    }
                }
            }
            if !moved {
                break;
            }
        }
    } else {
        let mut frontier = vec![dependency_root.to_path_buf()];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(from) = frontier.pop() {
            if !seen.insert(from.clone()) {
                continue;
            }
            for entry in environment {
                let Ok(Some(at)) = node_package_lookup(&from, &entry.name) else {
                    continue;
                };
                if !identity_at(&at).is_some_and(|found| found.same_package(entry)) {
                    continue;
                }
                if !located
                    .iter()
                    .any(|(known, _, _)| known.same_package(entry))
                {
                    located.push((entry.without_edge(), vec![at.clone()], Vec::new()));
                }
                frontier.push(at);
            }
        }
    }
    located
}

/// Whether this project's installed tree, resolved from the installed copy of
/// the package `specifier` names, is the dependency environment an acceptance
/// -- a compiled-in bundle or a project catalog entry -- was proven in.
///
/// The root is the directory [`installed_artifact_identity`] reads the
/// package's own identity from, so the environment is checked from exactly the
/// copy whose bytes were matched. Every lookup after that is Node's, from real
/// paths: a pnpm store sibling is found where Node finds it, and a hoisted
/// copy only where no nearer `node_modules` shadows it. Anything this cannot
/// state exactly -- a missing package, an unreadable manifest, a lockfile that
/// names no integrity or two -- answers `false`, so the acceptance is not
/// admitted.
#[cfg(test)]
pub(crate) fn installed_environment_matches(
    project_directory: &Path,
    specifier: &str,
    environment: &[crate::DependencyEnvironmentEntry],
) -> bool {
    installed_environment_matches_in(
        project_directory,
        specifier,
        environment,
        &crate::installed_patches::InstalledPatches::read(project_directory),
    )
}

/// [`installed_environment_matches`] with the tree's patch records already
/// read: an entry whose installed copy the tree records as patched is not the
/// certified one, however its lockfile integrity reads (ADR 0131).
fn installed_environment_matches_in(
    project_directory: &Path,
    specifier: &str,
    environment: &[crate::DependencyEnvironmentEntry],
    patches: &crate::installed_patches::InstalledPatches,
) -> bool {
    if environment.is_empty() {
        return true;
    }
    let Some(module) = package_name_of_specifier(specifier) else {
        return false;
    };
    let Ok(Some(directory)) = discover_package_directory(project_directory, &module) else {
        return false;
    };
    let (Ok(root), Ok(project)) = (
        fs::canonicalize(&directory),
        fs::canonicalize(project_directory),
    ) else {
        return false;
    };
    crate::accepted_bundles::environment_is_installed(
        environment,
        root,
        |from, name| node_package_lookup(from, name),
        |at| installed_environment_identity(&project, at),
        |at| installed_package_patch(patches, at),
    )
}

/// The first way this tree differs from `environment`, rendered, by the same
/// walk [`installed_environment_matches`] takes; `None` when it reproduces it.
/// Diagnostic only.
pub(crate) fn installed_environment_difference(
    project_directory: &Path,
    specifier: &str,
    environment: &[crate::DependencyEnvironmentEntry],
    patches: &crate::installed_patches::InstalledPatches,
) -> Option<String> {
    if environment.is_empty() {
        return None;
    }
    let unlocatable = || {
        Some(format!(
            "{specifier} is not installed where this tree can say"
        ))
    };
    let Some(module) = package_name_of_specifier(specifier) else {
        return unlocatable();
    };
    let Ok(Some(directory)) = discover_package_directory(project_directory, &module) else {
        return unlocatable();
    };
    let (Ok(root), Ok(project)) = (
        fs::canonicalize(&directory),
        fs::canonicalize(project_directory),
    ) else {
        return unlocatable();
    };
    crate::accepted_bundles::environment_difference(
        environment,
        root,
        |from, name| node_package_lookup(from, name),
        |at| installed_environment_identity(&project, at),
        |at| installed_package_patch(patches, at),
        |at| {
            serde_json::from_slice::<PackageManifest>(&fs::read(at.join("package.json")).ok()?)
                .ok()
                .map(|manifest| manifest.version)
                .filter(|version| !version.is_empty())
        },
    )
    .map(|difference| difference.to_string())
}

/// What the tree records about the package installed at `directory` being
/// patched, by its installed name and manifest version; `None` when nothing
/// does, or when the package states no identity (which refuses it anyway).
fn installed_package_patch(
    patches: &crate::installed_patches::InstalledPatches,
    directory: &Path,
) -> Option<String> {
    let name = installed_package_name(directory)?;
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(directory.join("package.json")).ok()?).ok()?;
    patches.of(directory, &name, &manifest.version)
}

/// Installed snapshot roots already computed in one admission, by canonical
/// package directory.
#[derive(Default)]
struct InstalledSnapshots(std::cell::RefCell<HashMap<PathBuf, Result<String, String>>>);

/// The [`crate::installed_package_snapshot_root`] of the package `specifier`
/// names, as installed for `project_directory`, or why its files cannot be
/// stated to be the published archive: a patch the tree records, or a member
/// no archive installs as (ADR 0131). The admission side of
/// [`crate::accepted_bundles::InstalledArtifactBytes`].
fn installed_artifact_bytes(
    project_directory: &Path,
    specifier: &str,
    patches: &crate::installed_patches::InstalledPatches,
    snapshots: &InstalledSnapshots,
) -> Result<String, String> {
    let unlocatable = || format!("{specifier} is not installed where this tree can say");
    let module = package_name_of_specifier(specifier).ok_or_else(unlocatable)?;
    let (directory, manifest) = installed_package_manifest(project_directory, &module)
        .ok()
        .flatten()
        .ok_or_else(unlocatable)?;
    let directory = fs::canonicalize(&directory).map_err(|_| unlocatable())?;
    if let Some(evidence) = patches.of(&directory, &module, &manifest.version) {
        return Err(format!(
            "{module}@{} is patched ({evidence})",
            manifest.version
        ));
    }
    snapshots
        .0
        .borrow_mut()
        .entry(directory)
        .or_insert_with_key(|directory| {
            crate::installed_package_snapshot_root(directory, &module, &manifest.version)
        })
        .clone()
}

/// [`installed_artifact_bytes`] with this tree's patch records read afresh.
#[cfg(test)]
pub(crate) fn installed_artifact_snapshot(
    project_directory: &Path,
    specifier: &str,
) -> Result<String, String> {
    installed_artifact_bytes(
        project_directory,
        specifier,
        &crate::installed_patches::InstalledPatches::read(project_directory),
        &InstalledSnapshots::default(),
    )
}

/// Node's lookup of the bare package `name` from the package installed at
/// `from` (a real path): the nearest `<ancestor>/node_modules/<name>`, skipping
/// ancestors that are themselves `node_modules` directories, as
/// `Module._nodeModulePaths` does.
///
/// A candidate Node would load that is not a package directory with a manifest
/// -- `<name>.js` beside it, or a directory with no `package.json` -- is not a
/// fact this can state, and answers `Err` rather than walking past what Node
/// would have stopped at.
fn node_package_lookup(from: &Path, name: &str) -> Result<Option<PathBuf>, ()> {
    for ancestor in from.ancestors() {
        if ancestor.file_name().is_some_and(|it| it == "node_modules") {
            continue;
        }
        let candidate = ancestor.join("node_modules").join(name);
        for extension in ["js", "json", "node"] {
            let mut file = candidate.clone().into_os_string();
            file.push(".");
            file.push(extension);
            match fs::symlink_metadata(PathBuf::from(file)) {
                Ok(_) => return Err(()),
                Err(error) if absent(&error) => {}
                Err(_) => return Err(()),
            }
        }
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() => {
                return match fs::metadata(candidate.join("package.json")) {
                    Ok(manifest) if manifest.is_file() => {
                        fs::canonicalize(&candidate).map(Some).map_err(|_| ())
                    }
                    _ => Err(()),
                };
            }
            Ok(_) => return Err(()),
            Err(error) if absent(&error) => {}
            Err(_) => return Err(()),
        }
    }
    Ok(None)
}

fn absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

/// The three facts an environment entry states, read from the package
/// installed at `directory` exactly as [`installed_artifact_identity`] reads
/// them for the imported package: the installed directory's package name, the
/// manifest version, and the lockfile integrity.
fn installed_environment_identity(
    project_directory: &Path,
    directory: &Path,
) -> Option<crate::DependencyEnvironmentEntry> {
    let name = installed_package_name(directory)?;
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(directory.join("package.json")).ok()?).ok()?;
    if manifest.version.is_empty() {
        return None;
    }
    let integrity = installed_package_integrity(project_directory, directory).ok()??;
    Some(crate::DependencyEnvironmentEntry::package(
        name,
        manifest.version,
        integrity,
    ))
}

pub(crate) fn installed_artifact_identity(
    project_directory: &Path,
    specifier: &str,
) -> Option<(String, String, String)> {
    let module = package_name_of_specifier(specifier)?;
    let (directory, manifest) = installed_package_manifest(project_directory, &module).ok()??;
    let integrity = installed_package_integrity(project_directory, &directory).ok()??;
    Some((module, manifest.version, integrity))
}

/// What this project resolved the specifier to, relative to the installed
/// package root -- the fact that lets an acceptance be bound to this project
/// without the host having to declare export conditions it usually does not
/// know. `None` whenever the project cannot state one exactly: an unresolved
/// import, a specifier no importer reached, or two importers that disagree
/// (a nested install), each of which admits nothing rather than picking.
///
/// Only the attested rows of the importers `counted` selects are read: a
/// nested catalog's own files, or the files whose resolution reaches the
/// install `project_directory` names ([`ImporterInstalls`]).
fn resolved_target_identity(
    project_directory: &Path,
    facts: &solid_facts::ProjectFacts,
    specifier: &str,
    counted: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let module = package_name_of_specifier(specifier)?;
    let (directory, _) = installed_package_manifest(project_directory, &module).ok()??;
    let root = fs::canonicalize(&directory).ok()?;
    let attested = facts.resolved_imports.as_ref()?;
    let mut selected: Option<String> = None;
    for (importer, import) in attested.iter() {
        if import.text.as_str() != specifier
            || import.resolution == solid_facts::ImportResolution::Unresolved
            || !counted(importer)
        {
            continue;
        }
        let resolved = fs::canonicalize(Path::new(import.resolved_path.as_ref())).ok()?;
        let relative = resolved
            .strip_prefix(&root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.is_empty() {
            return None;
        }
        match &selected {
            Some(existing) if existing != &relative => return None,
            Some(_) => {}
            None => selected = Some(relative),
        }
    }
    selected
}

fn package_name_of_specifier(specifier: &str) -> Option<String> {
    let mut parts = specifier.split('/');
    let first = parts.next()?;
    if first.starts_with('@') {
        let second = parts.next()?;
        return Some(format!("{first}/{second}"));
    }
    (!first.is_empty()).then(|| first.to_owned())
}

fn installed_package_manifest(
    project_directory: &Path,
    module: &str,
) -> Result<Option<(PathBuf, PackageManifest)>, BackendError> {
    let Some(directory) = discover_package_directory(project_directory, module)? else {
        return Ok(None);
    };
    read_installed_manifest(&directory)
}

/// An installed package directory with its parsed manifest, or `None` when the
/// directory has no `package.json`.
fn read_installed_manifest(
    directory: &Path,
) -> Result<Option<(PathBuf, PackageManifest)>, BackendError> {
    match fs::read(directory.join("package.json")) {
        Ok(data) => Ok(Some((
            directory.to_path_buf(),
            serde_json::from_slice(&data)?,
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// One entry of an npm lockfile's `packages` map.
///
/// Deliberately not `deny_unknown_fields`: a lockfile is written by npm, not
/// by this project, and every field beyond these two is irrelevant here.
#[derive(Deserialize)]
struct NpmLockfileEntry {
    /// Absent for a link, a workspace member, a `file:` dependency, and a git
    /// dependency — none of which have a registry tarball to hash. The absent
    /// case is not evidence of agreement; it is the absence of the fact.
    #[serde(default)]
    integrity: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NpmLockfile {
    #[serde(default)]
    lockfile_version: u32,
    /// The path-keyed installed tree, present from `lockfileVersion` 2 on.
    /// Version 1 has only the `dependencies` tree, whose keys are package
    /// names rather than install paths and therefore cannot identify *which*
    /// installed copy an entry describes under hoisting.
    #[serde(default)]
    packages: HashMap<String, NpmLockfileEntry>,
}

/// The package records Bun writes to `bun.lock`.
///
/// Bun's lockfile is JSON with trailing commas, and its `packages` map is
/// keyed by package name rather than install path. The installed package's
/// manifest version is therefore required to select the exact record; if
/// multiple records still disagree, integrity recovery fails closed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BunLockfile {
    #[serde(default)]
    lockfile_version: u32,
    #[serde(default)]
    packages: HashMap<String, Vec<serde_json::Value>>,
}

pub(crate) fn parse_json_with_trailing_commas<T: DeserializeOwned>(data: &[u8]) -> Option<T> {
    if let Ok(value) = serde_json::from_slice(data) {
        return Some(value);
    }

    let mut normalized = Vec::with_capacity(data.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0;
    while index < data.len() {
        let byte = data[index];
        if in_string {
            normalized.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            normalized.push(byte);
            index += 1;
            continue;
        }
        if byte == b',' {
            let mut next = index + 1;
            while next < data.len() && data[next].is_ascii_whitespace() {
                next += 1;
            }
            if next < data.len() && matches!(data[next], b'}' | b']') {
                index += 1;
                continue;
            }
        }
        normalized.push(byte);
        index += 1;
    }
    serde_json::from_slice(&normalized).ok()
}

fn installed_package_name(package_directory: &Path) -> Option<String> {
    let package = package_directory.file_name()?.to_str()?;
    let parent = package_directory.parent()?.file_name()?.to_str()?;
    if parent.starts_with('@') {
        Some(format!("{parent}/{package}"))
    } else {
        Some(package.to_owned())
    }
}

/// The integrity `bun.lock` records for the package installed at
/// `package_directory`, or `None` when it records none unambiguously.
///
/// Selection binds the exact installed identity, never the name alone: a
/// record is this package's only when its identifier is exactly the installed
/// directory's name at the installed manifest's version, and it is keyed at
/// that name -- hoisted (`name`), nested (`parent/name`), or by the identity
/// itself. Hoisting can record one name at several versions, so a key that
/// matches the name while its identifier names another version describes
/// another copy. Every selected record must carry an SRI integrity, and all of
/// them the same one.
fn bun_package_integrity(package_directory: &Path, data: &[u8]) -> Option<String> {
    let lockfile = parse_json_with_trailing_commas::<BunLockfile>(data)?;
    if !crate::contract_certification::BUN_LOCKFILE_VERSIONS.contains(&lockfile.lockfile_version) {
        return None;
    }
    let name = installed_package_name(package_directory)?;
    let version = serde_json::from_slice::<PackageManifest>(
        &fs::read(package_directory.join("package.json")).ok()?,
    )
    .ok()?
    .version;
    if version.is_empty() {
        return None;
    }
    let expected_identifier = format!("{name}@{version}");
    let nested = format!("/{name}");
    let mut found = None;
    for (key, record) in lockfile.packages {
        let identifier = record.first().and_then(serde_json::Value::as_str);
        if identifier != Some(expected_identifier.as_str())
            || !(key == name || key.ends_with(&nested) || key == expected_identifier)
        {
            continue;
        }
        let integrity = record.get(3).and_then(serde_json::Value::as_str)?;
        if !crate::contract_certification::is_sri_integrity(integrity) {
            return None;
        }
        match &found {
            None => found = Some(integrity.to_owned()),
            Some(existing) if existing != integrity => return None,
            Some(_) => {}
        }
    }
    found
}

/// The lockfile integrity for one installed package directory, or `None`
/// when no unambiguous integrity can be recovered.
///
/// The npm lockfile's `packages` map is keyed by install path relative to the
/// lockfile's own directory (`node_modules/foo`,
/// `node_modules/a/node_modules/foo`, `packages/app/node_modules/foo`), which
/// is what makes it usable at all: it names the *copy*, so a hoisted and a
/// nested install of the same package do not collide. That is also why
/// `lockfileVersion` 1 is skipped — its tree is keyed by package name, and
/// resolving a name to an install path would be the guess this must not make.
///
/// Every ambiguity resolves to `None` — no enforcement — rather than to a
/// verdict:
///
/// - two lockfiles that disagree about the same installed directory (which one
///   is authoritative is exactly the question this cannot answer);
/// - an entry with no `integrity` (a link, workspace member, `file:`, or git
///   dependency has no registry tarball);
/// - a lockfile this checker cannot parse, or one the package manager has not
///   written at all (pnpm and Yarn keep their own formats).
///
/// `None` therefore means "the installed integrity is not a fact this project
/// makes available", never "the integrities agree".
pub(crate) fn installed_package_integrity(
    project_directory: &Path,
    package_directory: &Path,
) -> Result<Option<String>, BackendError> {
    let mut found: Option<String> = None;
    for ancestor in project_directory.ancestors() {
        let Ok(relative) = package_directory.strip_prefix(ancestor) else {
            continue;
        };
        let key = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        // A lockfile key always descends through a `node_modules` directory.
        // Anything else is not an installed copy and has no entry to find.
        if !key.split('/').any(|segment| segment == "node_modules") {
            continue;
        }
        for candidate in [
            ancestor.join("package-lock.json"),
            // npm's hidden lockfile: the same shape, written into the tree it
            // describes, and keyed relative to that tree's parent.
            ancestor.join("node_modules").join(".package-lock.json"),
        ] {
            let data = match fs::read(&candidate) {
                Ok(data) => data,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            // A lockfile this checker cannot read is not a malformed *contract*
            // and must not fail the run over a file the project did not write
            // for it.
            let Ok(lockfile) = serde_json::from_slice::<NpmLockfile>(&data) else {
                continue;
            };
            if !matches!(lockfile.lockfile_version, 2 | 3) {
                continue;
            }
            let Some(entry) = lockfile.packages.get(&key) else {
                continue;
            };
            if entry.integrity.is_empty() {
                continue;
            }
            match &found {
                None => found = Some(entry.integrity.clone()),
                Some(existing) if *existing != entry.integrity => return Ok(None),
                Some(_) => {}
            }
        }

        let bun_candidate = ancestor.join("bun.lock");
        let data = match fs::read(&bun_candidate) {
            Ok(data) => data,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if let Some(integrity) = bun_package_integrity(package_directory, &data) {
            match &found {
                None => found = Some(integrity),
                Some(existing) if *existing != integrity => return Ok(None),
                Some(_) => {}
            }
        }
    }
    if found.is_none() {
        found = manifest_keyed_lockfile_integrity(project_directory, package_directory)?;
    }
    Ok(found)
}

/// The pnpm and Yarn-classic arm of the search above.
///
/// Separate from the ancestor walk because neither lockfile is keyed by install
/// path. pnpm's store path (`.pnpm/<name>@<version>_<peers>/node_modules/…`) is
/// not a key, and Yarn classic is keyed by *descriptor* (`name@range`), several
/// of which share one entry. Both are therefore selected by the installed
/// manifest's own name and version, which means there is no per-path
/// disambiguation to fold into the loop.
///
/// Consulted only when no npm or Bun lockfile answered, so an npm-installed
/// tree keeps its existing answer. Within an ancestor, pnpm is asked first for
/// the same reason: an established answer must not move.
fn manifest_keyed_lockfile_integrity(
    project_directory: &Path,
    package_directory: &Path,
) -> Result<Option<String>, BackendError> {
    let manifest = package_directory.join("package.json");
    let Ok(bytes) = fs::read(&manifest) else {
        return Ok(None);
    };
    // Only the identity fields matter here, and `PackageManifest` does not
    // carry the name, so read the two directly rather than widening a struct
    // the rest of this module uses for dependency edges.
    let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(None);
    };
    let (Some(name), Some(version)) = (
        manifest.get("name").and_then(serde_json::Value::as_str),
        manifest.get("version").and_then(serde_json::Value::as_str),
    ) else {
        return Ok(None);
    };
    type Reader = fn(&[u8], String, &str, &str) -> Option<String>;
    let pnpm: Reader = |data, locator, name, version| {
        crate::contract_certification::PublishedGraphLockSelection::from_pnpm_lock(
            data, locator, name, version,
        )
        .ok()
        .map(|selection| selection.integrity().to_owned())
    };
    let yarn: Reader = |data, locator, name, version| {
        crate::contract_certification::PublishedGraphLockSelection::from_yarn_lock(
            data, locator, name, version,
        )
        .ok()
        .map(|selection| selection.integrity().to_owned())
    };
    for ancestor in project_directory.ancestors() {
        for (file, read) in [("pnpm-lock.yaml", pnpm), ("yarn.lock", yarn)] {
            let data = match fs::read(ancestor.join(file)) {
                Ok(data) => data,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            // A lockfile this checker cannot read is not a malformed *contract*
            // and must not fail the run, exactly as for the npm arm. A Yarn
            // Berry lockfile lands here: it records a cache checksum rather than
            // the registry integrity, so it states no fact and admits nothing.
            return Ok(read(&data, format!("{name}@{version}"), name, version));
        }
    }
    Ok(None)
}

fn manifest_uses_solid(manifest: &PackageManifest) -> bool {
    [
        &manifest.dependencies,
        &manifest.peer_dependencies,
        &manifest.optional_dependencies,
    ]
    .iter()
    .any(|dependencies| {
        dependencies
            .keys()
            .any(|name| solid_dialect::ecosystem_dependency(name))
    })
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
    use std::{path::Path, sync::Arc};

    use solid_facts::core::Generation;
    use solid_facts::{ProjectFacts, TypeScriptTable};
    use solid_reactive_ir::{RuntimeEnvironment, contract_semantics::AcceptedContractIndex};

    use super::{
        DiagnosticSession, agreed_admissions, installed_environment_matches,
        installed_package_integrity, retain_enabled,
    };

    const MINIMAL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../benchmarks/package-contract-v2/phase6/minimal-unknown.json"
    ));

    fn acceptance_from(bytes: &[u8]) -> solid_reactive_ir::contract_semantics::AcceptedContract {
        let contract = crate::contract_document::decode(bytes)
            .unwrap()
            .normalize()
            .unwrap();
        let case = contract.artifact_cases()[0].id.clone();
        solid_reactive_ir::contract_semantics::proof::project_untrusted_proposal_for_generation(
            contract, &case,
        )
        .unwrap()
    }

    fn acceptance() -> solid_reactive_ir::contract_semantics::AcceptedContract {
        acceptance_from(MINIMAL)
    }

    /// A host that declares no export conditions cannot narrow two acceptances
    /// of one artifact to one, so both arrive here and this decides.
    ///
    /// It decides on what they *claim*. The rule this replaced admitted when
    /// every candidate named the same runtime file, which compares where a case
    /// came from: a contract describes an entry file's export surface while its
    /// semantics depend on the whole module closure, and conditions select that
    /// closure.
    #[test]
    fn two_candidates_for_one_specifier_are_admitted_only_when_they_agree() {
        let both = || {
            vec![
                ("pkg".to_owned(), "left".to_owned()),
                ("pkg".to_owned(), "right".to_owned()),
            ]
        };
        let agreeing = AcceptedContractIndex::from_artifact_acceptances([
            ("left".to_owned(), acceptance()),
            ("right".to_owned(), acceptance()),
        ]);
        assert_eq!(
            agreed_admissions(&agreeing, both()),
            [("pkg".to_owned(), "left".to_owned())],
            "two certifications that claim the same thing are one answer"
        );

        // One of them states a different export surface. Which describes what
        // this project runs is exactly the question nothing here can answer, so
        // neither may be applied.
        let divergent = String::from_utf8(MINIMAL.to_vec())
            .unwrap()
            .replace(r#""version": "plain-value""#, r#""release": "plain-value""#);
        assert_ne!(divergent.as_bytes(), MINIMAL, "the variant must differ");
        let disagreeing = AcceptedContractIndex::from_artifact_acceptances([
            ("left".to_owned(), acceptance()),
            ("right".to_owned(), acceptance_from(divergent.as_bytes())),
        ]);
        assert!(
            agreed_admissions(&disagreeing, both()).is_empty(),
            "two different answers about one artifact must admit neither"
        );

        // One candidate needs no comparison, and an identity the index does not
        // carry answers nothing.
        assert_eq!(
            agreed_admissions(&agreeing, vec![("pkg".to_owned(), "left".to_owned())]),
            [("pkg".to_owned(), "left".to_owned())]
        );
        assert!(
            agreed_admissions(&agreeing, vec![("pkg".to_owned(), "absent".to_owned())]).is_empty()
        );
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "solid-checker-diagnostics-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn lockfile(version: u32, key: &str, integrity: Option<&str>) -> String {
        let entry = integrity.map_or_else(
            || "{ \"resolved\": \"packages/pkg\", \"link\": true }".to_owned(),
            |value| format!("{{ \"version\": \"1.0.0\", \"integrity\": \"{value}\" }}"),
        );
        format!(
            "{{ \"lockfileVersion\": {version}, \"packages\": {{ \"\": {{}}, {key:?}: {entry} }} }}"
        )
    }

    /// Yarn classic states the registry integrity; Yarn Berry cannot.
    ///
    /// Both halves are the point. A v1 lockfile carries the tarball's
    /// subresource integrity and is keyed by descriptor, so several entries can
    /// reach one installed copy and they have to agree. Berry's `checksum:` is
    /// a hash of the package's zip in Yarn's own cache, which is not the fact
    /// artifact admission needs -- so a Berry project must state nothing rather
    /// than state something that will never reproduce an acceptance root.
    #[test]
    fn yarn_states_an_integrity_only_where_the_format_carries_one() {
        let root = scratch("yarn-integrity");
        let project = root.join("app");
        let package = project.join("node_modules/@scope/pkg");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{ "name": "@scope/pkg", "version": "1.0.0" }"#,
        )
        .unwrap();
        let yarn = project.join("yarn.lock");
        let write = |body: &str| std::fs::write(&yarn, body).unwrap();
        // Real SHA-512 SRI values, because this reader goes through
        // `PublishedGraphLockSelection`, which validates the shape. The npm arm
        // reads its integrity straight out of JSON and does not, which is why
        // the test above can use a placeholder and this one cannot.

        // Two descriptors share one entry, which is the ordinary v1 shape.
        write(concat!(
            "# THIS IS AN AUTOGENERATED FILE\n",
            "# yarn lockfile v1\n",
            "\n",
            "\"@scope/pkg@^1.0.0\", \"@scope/pkg@~1.0.0\":\n",
            "  version \"1.0.0\"\n",
            "  resolved \"https://registry.yarnpkg.com/@scope/pkg/-/pkg-1.0.0.tgz#abc\"\n",
            "  integrity sha512-e8jH4SHIsKZrfxbOqcFlhtmvZTmN8kDtn5STzePihkjB9e2JGwBet9s14tcPSCl4hucoKEPpRI/PshjSzpvDvA==\n",
            "  dependencies:\n",
            "    version \"^9.9.9\"\n",
        ));
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            Some("sha512-e8jH4SHIsKZrfxbOqcFlhtmvZTmN8kDtn5STzePihkjB9e2JGwBet9s14tcPSCl4hucoKEPpRI/PshjSzpvDvA==".to_owned()),
            "a dependency literally named `version` sits at four spaces and is not a field"
        );

        // Two entries reaching the same installed copy must agree.
        write(concat!(
            "# yarn lockfile v1\n",
            "\n",
            "\"@scope/pkg@^1.0.0\":\n",
            "  version \"1.0.0\"\n",
            "  integrity sha512-e8jH4SHIsKZrfxbOqcFlhtmvZTmN8kDtn5STzePihkjB9e2JGwBet9s14tcPSCl4hucoKEPpRI/PshjSzpvDvA==\n",
            "\n",
            "\"@scope/pkg@~1.0.0\":\n",
            "  version \"1.0.0\"\n",
            "  integrity sha512-SDYM9+5CDQbpn73xotxh2JVn3K9xKVAhCBZxyB+Oqa+wQfndYGUs86G3v6Ln0Dn6QB32gt3ReqcmaG1HVZuskA==\n",
        ));
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // A git dependency has no registry tarball and so no integrity.
        write(concat!(
            "# yarn lockfile v1\n",
            "\n",
            "\"@scope/pkg@git+ssh://git@github.com/scope/pkg.git#abc\":\n",
            "  version \"1.0.0\"\n",
            "  resolved \"git+ssh://git@github.com/scope/pkg.git#abc\"\n",
        ));
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None,
            "the `@` inside the URL is not the descriptor separator either"
        );

        // Berry. Refused as a format, not read as an empty classic file.
        write(concat!(
            "__metadata:\n",
            "  version: 8\n",
            "  cacheKey: 10c0\n",
            "\n",
            "\"@scope/pkg@npm:1.0.0\":\n",
            "  version: 1.0.0\n",
            "  resolution: \"@scope/pkg@npm:1.0.0\"\n",
            "  checksum: 10c0/not-a-registry-integrity\n",
        ));
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // An npm lockfile beside it still wins: an established answer must not
        // move because a second reader was added.
        write(concat!(
            "# yarn lockfile v1\n",
            "\n",
            "\"@scope/pkg@^1.0.0\":\n",
            "  version \"1.0.0\"\n",
            "  integrity sha512-e8jH4SHIsKZrfxbOqcFlhtmvZTmN8kDtn5STzePihkjB9e2JGwBet9s14tcPSCl4hucoKEPpRI/PshjSzpvDvA==\n",
        ));
        std::fs::write(
            project.join("package-lock.json"),
            lockfile(3, "node_modules/@scope/pkg", Some("sha512-npm")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            Some("sha512-npm".to_owned())
        );
    }

    /// The lockfile read is the whole basis of integrity enforcement, and every
    /// way it can fail to produce a fact must produce *no* fact — never a
    /// verdict. `None` here means the contract keeps applying on version
    /// identity, so a wrong `Some` would refuse a good contract and a wrong
    /// `None` would accept a bad one.
    #[test]
    fn lockfile_integrity_is_recovered_only_when_it_is_unambiguous() {
        let root = scratch("lockfile-integrity");
        let project = root.join("app");
        let package = project.join("node_modules/pkg");
        std::fs::create_dir_all(&package).unwrap();

        // No lockfile at all: a fresh checkout, or a manager none of these
        // readers covers.
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // Bun's lockfile records package identifiers and integrities in a
        // JSON-with-trailing-commas document. The installed manifest version
        // selects the package record.
        std::fs::write(
            package.join("package.json"),
            r#"{ "name": "pkg", "version": "1.0.0" }"#,
        )
        .unwrap();
        std::fs::write(
            project.join("bun.lock"),
            r#"{
              "lockfileVersion": 2,
              "packages": {
                "pkg": ["pkg@1.0.0", "", {}, "sha512-bun",],
              },
            }"#,
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            Some("sha512-bun".to_owned())
        );
        std::fs::remove_file(project.join("bun.lock")).unwrap();

        // The plain lockfile, keyed by install path.
        std::fs::write(
            project.join("package-lock.json"),
            lockfile(3, "node_modules/pkg", Some("sha512-one")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            Some("sha512-one".to_owned())
        );

        // The hidden lockfile agrees: still one fact.
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(
            project.join("node_modules/.package-lock.json"),
            lockfile(3, "node_modules/pkg", Some("sha512-one")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            Some("sha512-one".to_owned())
        );

        // The hidden lockfile disagrees. Which one describes the bytes on disk
        // is exactly the question this cannot answer, so it answers nothing.
        std::fs::write(
            project.join("node_modules/.package-lock.json"),
            lockfile(3, "node_modules/pkg", Some("sha512-two")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );
        std::fs::remove_file(project.join("node_modules/.package-lock.json")).unwrap();

        // A workspace link has no registry tarball, so it has no integrity.
        std::fs::write(
            project.join("package-lock.json"),
            lockfile(3, "node_modules/pkg", None),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // lockfileVersion 1 keys its tree by package *name*, which cannot say
        // which installed copy an entry describes under hoisting.
        std::fs::write(
            project.join("package-lock.json"),
            lockfile(1, "node_modules/pkg", Some("sha512-one")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // A lockfile this checker cannot parse is the project's file, not a
        // malformed contract: it yields no fact rather than failing the run.
        std::fs::write(project.join("package-lock.json"), "{ not json").unwrap();
        assert_eq!(
            installed_package_integrity(&project, &package).unwrap(),
            None
        );

        // A hoisted install: the package sits above the project, and the key is
        // relative to the lockfile that owns that tree.
        std::fs::remove_file(project.join("package-lock.json")).unwrap();
        let hoisted = root.join("node_modules/pkg");
        std::fs::create_dir_all(&hoisted).unwrap();
        std::fs::write(
            root.join("package-lock.json"),
            lockfile(3, "node_modules/pkg", Some("sha512-hoisted")),
        )
        .unwrap();
        assert_eq!(
            installed_package_integrity(&project, &hoisted).unwrap(),
            Some("sha512-hoisted".to_owned())
        );

        std::fs::remove_dir_all(&root).ok();
    }

    fn real_lockfile(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/lockfiles")
                .join(name),
        )
        .unwrap()
    }

    fn install(project: &Path, relative: &str, name: &str, version: &str) -> std::path::PathBuf {
        let directory = project.join(relative);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("package.json"),
            format!(r#"{{ "name": "{name}", "version": "{version}" }}"#),
        )
        .unwrap();
        directory
    }

    const META_NEXT_2: &str = "sha512-4aqPczFqDdep4JTAUXfh4nyfq7InbTgimd7NuN6ZWWE29UPv0uR5dyr53yFcf2YgVXjFdX+JGhXtsE0njGL+fA==";
    const META_0_29_4: &str = "sha512-zdIWBGpR9zGx1p1bzIPqF5Gs+Ks/BH8R6fWhmUa/dcK1L2rUC8BAcZJzNRYBQv74kScf1TSOs0EY//Vd/I0V8g==";

    /// `bun.lock` `lockfileVersion: 1` writes the same npm package tuple as
    /// version 2 (Civil's real lockfile), so its integrity is read -- bound to
    /// the exact installed identity. Civil hoists `@solidjs/meta` at
    /// `1.0.0-next.2` and nests `0.29.4` under `@tanstack/solid-router`: the
    /// nested copy's own record answers for it, where matching the hoisted key
    /// by name alone would hand it the other version's integrity.
    #[test]
    fn a_bun_lock_version_1_states_the_exact_installed_integrity() {
        let project = scratch("bun-lock-v1");
        let lock = real_lockfile("civil.bun.lock");
        std::fs::write(project.join("bun.lock"), &lock).unwrap();
        let hoisted = install(
            &project,
            "node_modules/@solidjs/meta",
            "@solidjs/meta",
            "1.0.0-next.2",
        );
        let nested = install(
            &project,
            "node_modules/@tanstack/solid-router/node_modules/@solidjs/meta",
            "@solidjs/meta",
            "0.29.4",
        );
        let integrity =
            |directory: &Path| installed_package_integrity(&project, directory).unwrap();
        assert_eq!(integrity(&hoisted), Some(META_NEXT_2.to_owned()));
        assert_eq!(integrity(&nested), Some(META_0_29_4.to_owned()));
        // A version the lockfile never recorded has no record, though the
        // hoisted key carries the name.
        let unrecorded = install(
            &project,
            "node_modules/@solidjs/meta",
            "@solidjs/meta",
            "1.0.0-next.3",
        );
        assert_eq!(integrity(&unrecorded), None);
        install(
            &project,
            "node_modules/@solidjs/meta",
            "@solidjs/meta",
            "1.0.0-next.2",
        );

        // Each refusal kept, one change to the real bytes at a time.
        for (name, changed) in [
            (
                "an unknown lockfileVersion",
                lock.replacen("\"lockfileVersion\": 1,", "\"lockfileVersion\": 3,", 1),
            ),
            (
                "lockfileVersion 0",
                lock.replacen("\"lockfileVersion\": 1,", "\"lockfileVersion\": 0,", 1),
            ),
            (
                "no lockfileVersion",
                lock.replacen("  \"lockfileVersion\": 1,\n", "", 1),
            ),
            (
                "a missing integrity",
                lock.replacen(&format!(", \"{META_NEXT_2}\"]"), "]", 1),
            ),
            (
                "an empty integrity",
                lock.replacen(META_NEXT_2, "", 1),
            ),
            (
                "an integrity that is not SRI",
                lock.replacen(META_NEXT_2, "md5-4aqPczFqDdep4JTAUXfh4n==", 1),
            ),
            (
                "a conflicting duplicate record",
                lock.replacen(
                    "  \"packages\": {\n",
                    &format!(
                        "  \"packages\": {{\n    \"other/@solidjs/meta\": [\"@solidjs/meta@1.0.0-next.2\", \"\", {{}}, \"{META_0_29_4}\"],\n\n"
                    ),
                    1,
                ),
            ),
        ] {
            assert_ne!(changed, lock, "{name}");
            std::fs::write(project.join("bun.lock"), &changed).unwrap();
            assert_eq!(integrity(&hoisted), None, "{name} was read instead of refused");
        }
        // An agreeing duplicate is one fact.
        std::fs::write(
            project.join("bun.lock"),
            lock.replacen(
                "  \"packages\": {\n",
                &format!(
                    "  \"packages\": {{\n    \"other/@solidjs/meta\": [\"@solidjs/meta@1.0.0-next.2\", \"\", {{}}, \"{META_NEXT_2}\"],\n\n"
                ),
                1,
            ),
        )
        .unwrap();
        assert_eq!(integrity(&hoisted), Some(META_NEXT_2.to_owned()));
        std::fs::remove_dir_all(&project).ok();
    }

    /// Every Bun reader accepts one `lockfileVersion` set: admission's
    /// integrity reader and certification's `from_bun_lock` both refuse a
    /// version 0, a version 3 and a missing version, so no receipt can be
    /// issued from a lockfile admission would refuse. The acquisition twin's
    /// half is "every Bun reader refuses a lockfileVersion admission refuses"
    /// in `published-contract-graph.test.mjs`, over the same bytes.
    #[test]
    fn every_bun_reader_refuses_the_versions_admission_refuses() {
        let project = scratch("bun-lock-versions");
        let lock = real_lockfile("civil.bun.lock");
        let hoisted = install(
            &project,
            "node_modules/@solidjs/meta",
            "@solidjs/meta",
            "1.0.0-next.2",
        );
        let certify = |bytes: &str| {
            crate::contract_certification::PublishedGraphLockSelection::from_bun_lock(
                bytes.as_bytes(),
                "@solidjs/meta",
                "@solidjs/meta",
                "1.0.0-next.2",
            )
        };
        let admit = |bytes: &str| {
            std::fs::write(project.join("bun.lock"), bytes).unwrap();
            installed_package_integrity(&project, &hoisted).unwrap()
        };
        for accepted in ["1", "2"] {
            let bytes = lock.replacen(
                "\"lockfileVersion\": 1,",
                &format!("\"lockfileVersion\": {accepted},"),
                1,
            );
            assert_eq!(admit(&bytes), Some(META_NEXT_2.to_owned()), "v{accepted}");
            assert_eq!(
                certify(&bytes).unwrap().integrity(),
                META_NEXT_2,
                "v{accepted}"
            );
        }
        for (name, bytes, reason) in [
            (
                "version 0",
                lock.replacen("\"lockfileVersion\": 1,", "\"lockfileVersion\": 0,", 1),
                "Bun lockfileVersion 0 is not 1 or 2; only those versions' package records are read",
            ),
            (
                "version 3",
                lock.replacen("\"lockfileVersion\": 1,", "\"lockfileVersion\": 3,", 1),
                "Bun lockfileVersion 3 is not 1 or 2; only those versions' package records are read",
            ),
            (
                "a missing version",
                lock.replacen("  \"lockfileVersion\": 1,\n", "", 1),
                "Bun lockfile does not declare a lockfileVersion",
            ),
        ] {
            assert_ne!(bytes, lock, "{name}");
            assert_eq!(admit(&bytes), None, "admission read {name}");
            let error = certify(&bytes).unwrap_err();
            assert!(format!("{error}").contains(reason), "{name}: {error}");
        }
        std::fs::remove_dir_all(&project).ok();
    }

    /// The pnpm arm reads a pnpm 11+ lockfile's project document, the same
    /// reader certification uses, so admission and the certified dependency
    /// environment state one integrity for it (finds.team's real lockfile).
    #[test]
    fn a_pnpm_11_lockfile_states_the_project_documents_integrity() {
        let project = scratch("pnpm-env-document");
        std::fs::write(
            project.join("pnpm-lock.yaml"),
            real_lockfile("finds-team.pnpm-lock.yaml"),
        )
        .unwrap();
        let router = install(
            &project,
            "node_modules/@tanstack/solid-router",
            "@tanstack/solid-router",
            "2.0.0-rc.8",
        );
        let pnpm = install(&project, "node_modules/pnpm", "pnpm", "12.5.1");
        assert_eq!(
            installed_package_integrity(&project, &router).unwrap(),
            Some(
                "sha512-szioKo5iiBnpYS8oSVinGRCS0PFsk07j/C++u+PNW+J6Kyj0luls6GG5EUulzy7WoG9H3qRpjo7G7Znm0fnfSA=="
                    .to_owned()
            )
        );
        // pnpm itself is the env document's, installed outside the project.
        assert_eq!(installed_package_integrity(&project, &pnpm).unwrap(), None);
        std::fs::remove_dir_all(&project).ok();
    }

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
            runtime_symbol_redirects: Default::default(),
        };
        let mut session = DiagnosticSession::default();
        let contracts = AcceptedContractIndex::default();

        let (initial, initial_timings) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
                super::RequestedRuleEnablement::default(),
            )
            .unwrap();
        let (reused, reused_timings) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
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
            runtime_symbol_redirects: Default::default(),
        };
        let mut session = DiagnosticSession::default();
        let contracts = AcceptedContractIndex::default();

        let (_, baseline) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
                super::RequestedRuleEnablement::default(),
            )
            .unwrap();
        let (preset_analysis, preset_miss) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
                super::RequestedRuleEnablement {
                    presets: &["preferences".into()],
                    rules: &[],
                    runtime: RuntimeEnvironment::default(),
                },
            )
            .unwrap();
        let (preset_reused, preset_hit) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
                super::RequestedRuleEnablement {
                    presets: &["preferences".into(), "preferences".into()],
                    rules: &[],
                    runtime: RuntimeEnvironment::default(),
                },
            )
            .unwrap();
        let (_, rule_miss) = session
            .analyze_accepted_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &contracts,
                super::RequestedRuleEnablement {
                    presets: &[],
                    rules: &["prefer-show".into()],
                    runtime: RuntimeEnvironment::default(),
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
    /// The filesystem half of environment admission, on a real installed
    /// tree: the certified environment is resolved from the imported package's
    /// own copy, the way Node resolves it, and compared by name, manifest
    /// version and lockfile integrity. Anything else refuses the bundle.
    #[test]
    fn a_bundle_environment_is_checked_against_the_installed_tree() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-bundle-environment-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        let write = |relative: &str, text: &str| {
            let path = directory.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        let lock = |signals: &str, nested: Option<&str>| {
            let nested = nested.map_or_else(String::new, |integrity| {
                format!(
                    r#","node_modules/@solid-primitives/utils/node_modules/@solidjs/signals":
                        {{"version":"2.0.0-rc.0","integrity":"{integrity}"}}"#
                )
            });
            format!(
                r#"{{"lockfileVersion":3,"packages":{{
                    "node_modules/@solid-primitives/utils":
                        {{"version":"7.0.0-next.4","integrity":"sha512-utils"}},
                    "node_modules/@solidjs/signals":
                        {{"version":"2.0.0-rc.6","integrity":"{signals}"}}{nested}}}}}"#
            )
        };
        write(
            "node_modules/@solid-primitives/utils/package.json",
            r#"{"name":"@solid-primitives/utils","version":"7.0.0-next.4"}"#,
        );
        write(
            "node_modules/@solidjs/signals/package.json",
            r#"{"name":"@solidjs/signals","version":"2.0.0-rc.6"}"#,
        );
        write("package-lock.json", &lock("sha512-signals-rc6", None));
        let signals = |version: &str, integrity: &str| {
            crate::DependencyEnvironmentEntry::package("@solidjs/signals", version, integrity)
        };
        let head = [signals("2.0.0-rc.6", "sha512-signals-rc6")];
        let matches = |environment: &[crate::DependencyEnvironmentEntry]| {
            installed_environment_matches(&directory, "@solid-primitives/utils", environment)
        };

        assert!(
            matches(&head),
            "the certified signals, hoisted beside utils"
        );
        assert!(matches(&[]), "an empty environment needs nothing installed");
        assert!(
            !matches(&[signals("2.0.0-rc.0", "sha512-signals-rc6")]),
            "a different signals version refuses"
        );
        assert!(
            !matches(&[signals("2.0.0-rc.6", "sha512-signals-repacked")]),
            "a different dependency integrity refuses"
        );
        assert!(
            !matches(&[crate::DependencyEnvironmentEntry::package(
                "@solidjs/web",
                "2.0.0-rc.6",
                "sha512-web",
            )]),
            "a dependency that is not installed refuses"
        );

        // The lockfile names no integrity for the installed copy: not a fact
        // this project makes available, so nothing is admitted on it.
        write(
            "package-lock.json",
            r#"{"lockfileVersion":3,"packages":{
                "node_modules/@solid-primitives/utils":{"version":"7.0.0-next.4","integrity":"sha512-utils"},
                "node_modules/@solidjs/signals":{"version":"2.0.0-rc.6"}}}"#,
        );
        assert!(
            !matches(&head),
            "a dependency with no stated integrity refuses"
        );

        // utils carries its own nested rc.0, which is what Node loads from it,
        // however right the hoisted copy is.
        write(
            "node_modules/@solid-primitives/utils/node_modules/@solidjs/signals/package.json",
            r#"{"name":"@solidjs/signals","version":"2.0.0-rc.0"}"#,
        );
        write(
            "package-lock.json",
            &lock("sha512-signals-rc6", Some("sha512-signals-rc0")),
        );
        assert!(
            !matches(&head),
            "the copy the imported package resolves is the nested one"
        );
        assert!(
            matches(&[signals("2.0.0-rc.0", "sha512-signals-rc0")]),
            "and that nested copy is the one checked"
        );
        let _ = std::fs::remove_dir_all(&directory);
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

    /// A discovered catalog that needs trust nobody supplied is withheld and
    /// explained, never fatal; one the user named still refuses; and a catalog
    /// that needs no trust is read exactly as before. The process-level half,
    /// with a real signed receipt, is
    /// `an_authorized_catalog_is_not_admitted_without_the_trust_configuration`.
    #[test]
    fn a_catalog_needing_absent_trust_is_withheld_only_when_discovered() {
        let fixture = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/reactive-ir/package-return-consumer/.solid-checker/accepted-contracts.json"
        ))
        .unwrap();
        assert!(fixture.contains(r#""status": "obsolete-policy1""#));
        let root = std::env::temp_dir().join(format!(
            "solid-checker-withheld-catalog-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project = |name: &str, catalog: &str| {
            let directory = root.join(name);
            std::fs::create_dir_all(directory.join(".solid-checker")).unwrap();
            std::fs::write(
                directory.join(".solid-checker/accepted-contracts.json"),
                catalog,
            )
            .unwrap();
            directory
        };
        let signed = project(
            "signed",
            &fixture.replace(
                r#""status": "obsolete-policy1""#,
                r#""status": "policy2-portable""#,
            ),
        );
        let catalog = signed.join(".solid-checker/accepted-contracts.json");

        let withheld = super::select_project_catalogs(&signed, "", false).unwrap();
        assert!(withheld.admitted.is_empty());
        assert!(
            !withheld.is_empty(),
            "a withheld catalog still counts as found"
        );
        assert_eq!(withheld.unauthenticated.len(), 1);
        assert_eq!(withheld.unauthenticated[0].path, catalog);
        assert_eq!(withheld.unauthenticated[0].packages, ["reactive-package"]);
        let notice = withheld.notice().unwrap();
        assert!(notice.contains(&catalog.display().to_string()), "{notice}");
        assert!(notice.contains("reactive-package"), "{notice}");
        assert!(
            notice.contains("--receipt-trust-configuration <trust.json>"),
            "{notice}"
        );
        // It replaces a compiled-in tier's refusal: the project tier answers first.
        let mut refusals = std::collections::BTreeMap::from([(
            "reactive-package".to_owned(),
            "a compiled-in contract exists for this package and was not admitted".to_owned(),
        )]);
        withheld.extend_refusals(&mut refusals);
        assert!(refusals["reactive-package"].contains("was not read"));
        assert!(refusals["reactive-package"].contains(&catalog.display().to_string()));

        let trusted = super::select_project_catalogs(&signed, "", true).unwrap();
        assert_eq!(trusted.admitted, std::slice::from_ref(&catalog));
        assert!(trusted.unauthenticated.is_empty() && trusted.notice().is_none());

        let explicit = catalog.to_string_lossy().into_owned();
        let refusal = super::select_project_catalogs(&root, &explicit, false)
            .unwrap_err()
            .to_string();
        assert!(
            refusal.contains("authenticated issuer provenance"),
            "{refusal}"
        );
        assert!(refusal.contains(&explicit), "{refusal}");
        let named = super::select_project_catalogs(&root, &explicit, true).unwrap();
        assert_eq!(named.admitted, std::slice::from_ref(&catalog));

        // Obsolete policy-1 entries need no trust to be read as uncertifiable,
        // so nothing about such a catalog changes.
        let obsolete = project("obsolete", &fixture);
        let unchanged = super::select_project_catalogs(&obsolete, "", false).unwrap();
        assert_eq!(unchanged.admitted.len(), 1);
        assert!(unchanged.notice().is_none());
        let explicit = obsolete.join(".solid-checker/accepted-contracts.json");
        assert!(super::select_project_catalogs(&root, &explicit.to_string_lossy(), false).is_ok());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn nested_catalog_candidates_are_the_directories_between_a_file_and_the_project() {
        let candidates = super::nested_catalog_candidates(
            Path::new("/mono"),
            [
                "/mono/packages/a/src/App.ts",
                "/mono/packages/a/src/deep/Other.ts",
                "/mono/packages/b/Main.ts",
                // The project's own directory is not a candidate: its catalog
                // is the project's.
                "/mono/index.ts",
                // Installed packages are not this project's sources.
                "/mono/node_modules/pkg/index.d.ts",
                "/mono/packages/a/node_modules/pkg/index.d.ts",
                // Outside the project nothing is nested in it.
                "/elsewhere/src/main.ts",
                "/monorepo/src/main.ts",
            ],
        );
        assert_eq!(
            candidates,
            [
                "/mono/packages",
                "/mono/packages/a",
                "/mono/packages/a/src",
                "/mono/packages/a/src/deep",
                "/mono/packages/b",
            ]
            .map(std::path::PathBuf::from)
        );
        assert!(
            super::nested_catalog_candidates(Path::new("/mono"), ["/mono/src/App.ts"])
                .into_iter()
                .eq([std::path::PathBuf::from("/mono/src")])
        );
    }
}
