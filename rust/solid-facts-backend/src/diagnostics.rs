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
use solid_reactive_ir::{CacheRetention, IncrementalBuilder, PackageContract, Program};
use solid_reactive_solver::{Finding, Rule};

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DiagnosticIdentity {
    project_id: String,
    generation: u64,
    contracts: Vec<String>,
    explicit_contract_paths: Vec<String>,
}

struct RetainedDiagnostic {
    identity: DiagnosticIdentity,
    analysis: Arc<DiagnosticAnalysis>,
}

/// Retains the complete diagnostic result for one coherent project
/// generation. The session owns IR cache policy, solving, and snapshot
/// construction so callers cannot accidentally select a slower fresh path.
#[derive(Default)]
pub struct DiagnosticSession {
    builder: IncrementalBuilder,
    retained: Option<RetainedDiagnostic>,
}

impl DiagnosticSession {
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

    pub fn analyze_measured(
        &mut self,
        project: &Path,
        sources: &[SourceFile],
        facts: &ProjectFacts,
        explicit_contract_paths: &[String],
        bundled_solid_js: Option<PackageContract>,
    ) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
        let ir_started = Instant::now();
        let contracts =
            load_package_contracts_with(project, facts, explicit_contract_paths, bundled_solid_js)?;
        let identity = DiagnosticIdentity {
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: contracts
                .iter()
                .map(|contract| format!("{contract:?}"))
                .collect(),
            explicit_contract_paths: explicit_contract_paths.to_vec(),
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

        let (program, _) = self
            .builder
            .build_with_contracts_shared(facts, &contracts)?;
        let reactive_ir = ir_started.elapsed();
        let solve_started = Instant::now();
        let analysis = Arc::new(finish_analysis(
            project,
            sources,
            facts,
            explicit_contract_paths,
            contracts,
            program,
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
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
) -> Result<Arc<DiagnosticAnalysis>, BackendError> {
    analyze_project_measured(project, sources, facts, explicit_contract_paths)
        .map(|(analysis, _)| analysis)
}

pub fn analyze_project_measured(
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    analyze_project_measured_with(project, sources, facts, explicit_contract_paths, None)
}

/// As [`analyze_project_measured`], but reuses a bundled solid-js contract the
/// caller already decoded (see [`bundled_solid_js_contract`]) instead of
/// decoding the compile-time-embedded JSON on the analysis path. The cold
/// path decodes it while the service builds the program; the preloaded value
/// is ignored when the project does not import solid-js.
pub fn analyze_project_measured_with(
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
) -> Result<(Arc<DiagnosticAnalysis>, DiagnosticTimings), BackendError> {
    DiagnosticSession::default().analyze_measured(
        project,
        sources,
        facts,
        explicit_contract_paths,
        bundled_solid_js,
    )
}

fn finish_analysis(
    project: &Path,
    sources: &[SourceFile],
    facts: &ProjectFacts,
    explicit_contract_paths: &[String],
    contracts: Vec<PackageContract>,
    program: Arc<Program>,
) -> Result<DiagnosticAnalysis, BackendError> {
    let statuses =
        package_contract_statuses_with(project, facts, explicit_contract_paths, &contracts)?;
    let missing_contracts = statuses
        .iter()
        .filter(|status| matches!(status.status.as_str(), "missing" | "unverified"))
        .collect::<Vec<_>>();
    let mut metrics = analysis_metrics(facts, &program, &contracts);
    metrics.proof_obligations += missing_contracts.len();
    metrics.unresolved_obligations += missing_contracts.len();
    let mut findings = solid_reactive_solver::solve(&program);
    findings.extend(missing_contracts.into_iter().map(|status| {
        let location = facts
            .files
            .iter()
            .find_map(|file| {
                file.ast
                    .imports
                    .iter()
                    .find(|import| package_root(&import.module) == status.name)
                    .map(|import| typefacts::Location {
                        path: file.path.as_str().to_owned().into(),
                        start_byte: u64::from(import.span.start),
                        end_byte: u64::from(import.span.end),
                    })
            })
            .unwrap_or_else(|| typefacts::Location {
                path: project.to_string_lossy().into_owned().into(),
                start_byte: 0,
                end_byte: 0,
            });
        Finding {
            analysis_context: "package contract completeness".into(),
            subject_kind: "package".into(),
            hint: if status.status == "unverified" {
                "Verify the generated contract against the exact package artifacts and behavioral probes, then record verified, reviewed, or attested evidence.".into()
            } else {
                format!(
                    "Create a local contract at {}, or pass one explicitly with --contract <PATH>. If you maintain {}, ship solid-reactivity.json in the package root so every consumer gets it. See docs/package-contracts.md for the format.",
                    status.contract_path, status.name
                )
            },
            ..Finding::new(
                Rule::PackageContractMissing,
                format!(
                    "imported Solid package {:?} {}; solid-checker cannot rely on its export summaries, so every use of them is uncertifiable",
                    status.name,
                    if status.status == "unverified" {
                        "has only an unverified generated reactivity contract"
                    } else {
                        "has no reactivity contract"
                    }
                ),
                location,
            )
        }
    }));
    let snapshot = snapshot(sources, &contracts, metrics, findings);
    Ok(DiagnosticAnalysis {
        program,
        contracts,
        snapshot,
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
            let Some(contract) = contracts
                .iter()
                .filter(|contract| {
                    import.module == contract.package.name
                        || import
                            .module
                            .strip_prefix(&contract.package.name)
                            .is_some_and(|suffix| suffix.starts_with('/'))
                })
                .max_by_key(|contract| contract.package.name.len())
            else {
                continue;
            };
            for binding in &import.bindings {
                if binding.kind == solid_facts::solid_ast_facts::ImportKind::Namespace {
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
        + program.unresolved_cleanup_returns.len();
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
            + program.invalid_cleanup_returns.len()
            + program.unresolved_cleanup_returns.len()
            + program.directive_creations.len()
            + program.static_violations.len(),
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
pub fn bundled_solid_js_contract() -> Result<PackageContract, BackendError> {
    let mut bundled = decode_package_contract(include_bytes!(
        "../../../pkg/contracts/bundled/solid-js.json"
    ))?;
    bundled.source_path = "bundled://solid-js.json".into();
    Ok(bundled)
}

fn bundled_solidjs_web_contract() -> Result<PackageContract, BackendError> {
    let mut bundled = decode_package_contract(include_bytes!(
        "../../../pkg/contracts/bundled/solidjs-web.json"
    ))?;
    bundled.source_path = "bundled://solidjs-web.json".into();
    Ok(bundled)
}

pub fn load_package_contracts(
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
) -> Result<Vec<PackageContract>, BackendError> {
    load_package_contracts_with(project, facts, explicit_paths, None)
}

/// As [`load_package_contracts`], but reuses a preloaded bundled solid-js
/// contract instead of decoding the embedded JSON. The preloaded value is used
/// only when the project imports solid-js; the discovery walk and explicit
/// overrides are unchanged, so the resolved contract set is identical.
pub fn load_package_contracts_with(
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
    bundled_solid_js: Option<PackageContract>,
) -> Result<Vec<PackageContract>, BackendError> {
    let mut contracts = HashMap::<String, PackageContract>::new();
    let modules = imported_package_roots(facts);
    let modules = modules.iter().map(String::as_str).collect::<HashSet<_>>();
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
    if modules.contains("solid-js") {
        let bundled = match bundled_solid_js {
            Some(bundled) => bundled,
            None => bundled_solid_js_contract()?,
        };
        if contract_matches_installed_package(project_directory, "solid-js", &bundled)? {
            contracts.insert(bundled.package.name.clone(), bundled);
        }
    }
    if modules.contains("@solidjs/web") {
        let bundled = bundled_solidjs_web_contract()?;
        if contract_matches_installed_package(project_directory, "@solidjs/web", &bundled)? {
            contracts.insert(bundled.package.name.clone(), bundled);
        }
    }
    for module in &modules {
        if let Some(path) = discover_contract(project_directory, module)? {
            let contract = read_package_contract(&path)?;
            validate_contract_identity(project_directory, module, &contract)?;
            contracts.insert(contract.package.name.clone(), contract);
        }
    }
    for module in &modules {
        if let Some(path) = discover_local_contract(project_directory, module)? {
            let contract = read_package_contract(&path)?;
            validate_contract_identity(project_directory, module, &contract)?;
            contracts.insert(contract.package.name.clone(), contract);
        }
    }
    for path in explicit_paths {
        let contract = read_package_contract(Path::new(path))?;
        if modules.contains(contract.package.name.as_str()) {
            validate_contract_identity(
                project_directory,
                contract.package.name.as_str(),
                &contract,
            )?;
        }
        contracts.insert(contract.package.name.clone(), contract);
    }
    let mut contracts = contracts.into_values().collect::<Vec<_>>();
    contracts.sort_by(|left, right| left.package.name.cmp(&right.package.name));
    Ok(contracts)
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
    pub contract_path: String,
}

fn contract_evidence_is_certifiable(contract: &PackageContract) -> bool {
    matches!(
        contract.evidence.kind.as_str(),
        "verified" | "reviewed" | "trusted" | "attested"
    )
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

fn contract_matches_installed_package(
    project_directory: &Path,
    module: &str,
    contract: &PackageContract,
) -> Result<bool, BackendError> {
    let Some(directory) = discover_package_directory(project_directory, module)? else {
        // Ambient-module and virtual test projects have no package manifest.
        return Ok(true);
    };
    let manifest = match fs::read(directory.join("package.json")) {
        Ok(data) => serde_json::from_slice::<PackageManifest>(&data)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(error.into()),
    };
    Ok(manifest.version == contract.package.version)
}

fn validate_contract_identity(
    project_directory: &Path,
    module: &str,
    contract: &PackageContract,
) -> Result<(), BackendError> {
    validate_discovered_contract_name(module, contract)?;
    if !contract_matches_installed_package(project_directory, module, contract)? {
        let directory = discover_package_directory(project_directory, module)?
            .expect("version mismatch requires an installed package");
        let manifest: PackageManifest =
            serde_json::from_slice(&fs::read(directory.join("package.json"))?)?;
        return Err(BackendError::Contract(format!(
            "contract for {module:?} declares version {:?}, but the installed package is version {:?}",
            contract.package.version, manifest.version
        )));
    }
    Ok(())
}

/// Reports imported packages whose own manifest indicates that they integrate
/// with Solid. General-purpose packages do not need reactive effect summaries,
/// so they are deliberately omitted from this preflight.
pub fn package_contract_statuses(
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
        let bundled = match module.as_str() {
            "solid-js" => Some(bundled_solid_js_contract()?),
            "@solidjs/web" => Some(bundled_solidjs_web_contract()?),
            _ => None,
        };
        let is_bundled_package = bundled.is_some();
        let bundled_path = if let Some(contract) = bundled.as_ref()
            && contract_matches_installed_package(project_directory, &module, contract)?
        {
            Some(contract.source_path.as_str())
        } else {
            None
        };
        let package_directory = discover_package_directory(project_directory, &module)?;
        let uses_solid = package_directory
            .as_deref()
            .map(package_uses_solid)
            .transpose()?
            .unwrap_or(false);
        if !is_bundled_package && !uses_solid {
            continue;
        }
        let local = discover_local_contract(project_directory, &module)?;
        let published = discover_contract(project_directory, &module)?;
        let (status, contract_path) = if let Some(contract) = explicit.get(&module) {
            validate_contract_identity(project_directory, &module, contract)?;
            (
                if contract_evidence_is_certifiable(contract) {
                    "explicit"
                } else {
                    "unverified"
                },
                contract.source_path.clone(),
            )
        } else if let Some(path) = local {
            let contract = read_package_contract(&path)?;
            validate_contract_identity(project_directory, &module, &contract)?;
            (
                if contract_evidence_is_certifiable(&contract) {
                    "local"
                } else {
                    "unverified"
                },
                contract.source_path,
            )
        } else if let Some(path) = published {
            let contract = read_package_contract(&path)?;
            validate_contract_identity(project_directory, &module, &contract)?;
            (
                if contract_evidence_is_certifiable(&contract) {
                    "published"
                } else {
                    "unverified"
                },
                contract.source_path,
            )
        } else if let Some(path) = bundled_path {
            ("bundled", path.into())
        } else {
            (
                "missing",
                local_contract_path(project_directory, &module)
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        statuses.push(PackageContractStatus {
            name: module,
            status: status.into(),
            contract_path,
        });
    }
    Ok(statuses)
}

/// As [`package_contract_statuses`], but classifies from an already-loaded
/// contract set instead of re-running contract discovery. Analysis loads the
/// contracts first, so this keeps the completeness check off a second
/// filesystem walk; only the per-package manifest probe remains. Each loaded
/// contract is the discovery winner for its package, so its source path
/// identifies the tier the original decision tree would have chosen.
pub fn package_contract_statuses_with(
    project: &Path,
    facts: &ProjectFacts,
    explicit_paths: &[String],
    contracts: &[PackageContract],
) -> Result<Vec<PackageContractStatus>, BackendError> {
    let project_directory = project
        .parent()
        .ok_or_else(|| BackendError::Contract("tsconfig has no parent".into()))?;
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
        let bundled = matches!(module.as_str(), "solid-js" | "@solidjs/web");
        let package_directory = discover_package_directory(project_directory, &module)?;
        let uses_solid = package_directory
            .as_deref()
            .map(package_uses_solid)
            .transpose()?
            .unwrap_or(false);
        if !bundled && !uses_solid {
            continue;
        }
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
        statuses.push(PackageContractStatus {
            name: module,
            status: status.into(),
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
    Ok([
        manifest.dependencies,
        manifest.peer_dependencies,
        manifest.optional_dependencies,
    ]
    .iter()
    .any(|dependencies| {
        dependencies
            .keys()
            .any(|name| name == "solid-js" || name.starts_with("@solidjs/"))
    }))
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

    use solid_facts::{ProjectFacts, TypeScriptTable};
    use solid_facts_core::Generation;

    use super::DiagnosticSession;

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
}
