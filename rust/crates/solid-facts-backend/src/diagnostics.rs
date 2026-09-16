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
        let identity = DiagnosticIdentity {
            dialect: self.dialect.id,
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: vec![contracts.cache_fingerprint()],
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
        let metrics = analysis_metrics(facts, &program, contracts);
        let snapshot = snapshot_with_package_summaries(
            sources,
            accepted_package_summaries(contracts),
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

fn accepted_package_summaries(contracts: &AcceptedContractIndex) -> Vec<PackageSummary> {
    let summary = |package: &solid_reactive_ir::contract_semantics::PackageIdentity,
                   digest: &str| PackageSummary {
        name: package.name.clone(),
        version: package.version.clone(),
        contract_hash: digest.into(),
        evidence: "accepted".into(),
        exports_analyzed: 0,
    };
    let mut summaries = contracts
        .semantic_identity()
        .iter()
        .map(|binding| {
            summary(
                &binding.semantics.package,
                binding.semantics.semantic_digest.as_str(),
            )
        })
        // An acceptance admitted by artifact identity -- a project catalog
        // certified from another file, or a contract compiled into this
        // checker -- is one the analysis reads from. Reporting only the
        // importer-keyed ones said a project that analysed against a bundled
        // contract had accepted nothing.
        .chain(contracts.admitted_contracts().map(|(_, contract)| {
            summary(
                contract.package(),
                contract.semantic_identity().semantic_digest.as_str(),
            )
        }))
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        (&left.name, &left.version, &left.contract_hash).cmp(&(
            &right.name,
            &right.version,
            &right.contract_hash,
        ))
    });
    summaries.dedup_by(|left, right| {
        left.name == right.name
            && left.version == right.version
            && left.contract_hash == right.contract_hash
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

/// Reports external receipt coverage and the separately identified built-in
/// runtime foundation. Built-in model selection is not artifact certification.
/// Coverage is complete only when every exact imported specifier binds in the
/// already receipt-validated normalized index.
pub fn accepted_package_contract_statuses(
    dialect: &'static Dialect,
    project: &Path,
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
) -> Result<Vec<PackageContractStatus>, BackendError> {
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    let first_party = match dialect.id {
        "solid-v1" => &[
            "solid-js",
            "@solid-primitives/scheduled",
            "@solid-primitives/debounce",
            "@solid-primitives/rootless",
        ][..],
        "solid-v2" => &["solid-js", "@solidjs/web", "@solidjs/signals"][..],
        _ => &[][..],
    };
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
                status: if modeled { "builtin" } else { "unsupported-runtime" }.into(),
                installed_integrity: None,
                detail: Some(if modeled {
                    format!("built-in runtime model {}; no package receipt is required; model selection is not installed-artifact authentication", dialect.vocabulary.runtime_model_identity())
                } else {
                    format!("this core package is outside the {} runtime model", dialect.id)
                }),
                remedy: (!modeled).then(|| "select a compatible Solid dialect and runtime; a core package contract cannot extend the built-in model".into()),
                contract_path: String::new(),
            });
            continue;
        }
        let installed = installed_package_manifest(project_directory, &module)?;
        let manifest = installed.as_ref().map(|(_, manifest)| manifest);
        if !first_party.contains(&module.as_str()) && !manifest.is_some_and(manifest_uses_solid) {
            continue;
        }
        let package_directory = installed.as_ref().map(|(directory, _)| directory.as_path());
        let installed_integrity = package_directory
            .map(|directory| installed_package_integrity(project_directory, directory))
            .transpose()?
            .flatten();
        let imports = resolved
            .into_iter()
            .flat_map(|imports| imports.iter())
            .filter(|(_, import)| {
                import
                    .resolver_package_name
                    .as_deref()
                    .or(import.package_name.as_deref())
                    == Some(module.as_str())
            })
            .collect::<Vec<_>>();
        let bound = imports
            .iter()
            .filter(|(importer, import)| contracts.contract(importer, &import.text).is_ok())
            .count();
        let (status, detail, remedy, contract_path) = if !imports.is_empty()
            && bound == imports.len()
        {
            (
                "certified".into(),
                None,
                None,
                "receipt-issued stable-v1 index".into(),
            )
        } else {
            let status = if bound == 0 { "missing" } else { "unbound" };
            let detail = if imports.is_empty() {
                Some("exact import identity facts are unavailable".into())
            } else if bound == 0 {
                Some(format!(
                    "none of the {} exact imported artifact case(s) has a matching receipt",
                    imports.len()
                ))
            } else {
                Some(format!(
                    "{bound} of {} exact imported artifact case(s) have matching receipts",
                    imports.len()
                ))
            };
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
                        "solid-checker contract generate --package-root {root} --integrity {integrity} --output .solid-checker/contracts/{module}/solid-reactivity.json, then verify the proposal and add its receipt to .solid-checker/accepted-contracts.json"
                    ))
                },
            );
            (status.into(), detail, remedy, String::new())
        };
        statuses.push(PackageContractStatus {
            name: module,
            status,
            installed_integrity,
            detail,
            remedy,
            contract_path,
        });
    }
    statuses.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(statuses)
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
/// because its own installed artifact is the one that acceptance names.
///
/// Lives here rather than beside the catalog reader because the answer depends
/// on the installed tree, which is this module's business: the package the
/// specifier resolves to, its version, and the registry integrity its lockfile
/// selected. A package whose installs disagree yields no integrity at all
/// (`installed_package_integrity` returns `None`), so a project with two
/// versions of one dependency admits neither — which is the nested-install case
/// the importer key used to guard.
pub fn admitted_project_artifacts(
    catalogs: &[PathBuf],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    project_directory: &Path,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
) -> Result<Vec<(String, String)>, BackendError> {
    let installed = |specifier: &str| installed_artifact_identity(project_directory, specifier);
    let resolved_target =
        |specifier: &str| resolved_target_identity(project_directory, facts, specifier);
    crate::contract_interface::admitted_project_artifacts(
        catalogs,
        trust,
        project_directory,
        conditions,
        &installed,
        &resolved_target,
    )
    .map_err(|error| BackendError::Contract(error.to_string()))
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
/// differ is what happens afterwards.
pub fn project_accepted_contracts(
    directory: &Path,
    catalogs: &[PathBuf],
    trust: Option<&crate::contract_certification::Policy2TrustConfiguration>,
    bundled: bool,
    conditions: &std::collections::BTreeSet<String>,
    facts: &solid_facts::ProjectFacts,
    requirements: AcceptedContractIndex,
) -> Result<AcceptedContractIndex, BackendError> {
    let mut contracts = AcceptedContractIndex::default();
    for path in catalogs {
        contracts =
            crate::contract_interface::read_external_contract_catalog_with_trust(path, trust)
                .map_err(|error| BackendError::Contract(error.to_string()))?
                .with_fallback(contracts);
    }
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
    let project = admitted_project_artifacts(catalogs, trust, directory, conditions, facts)?;
    let project = agreed_admissions(&contracts, project);
    if !project.is_empty() {
        contracts = contracts.with_admitted_artifacts(project);
    }
    if bundled {
        let bundles = admitted_bundled_artifacts(directory, conditions, facts)?;
        let bundles = agreed_admissions(&contracts, bundles);
        if !bundles.is_empty() {
            contracts = contracts.with_admitted_artifacts(bundles);
        }
    }
    Ok(contracts)
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
    }
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
) -> Result<Vec<(String, String)>, BackendError> {
    let installed = |specifier: &str| installed_artifact_identity(project_directory, specifier);
    let resolved_target =
        |specifier: &str| resolved_target_identity(project_directory, facts, specifier);
    crate::accepted_bundles::admitted_bundle_artifacts(conditions, &installed, &resolved_target)
        .map_err(|error| BackendError::Contract(error.to_string()))
}

