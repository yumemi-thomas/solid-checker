use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solid_facts::ProjectFacts;
use solid_reactive_ir::{
    CacheRetention, Finding, IncrementalBuilder, PackageContract, PackageContractIssue,
    PackageContractIssueKind, Program, RuleOptions, suppress_findings_owned_by_enabled_rules,
};

use crate::dialect::{self, Dialect};
use crate::{BackendError, SourceFile};

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
    pub evidence: String,
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
    pub contracts: Vec<PackageContract>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DiagnosticTimings {
    pub reactive_ir: Duration,
    pub solve_and_snapshot: Duration,
    pub reused: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RequestedRuleEnablement<'a> {
    pub presets: &'a [String],
    pub rules: &'a [String],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticIdentity {
    /// Which dialect's catalog and compiler produced the retained analysis;
    /// a retained result never answers for a different dialect.
    dialect: &'static str,
    project_id: String,
    generation: u64,
    contracts: Vec<[u8; 32]>,
    /// Contracts refused for describing another version. These produce
    /// findings but never enter `contracts`, so without them a retained
    /// analysis from before a contract went stale would still answer.
    stale_contracts: Vec<StaleContract>,
    explicit_contract_paths: Vec<String>,
    /// Per-rule options are re-read from disk on every analysis, so an
    /// edited `.solid-checker/rule-options.json` invalidates a retained
    /// diagnostic even within one generation.
    rule_options: RuleOptions,
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
        explicit_contract_paths: &[String],
    ) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
        self.analyze_measured(project, sources, facts, explicit_contract_paths, None)
            .map(|(analysis, _)| analysis)
    }

    pub fn analyze_with_enablement(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        explicit_contract_paths: &[String],
        presets: &[String],
        enable_rules: &[String],
    ) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
        self.analyze_measured_with_enablement(
            project,
            sources,
            facts,
            explicit_contract_paths,
            None,
            RequestedRuleEnablement {
                presets,
                rules: enable_rules,
            },
        )
        .map(|(analysis, _)| analysis)
    }

    pub fn analyze_measured(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        explicit_contract_paths: &[String],
        bundled_solid_js: Option<PackageContract>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        self.analyze_measured_with_enablement(
            project,
            sources,
            facts,
            explicit_contract_paths,
            bundled_solid_js,
            RequestedRuleEnablement::default(),
        )
    }

    pub fn analyze_measured_with_enablement(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        explicit_contract_paths: &[String],
        bundled_solid_js: Option<PackageContract>,
        enablement: RequestedRuleEnablement<'_>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        let ir_started = Instant::now();
        let loaded = load_package_contracts_reporting(
            self.dialect,
            project,
            facts,
            explicit_contract_paths,
            bundled_solid_js,
        )?;
        let mut rule_options = discover_rule_options(project)?;
        rule_options.request_presets(enablement.presets.iter().cloned());
        rule_options.request_rules(enablement.rules.iter().cloned());
        let identity = DiagnosticIdentity {
            dialect: self.dialect.id,
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: loaded
                .contracts
                .iter()
                .map(PackageContract::analysis_fingerprint)
                .collect(),
            stale_contracts: loaded.stale.clone(),
            explicit_contract_paths: explicit_contract_paths.to_vec(),
            rule_options: rule_options.clone(),
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

        let (program, _) = self.builder.build_with_contracts_shared(
            facts,
            self.dialect.vocabulary,
            &loaded.contracts,
            &rule_options,
        )?;
        let reactive_ir = ir_started.elapsed();
        let solve_started = Instant::now();
        let analysis = Arc::new(finish_analysis(
            self.dialect,
            project,
            sources,
            facts,
            loaded,
            program,
            &identity,
        )?);
        let solve_and_snapshot = solve_started.elapsed();
        self.retained = Some(RetainedDiagnostic {
            identity,
            analysis: Arc::clone(&analysis),
        });
        Ok((
            analysis,
            DiagnosticTimings {
                reactive_ir,
                solve_and_snapshot,
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

pub fn analyze_project(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
    analyze_project_measured(dialect, project, sources, facts, explicit_contract_paths)
        .map(|(analysis, _)| analysis)
}

pub fn analyze_project_measured(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    analyze_project_measured_with(
        dialect,
        project,
        sources,
        facts,
        explicit_contract_paths,
        None,
    )
}

/// As [`analyze_project_measured`], but reuses a bundled solid-js contract the
/// caller already decoded (with the Solid 2 bundled-contract decoder) instead of
/// decoding the compile-time-embedded JSON on the analysis path. The cold
/// path decodes it while the service builds the program; the preloaded value
/// is ignored when the project does not import solid-js.
pub fn analyze_project_measured_with(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    DiagnosticSession::new(dialect).analyze_measured(
        project,
        sources,
        facts,
        explicit_contract_paths,
        bundled_solid_js,
    )
}

pub fn analyze_project_measured_with_enablement(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
    enablement: RequestedRuleEnablement<'_>,
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    DiagnosticSession::new(dialect).analyze_measured_with_enablement(
        project,
        sources,
        facts,
        explicit_contract_paths,
        bundled_solid_js,
        enablement,
    )
}

fn finish_analysis(
    dialect: &'static Dialect,
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    loaded: LoadedContracts,
    program: Arc<Program>,
    identity: &DiagnosticIdentity,
) -> Result<DiagnosticAnalysis, BackendError> {
    let LoadedContracts {
        contracts,
        stale: stale_contracts,
    } = loaded;
    let stale_contracts = stale_contracts.as_slice();
    let statuses = package_contract_statuses_with(
        dialect,
        project,
        facts,
        &identity.explicit_contract_paths,
        &contracts,
        stale_contracts,
    )?;
    let unusable = statuses
        .iter()
        .filter(|status| status.needs_action())
        .collect::<Vec<_>>();
    let mut metrics = analysis_metrics(facts, &program, &contracts);
    metrics.proof_obligations += unusable.len();
    metrics.unresolved_obligations += unusable.len();
    let stale_by_package = stale_contracts
        .iter()
        .map(|entry| (entry.package.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let mut findings = dialect.solve(&program);
    findings.extend(unusable.into_iter().map(|status| {
        let location = facts
            .files
            .iter()
            .find_map(|file| {
                file.ast
                    .imports
                    .iter()
                    .find(|import| package_root(&import.module) == status.name)
                    .map(|import| import.span.location(file.path.shared()))
            })
            .unwrap_or_else(|| typefacts::Location {
                path: project.to_string_lossy().into_owned().into(),
                start_byte: 0,
                end_byte: 0,
            });
        let kind = match status.status.as_str() {
            "unverified" => PackageContractIssueKind::Unverified,
            "stale" => stale_by_package.get(status.name.as_str()).map_or(
                PackageContractIssueKind::Missing,
                |entry| {
                    if entry.bundled {
                        PackageContractIssueKind::StaleBundled {
                            audited_version: entry.contract_version.clone(),
                            installed_version: entry.installed_version.clone(),
                        }
                    } else {
                        PackageContractIssueKind::Stale {
                            contract_version: entry.contract_version.clone(),
                            installed_version: entry.installed_version.clone(),
                        }
                    }
                },
            ),
            _ => PackageContractIssueKind::Missing,
        };
        (dialect.package_contract_finding)(&PackageContractIssue {
            package: status.name.clone(),
            contract_path: status.contract_path.clone(),
            status: kind,
            location,
        })
    }));
    retain_enabled(dialect, &identity.rule_options, &mut findings)?;
    suppress_findings_owned_by_enabled_rules(&mut findings, dialect.catalog_capabilities);
    let snapshot = snapshot(sources, &contracts, metrics, findings);
    Ok(DiagnosticAnalysis {
        program,
        contracts,
        snapshot,
    })
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

pub fn snapshot(
    sources: &[SourceFile],
    contracts: &[PackageContract],
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
        package_summaries: contracts
            .iter()
            .map(|contract| PackageSummary {
                name: contract.package.name.clone(),
                version: contract.package.version.clone(),
                contract_hash: contract.contract_hash.clone(),
                evidence: contract.evidence.kind.clone(),
                exports_analyzed: contract.export_count(),
            })
            .collect(),
        metrics,
    }
}

pub fn analysis_metrics(
    facts: &ProjectFacts,
    program: &Program,
    contracts: &[PackageContract],
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
            let Some(contract) = PackageContract::for_module(contracts, &import.module) else {
                continue;
            };
            for binding in &import.bindings {
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    continue;
                }
                let exported = binding.imported.as_deref().unwrap_or("default");
                let Some(summary) = contract
                    .exports_for_module(&import.module)
                    .and_then(|exports| exports.get(exported))
                else {
                    continue;
                };
                if summary.reactive_reads.is_empty()
                    && summary.returns.is_none()
                    && summary.callbacks.is_empty()
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
                    summary
                        .returns
                        .as_ref()
                        .map(|returned| returned.kind.clone()),
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

/// Decodes the compile-time-embedded solid-js package contract. The result is
/// facts-independent, so a cold-start caller can decode it while the TypeFacts
/// service builds its program, then hand it to [`load_package_contracts_with`]
/// or [`analyze_project_measured_with`].
#[cfg(feature = "dialect-v2")]
pub fn bundled_solid_js_contract() -> Result<PackageContract, BackendError> {
    let mut bundled = decode_package_contract(include_bytes!(
        "../../../../pkg/contracts/bundled/solid-v2/solid-js.json"
    ))?;
    bundled.source_path = "bundled://solid-v2/solid-js.json".into();
    Ok(bundled)
}

#[cfg(feature = "dialect-v2")]
fn bundled_solidjs_web_contract() -> Result<PackageContract, BackendError> {
    let mut bundled = decode_package_contract(include_bytes!(
        "../../../../pkg/contracts/bundled/solid-v2/solidjs-web.json"
    ))?;
    bundled.source_path = "bundled://solid-v2/solidjs-web.json".into();
    Ok(bundled)
}

/// The Solid 2.0 dialect's bundled contract set, keyed by package root.
#[cfg(feature = "dialect-v2")]
pub(crate) fn bundled_contract_v2(package: &str) -> Result<Option<PackageContract>, BackendError> {
    Ok(match package {
        "solid-js" => Some(bundled_solid_js_contract()?),
        "@solidjs/web" => Some(bundled_solidjs_web_contract()?),
        _ => None,
    })
}

/// The Solid 1.x dialect's bundled contract for `solid-js@1.x`, covering the
/// `.`, `./store` and `./web` entrypoints of the package that version
/// actually ships.
#[cfg(feature = "dialect-v1")]
pub(crate) fn bundled_contract_v1(package: &str) -> Result<Option<PackageContract>, BackendError> {
    Ok(match package {
        "solid-js" => {
            let mut bundled = decode_package_contract(include_bytes!(
                "../../../../pkg/contracts/bundled/solid-v1/solid-js.json"
            ))?;
            bundled.source_path = "bundled://solid-v1/solid-js.json".into();
            Some(bundled)
        }
        "@solid-primitives/scheduled" => {
            let mut bundled = decode_package_contract(include_bytes!(
                "../../../../pkg/contracts/bundled/solid-v1/solid-primitives-scheduled.json"
            ))?;
            bundled.source_path = "bundled://solid-v1/solid-primitives-scheduled.json".into();
            Some(bundled)
        }
        _ => None,
    })
}

/// A contract that was discovered, parsed, and then refused because it
/// describes a different release than the installed one.
///
/// Loading is the only place that reads both the contract and the installed
/// manifest, so it is also the only place that can report this without a second
/// pass over the filesystem. Carrying the refusal forward is what lets analysis
/// report drift as a finding instead of rediscovering it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleContract {
    pub package: String,
    pub contract_path: String,
    pub contract_version: String,
    pub installed_version: String,
    /// Whether the refused contract was this checker's own bundled artifact,
    /// which the consumer cannot regenerate.
    pub bundled: bool,
}

pub fn load_package_contracts(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
) -> Result<Vec<PackageContract>, BackendError> {
    load_package_contracts_with(dialect, project, facts, explicit_paths, None)
}

/// As [`load_package_contracts`], but reuses a preloaded bundled solid-js
/// contract instead of decoding the embedded JSON. The preloaded value is used
/// only when the project imports solid-js; the discovery walk and explicit
/// overrides are unchanged, so the resolved contract set is identical.
pub fn load_package_contracts_with(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
) -> Result<Vec<PackageContract>, BackendError> {
    Ok(
        load_package_contracts_reporting(
            dialect,
            project,
            facts,
            explicit_paths,
            bundled_solid_js,
        )?
        .contracts,
    )
}

/// As [`load_package_contracts_with`], but also returns the contracts that were
/// refused for describing another version.
///
/// A stale contract is not an error: it is the same epistemic state as a
/// missing one — no usable summary for the installed package — and it is
/// reported the same way, as an uncertifiable finding. A *malformed* contract
/// still fails the analysis, because that is a broken file rather than drift.
pub fn load_package_contracts_reporting(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
) -> Result<LoadedContracts, BackendError> {
    let mut stale = Vec::new();
    let mut contracts = HashMap::<String, PackageContract>::new();
    let modules = imported_package_roots(facts);
    let modules = modules.iter().map(String::as_str).collect::<HashSet<_>>();
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    let mut bundled_solid_js = bundled_solid_js;
    // Each tier records its refusal under the module it was discovered for. A
    // later tier that supplies a usable contract clears the earlier refusal:
    // a stale published contract is not a defect when the project ships its own
    // override for the installed version.
    let refuse = |stale: &mut Vec<StaleContract>, entry: StaleContract| {
        stale.retain(|existing| existing.package != entry.package);
        stale.push(entry);
    };
    for module in &modules {
        let bundled = if *module == "solid-js" && bundled_solid_js.is_some() {
            bundled_solid_js.take()
        } else {
            (dialect.bundled_contract)(module)?
        };
        if let Some(bundled) = bundled {
            let installed = installed_package_manifest(project_directory, module)?;
            let installed = installed.as_ref().map(|(_, manifest)| manifest);
            if contract_matches_manifest(installed, &bundled) {
                contracts.insert(bundled.package.name.clone(), bundled);
            } else {
                refuse(
                    &mut stale,
                    StaleContract {
                        package: (*module).to_owned(),
                        contract_path: bundled.source_path.clone(),
                        contract_version: bundled.package.version.clone(),
                        installed_version: installed_version(installed),
                        bundled: true,
                    },
                );
            }
        }
    }
    for module in &modules {
        if let Some(path) = discover_contract(project_directory, module)? {
            let contract = read_package_contract(&path)?;
            match classify_identity(project_directory, module, &contract)? {
                Some(entry) => refuse(&mut stale, entry),
                None => {
                    contracts.insert(contract.package.name.clone(), contract);
                }
            }
        }
    }
    for module in &modules {
        if let Some(path) = discover_local_contract(project_directory, module)? {
            let contract = read_package_contract(&path)?;
            match classify_identity(project_directory, module, &contract)? {
                Some(entry) => refuse(&mut stale, entry),
                None => {
                    contracts.insert(contract.package.name.clone(), contract);
                }
            }
        }
    }
    for path in explicit_paths {
        let contract = read_package_contract(Path::new(path))?;
        if modules.contains(contract.package.name.as_str()) {
            let module = contract.package.name.clone();
            if let Some(entry) = classify_identity(project_directory, &module, &contract)? {
                refuse(&mut stale, entry);
                continue;
            }
        }
        contracts.insert(contract.package.name.clone(), contract);
    }
    // A tier that supplied a usable contract wins over any earlier refusal for
    // the same package.
    stale.retain(|entry| !contracts.contains_key(&entry.package));
    let mut contracts = contracts.into_values().collect::<Vec<_>>();
    contracts.sort_by(|left, right| left.package.name.cmp(&right.package.name));
    stale.sort_by(|left, right| left.package.cmp(&right.package));
    Ok(LoadedContracts { contracts, stale })
}

/// The contracts one analysis will use, and the ones it refused as stale.
pub struct LoadedContracts {
    pub contracts: Vec<PackageContract>,
    pub stale: Vec<StaleContract>,
}

fn installed_version(manifest: Option<&PackageManifest>) -> String {
    manifest.map_or_else(|| "unknown".to_owned(), |manifest| manifest.version.clone())
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

/// The package manifests and contract files that influence contract discovery
/// for the given imported modules. The retained check daemon uses this to
/// validate a cached snapshot without re-running analysis.
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
        if let Some(path) = discover_contract(project_directory, module)? {
            paths.push(path);
        }
        if let Some(path) = discover_local_contract(project_directory, module)? {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageContractStatus {
    pub name: String,
    pub status: String,
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
}

impl PackageContractStatus {
    /// Whether this status blocks contract-backed certification. These are the
    /// statuses `--check-contracts` counts and exits non-zero on.
    pub fn needs_action(&self) -> bool {
        matches!(self.status.as_str(), "missing" | "unverified" | "stale")
    }
}

fn contract_evidence_is_certifiable(contract: &PackageContract) -> bool {
    matches!(
        contract.evidence.kind.as_str(),
        "verified" | "reviewed" | "trusted" | "attested"
    ) && contract.claims_are_certifiable()
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
fn installed_package_manifest(
    project_directory: &Path,
    module: &str,
) -> Result<Option<(PathBuf, PackageManifest)>, BackendError> {
    let Some(directory) = discover_package_directory(project_directory, module)? else {
        return Ok(None);
    };
    match fs::read(directory.join("package.json")) {
        Ok(data) => Ok(Some((directory, serde_json::from_slice(&data)?))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Whether a contract describes the version of the package that is actually
/// installed. A contract that describes another version is stale: it was
/// audited against an artifact this project no longer has.
fn contract_matches_manifest(
    manifest: Option<&PackageManifest>,
    contract: &PackageContract,
) -> bool {
    manifest.is_none_or(|manifest| manifest.version == contract.package.version)
}

/// The command that regenerates a project-owned contract for `module`.
///
/// A project-owned contract overrides both a package-published contract and a
/// bundled one, so this is the remedy for every stale or missing tier. It is
/// built in one place because the analysis error and the `--check-contracts`
/// report must not print divergent instructions. `package_root` is the
/// installed package directory when discovery found one, so the printed
/// command stays correct under hoisting.
pub fn contract_regeneration_command(
    project_directory: &Path,
    module: &str,
    package_root: Option<&Path>,
) -> String {
    // Printed relative to the project when the package lives under it, which
    // is the common case and keeps the command short enough to read; a hoisted
    // package keeps its absolute path so the command stays runnable.
    let root = package_root.map_or_else(
        || format!("node_modules/{module}"),
        |path| {
            path.strip_prefix(project_directory)
                .unwrap_or(path)
                .display()
                .to_string()
        },
    );
    format!(
        "solid-checker contract generate --package-root {root} \
  --output .solid-checker/contracts/{module}/solid-reactivity.json"
    )
}

/// Validates a discovered contract's identity, returning the refusal when it
/// describes a version other than the installed one.
///
/// A wrong *name* is still an error: a file claiming to be another package's
/// contract is malformed, not merely out of date. A wrong *version* is drift,
/// and drift is reported as an uncertifiable finding so one upgraded dependency
/// does not take the whole run down with it.
fn classify_identity(
    project_directory: &Path,
    module: &str,
    contract: &PackageContract,
) -> Result<Option<StaleContract>, BackendError> {
    validate_discovered_contract_name(module, contract)?;
    let installed = installed_package_manifest(project_directory, module)?;
    let installed = installed.as_ref().map(|(_, manifest)| manifest);
    if contract_matches_manifest(installed, contract) {
        return Ok(None);
    }
    Ok(Some(StaleContract {
        package: module.to_owned(),
        contract_path: contract.source_path.clone(),
        contract_version: contract.package.version.clone(),
        installed_version: installed_version(installed),
        bundled: false,
    }))
}

/// Reports imported packages whose own manifest indicates that they integrate
/// with Solid. General-purpose packages do not need reactive effect summaries,
/// so they are deliberately omitted from this preflight.
pub fn package_contract_statuses(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
) -> Result<Vec<PackageContractStatus>, BackendError> {
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    let explicit = explicit_paths
        .iter()
        .map(|path| read_package_contract(Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|contract| (contract.package.name.clone(), contract))
        .collect::<HashMap<_, _>>();
    let mut statuses = Vec::new();
    for module in imported_package_roots(facts) {
        // One walk and one manifest read per module: the installed version and
        // the solid-dependency probe both come from this single read, and every
        // tier below compares against it instead of reading it again.
        let installed = installed_package_manifest(project_directory, &module)?;
        let installed_manifest = installed.as_ref().map(|(_, manifest)| manifest);
        let package_directory = installed.as_ref().map(|(directory, _)| directory.clone());
        let bundled = (dialect.bundled_contract)(module.as_str())?;
        let is_bundled_package = bundled.is_some();
        let bundled_path = bundled
            .as_ref()
            .filter(|contract| contract_matches_manifest(installed_manifest, contract))
            .map(|contract| contract.source_path.as_str());
        let uses_solid = installed_manifest.is_some_and(manifest_uses_solid);
        if !is_bundled_package && !uses_solid {
            continue;
        }
        // A tier whose contract describes another version is reported as
        // `stale`, not raised as an error: this report exists to tell the user
        // which contracts need regenerating, so it must survive exactly the
        // drift that stops analysis.
        let classify = |contract: &PackageContract, fresh: &'static str| {
            if !contract_matches_manifest(installed_manifest, contract) {
                (
                    "stale",
                    Some(format!(
                        "the contract describes {} {}, but {} is installed",
                        contract.package.name,
                        contract.package.version,
                        installed_manifest
                            .map_or("another version", |manifest| manifest.version.as_str())
                    )),
                )
            } else if contract_evidence_is_certifiable(contract) {
                (fresh, None)
            } else {
                (
                    "unverified",
                    Some(format!(
                        "the contract's evidence is {:?}: its claims were generated, not reviewed",
                        contract.evidence.kind
                    )),
                )
            }
        };
        let local = discover_local_contract(project_directory, &module)?;
        let published = discover_contract(project_directory, &module)?;
        // `audited` is set only when the winning tier is the checker's own
        // bundled artifact, which the consumer cannot regenerate. Every other
        // tier is a file the project owns.
        let (status, detail, contract_path, audited) = if let Some(contract) = explicit.get(&module)
        {
            validate_discovered_contract_name(&module, contract)?;
            let (status, detail) = classify(contract, "explicit");
            (status, detail, contract.source_path.clone(), None)
        } else if let Some(path) = local {
            let contract = read_package_contract(&path)?;
            validate_discovered_contract_name(&module, &contract)?;
            let (status, detail) = classify(&contract, "local");
            (status, detail, contract.source_path, None)
        } else if let Some(path) = published {
            let contract = read_package_contract(&path)?;
            validate_discovered_contract_name(&module, &contract)?;
            let (status, detail) = classify(&contract, "published");
            (status, detail, contract.source_path, None)
        } else if let Some(path) = bundled_path {
            ("bundled", None, path.into(), None)
        } else if let Some(contract) = bundled.as_ref() {
            // The dialect ships a contract for this package, but audited
            // another version. That is staleness, not absence: reporting it as
            // a missing contract would point the user at a generation command
            // for a package whose contract they do not own.
            (
                "stale",
                Some(format!(
                    "this checker audited {module} {}, but {} is installed",
                    contract.package.version,
                    installed_manifest
                        .map_or("another version", |manifest| manifest.version.as_str())
                )),
                contract.source_path.clone(),
                Some(contract.package.version.as_str()),
            )
        } else {
            (
                "missing",
                None,
                local_contract_path(project_directory, &module)
                    .to_string_lossy()
                    .into_owned(),
                None,
            )
        };
        let remedy = contract_remedy(
            project_directory,
            status,
            &module,
            package_directory.as_deref(),
            audited,
        );
        statuses.push(PackageContractStatus {
            name: module,
            status: status.into(),
            detail,
            remedy,
            contract_path,
        });
    }
    Ok(statuses)
}

/// The action that resolves a non-certifying contract status, or `None` when
/// the status already certifies.
///
/// `unverified` deliberately has no regeneration command: generation never
/// promotes inferred claims, so re-running it would loop the user. The review
/// checklist is the actual next step.
fn contract_remedy(
    project_directory: &Path,
    status: &str,
    module: &str,
    package_directory: Option<&Path>,
    audited_bundled_version: Option<&str>,
) -> Option<String> {
    match status {
        // A bundled contract is the checker's own audited artifact. The
        // consumer cannot regenerate it, so the remedy names the two real
        // options instead of a command they should not run.
        "stale" if audited_bundled_version.is_some() => Some(format!(
            "install the audited version of {module}, or upgrade solid-checker to a release that \
             audits the installed one"
        )),
        "missing" | "stale" => Some(contract_regeneration_command(
            project_directory,
            module,
            package_directory,
        )),
        "unverified" => Some(format!(
            "review the generated checklist beside the contract and record reviewed evidence; \
             regenerating {module:?} will not promote its inferred claims"
        )),
        _ => None,
    }
}

/// As [`package_contract_statuses`], but classifies from an already-loaded
/// contract set instead of re-running contract discovery. Analysis loads the
/// contracts first, so this keeps the completeness check off a second
/// filesystem walk; only the per-package manifest probe remains. Each loaded
/// contract is the discovery winner for its package, so its source path
/// identifies the tier the original decision tree would have chosen.
pub fn package_contract_statuses_with(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
    contracts: &[PackageContract],
    stale_contracts: &[StaleContract],
) -> Result<Vec<PackageContractStatus>, BackendError> {
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    let stale_by_package = stale_contracts
        .iter()
        .map(|entry| (entry.package.as_str(), entry))
        .collect::<HashMap<_, _>>();
    let explicit_sources = explicit_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let by_name = contracts
        .iter()
        .map(|contract| (contract.package.name.as_str(), contract))
        .collect::<HashMap<_, _>>();
    let mut statuses = Vec::new();
    for module in imported_package_roots(facts) {
        let bundled = dialect.bundled_packages.contains(&module.as_str());
        let package_directory = discover_package_directory(project_directory, &module)?;
        let uses_solid = package_directory
            .as_deref()
            .map(package_uses_solid)
            .transpose()?
            .unwrap_or(false);
        if !bundled && !uses_solid {
            continue;
        }
        // A refusal recorded during loading wins: the package has a contract
        // file, it just describes another release, and saying "missing" would
        // send the user looking for a file that is already there.
        let (status, detail, contract_path) =
            if let Some(entry) = stale_by_package.get(module.as_str()) {
                (
                    "stale",
                    Some(if entry.bundled {
                        format!(
                            "this checker audited {module} {}, but {} is installed",
                            entry.contract_version, entry.installed_version
                        )
                    } else {
                        format!(
                            "the contract describes {module} {}, but {} is installed",
                            entry.contract_version, entry.installed_version
                        )
                    }),
                    entry.contract_path.clone(),
                )
            } else {
                let (status, contract_path) = match by_name.get(module.as_str()) {
                    Some(contract) if !contract_evidence_is_certifiable(contract) => {
                        ("unverified", contract.source_path.clone())
                    }
                    Some(contract) if explicit_sources.contains(contract.source_path.as_str()) => {
                        ("explicit", contract.source_path.clone())
                    }
                    Some(contract) if contract.source_path.starts_with("bundled://") => {
                        ("bundled", contract.source_path.clone())
                    }
                    Some(contract)
                        if Path::new(&contract.source_path)
                            == local_contract_path(project_directory, &module) =>
                    {
                        ("local", contract.source_path.clone())
                    }
                    Some(contract) => ("published", contract.source_path.clone()),
                    None => (
                        "missing",
                        local_contract_path(project_directory, &module)
                            .to_string_lossy()
                            .into_owned(),
                    ),
                };
                let detail = match status {
                "unverified" => by_name.get(module.as_str()).map(|contract| {
                    format!(
                        "the contract's evidence is {:?}: its claims were generated, not reviewed",
                        contract.evidence.kind
                    )
                }),
                _ => None,
            };
                (status, detail, contract_path)
            };
        let remedy = contract_remedy(
            project_directory,
            status,
            &module,
            package_directory.as_deref(),
            stale_by_package
                .get(module.as_str())
                .filter(|entry| entry.bundled)
                .map(|entry| entry.contract_version.as_str()),
        );
        statuses.push(PackageContractStatus {
            name: module,
            status: status.into(),
            detail,
            remedy,
            contract_path,
        });
    }
    Ok(statuses)
}

fn validate_discovered_contract_name(
    module: &str,
    contract: &PackageContract,
) -> Result<(), BackendError> {
    if contract.package.name != module {
        return Err(BackendError::Contract(format!(
            "contract discovered for package {module:?} declares package name {:?}",
            contract.package.name
        )));
    }
    Ok(())
}

fn package_uses_solid(directory: &Path) -> Result<bool, BackendError> {
    let manifest = match fs::read(directory.join("package.json")) {
        Ok(data) => serde_json::from_slice::<PackageManifest>(&data)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(manifest_uses_solid(&manifest))
}

/// As [`package_uses_solid`], for a manifest a caller has already read.
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
            .any(|name| name == "solid-js" || name.starts_with("@solidjs/"))
    })
}

fn discover_contract(directory: &Path, module: &str) -> Result<Option<PathBuf>, BackendError> {
    Ok(discover_package_directory(directory, module)?
        .map(|directory| directory.join("solid-reactivity.json"))
        .filter(|candidate| candidate.is_file()))
}

fn discover_package_directory(
    directory: &Path,
    module: &str,
) -> Result<Option<PathBuf>, BackendError> {
    for ancestor in directory.ancestors() {
        let candidate = ancestor.join("node_modules").join(module);
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() => return Ok(Some(candidate)),
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
        |rule| dialect::by_id("solid-v1").is_some_and(|dialect| (dialect.has_rule)(rule)),
    )
}

fn discover_rule_options_with(
    project: &Path,
    has_rule: impl Fn(&str) -> bool,
    owns_solid1x_options: impl Fn(&str) -> bool,
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
                return RuleOptions::parse_with_aliases(
                    &encoded,
                    &has_rule,
                    &owns_solid1x_options,
                    dialect::rule_alias,
                )
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

fn local_contract_path(project_directory: &Path, module: &str) -> PathBuf {
    project_directory
        .join(".solid-checker")
        .join("contracts")
        .join(module)
        .join("solid-reactivity.json")
}

fn discover_local_contract(
    project_directory: &Path,
    module: &str,
) -> Result<Option<PathBuf>, BackendError> {
    for ancestor in project_directory.ancestors() {
        let candidate = local_contract_path(ancestor, module);
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => return Ok(Some(candidate)),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}

pub fn read_package_contract(path: &Path) -> Result<PackageContract, BackendError> {
    let data = fs::read(path)?;
    let mut contract = decode_package_contract(&data).map_err(|error| {
        BackendError::Contract(format!(
            "decode package contract {}: {error}",
            path.display()
        ))
    })?;
    contract.source_path = path.canonicalize()?.to_string_lossy().into_owned();
    validate_contract_artifacts(path, &contract)?;
    Ok(contract)
}

fn decode_package_contract(data: &[u8]) -> Result<PackageContract, BackendError> {
    let mut contract = crate::contract_document::decode(data)?;
    contract.contract_hash = format!("sha256:{:x}", Sha256::digest(data));
    Ok(contract)
}

fn validate_contract_artifacts(
    contract_path: &Path,
    contract: &PackageContract,
) -> Result<(), BackendError> {
    let directory = contract_path.parent().unwrap_or_else(|| Path::new("."));
    for (name, artifact) in [
        ("declaration", contract.artifacts.declaration.as_ref()),
        ("implementation", contract.artifacts.implementation.as_ref()),
    ] {
        let Some(artifact) = artifact else {
            continue;
        };
        let relative = Path::new(&artifact.path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
        {
            return Err(BackendError::Contract(format!(
                "package contract {name} artifact path is invalid"
            )));
        }
        let data = fs::read(directory.join(relative)).map_err(|error| {
            BackendError::Contract(format!("read package contract {name} artifact: {error}"))
        })?;
        let actual = format!("sha256:{:x}", Sha256::digest(data));
        if actual != artifact.hash {
            return Err(BackendError::Contract(format!(
                "package contract {name} hash {:?} does not match artifact hash {actual:?}",
                artifact.hash
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use solid_facts::core::Generation;
    use solid_facts::{ProjectFacts, TypeScriptTable};

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
        };
        let mut session = DiagnosticSession::default();

        let (initial, initial_timings) = session
            .analyze_measured(Path::new(&facts.project_id), &[], &facts, &[], None)
            .unwrap();
        let (reused, reused_timings) = session
            .analyze_measured(Path::new(&facts.project_id), &[], &facts, &[], None)
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
        };
        let mut session = DiagnosticSession::default();

        let (_, baseline) = session
            .analyze_measured(Path::new(&facts.project_id), &[], &facts, &[], None)
            .unwrap();
        let (preset_analysis, preset_miss) = session
            .analyze_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &[],
                None,
                super::RequestedRuleEnablement {
                    presets: &["preferences".into()],
                    rules: &[],
                },
            )
            .unwrap();
        let (preset_reused, preset_hit) = session
            .analyze_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &[],
                None,
                super::RequestedRuleEnablement {
                    presets: &["preferences".into(), "preferences".into()],
                    rules: &[],
                },
            )
            .unwrap();
        let (_, rule_miss) = session
            .analyze_measured_with_enablement(
                Path::new(&facts.project_id),
                &[],
                &facts,
                &[],
                None,
                super::RequestedRuleEnablement {
                    presets: &[],
                    rules: &["prefer-show".into()],
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
}