fn installed_artifact_identity(
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
fn resolved_target_identity(
    project_directory: &Path,
    facts: &solid_facts::ProjectFacts,
    specifier: &str,
) -> Option<String> {
    let module = package_name_of_specifier(specifier)?;
    let (directory, _) = installed_package_manifest(project_directory, &module).ok()??;
    let root = fs::canonicalize(&directory).ok()?;
    let attested = facts.resolved_imports.as_ref()?;
    let mut selected: Option<String> = None;
    for (_, import) in attested.iter() {
        if import.text.as_str() != specifier
            || import.resolution == solid_facts::ImportResolution::Unresolved
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
    match fs::read(directory.join("package.json")) {
        Ok(data) => Ok(Some((directory, serde_json::from_slice(&data)?))),
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

fn parse_json_with_trailing_commas<T: DeserializeOwned>(data: &[u8]) -> Option<T> {
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

fn bun_package_integrity(package_directory: &Path, data: &[u8]) -> Option<String> {
    let lockfile = parse_json_with_trailing_commas::<BunLockfile>(data)?;
    if lockfile.lockfile_version != 2 {
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
    let mut found = None;
    for (key, record) in lockfile.packages {
        let identifier = record.first().and_then(serde_json::Value::as_str);
        if key != name
            && key != expected_identifier
            && identifier != Some(expected_identifier.as_str())
        {
            continue;
        }
        let Some(integrity) = record.get(3).and_then(serde_json::Value::as_str) else {
            continue;
        };
        if integrity.is_empty() {
            continue;
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
            .any(|name| name == "solid-js" || name.starts_with("@solidjs/"))
    })
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
    let rule = if dialect.id == "solid-v1" {
        "v1/prefer-for"
    } else {
        "prefer-for"
    };
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

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use solid_facts::core::Generation;
    use solid_facts::{ProjectFacts, TypeScriptTable};
    use solid_reactive_ir::{RuntimeEnvironment, contract_semantics::AcceptedContractIndex};

    use super::{
        DiagnosticSession, agreed_admissions, installed_package_integrity, retain_enabled,
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
