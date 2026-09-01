mod daemon_cache;
mod idle_memory;
mod json_output;
mod snapshot_emission;

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_facts_backend::{
    BackendError, ImportIdentityMeasurement, RequestedRuleEnablement, SemanticDemandOptions,
    SourceFile, TypeFactsProvider, TypeFactsSession, accepted_package_contract_statuses,
    analyze_project_accepted_measured_with_enablement, attest_import_identities,
    build_project_native_measured_with_demands, bundled_first_party_contract_index,
    contract_identity_scope, default_typefacts_executable, dialect,
    encode_inferred_entrypoint_workflow_with_external_targets, merge_contract_proposals,
    merge_plans, read_accepted_contract_catalog_with_trust, read_policy2_trust_configuration,
    read_proposal_dependency_catalog_for_generation, review_contract_document,
    semantic_demand_options_for_enablement, validate_contract_document,
};
use solid_reactive_ir::{RuntimeBuild, RuntimeEnvironment, RuntimeRendering, RuntimeTarget};

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
    /// Host-acquired exact imports paired with stable-v1 main documents
    /// and proof-issued receipts.
    #[serde(default)]
    accepted_contract_catalog: String,
    /// Host-selected receipt trust configuration outside the analyzed
    /// project. A project catalog cannot nominate its own policy-2 issuer.
    #[serde(default)]
    receipt_trust_configuration: String,
    /// Private open-proposal semantics used only while emitting another node
    /// in one graph transaction. This is never ordinary accepted-contract
    /// authority and cannot be combined with a receipt-bearing catalog.
    #[serde(default)]
    proposal_dependency_catalog: String,
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
    /// Private generator optimization: compatible exact artifact targets are
    /// inferred from one shared Type Facts project, then emitted separately.
    #[serde(default)]
    emit_contract_batch: String,
    #[serde(default)]
    contract_batch_results: String,
    /// Private, non-contract input recipes for the runtime probe. This is a
    /// sidecar because constructing an argument is not evidence of behavior.
    /// Generator-owned declaration query used only to derive and validate
    /// probe construction recipes. It never enters contract inference.
    #[serde(default)]
    declaration_probe_plan: String,
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
    /// Full Phase-7 exact package resolution for the artifact being emitted.
    #[serde(default)]
    contract_resolution: String,
    #[serde(default)]
    emit_proposal_plan: String,
    #[serde(default)]
    merge_contract_paths: Vec<String>,
    #[serde(default)]
    merge_contract_output: String,
    #[serde(default)]
    merge_proposal_plan_paths: Vec<String>,
    #[serde(default)]
    merge_proposal_plan_output: String,
    #[serde(default)]
    review_contract: String,
    #[serde(default)]
    review_output: String,
    #[serde(default)]
    runtime_probe_proposal: String,
    #[serde(default)]
    runtime_probe_proposal_plan: String,
    #[serde(default)]
    runtime_probe_request: String,
    #[serde(default)]
    runtime_probe_plan_output: String,
    #[serde(default)]
    runtime_probe_runs: String,
    #[serde(default)]
    runtime_probe_evaluation_output: String,
    /// Node-owned acquisition request for policy-2 planning. Rust reads the
    /// proposal, registry metadata, archive, and exact resolution once, then
    /// emits only verifier-derived demand identities. This is planning, never
    /// receipt authority.
    #[serde(default)]
    plan_contract_certification: String,
    #[serde(default)]
    certification_plan_output: String,
    /// Complete opaque value-only policy-2 transaction.
    #[serde(default)]
    execute_contract_certification: String,
    /// Fresh-process ordinary discovery postcondition for the transaction.
    #[serde(default)]
    verify_policy2_discovery: String,
    #[serde(default)]
    package_name: String,
    #[serde(default)]
    package_version: String,
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationPlanningRequest {
    schema_version: u16,
    proposal: String,
    resolution: solid_facts_backend::ResolvedImport,
    #[serde(default)]
    export_conditions: Vec<String>,
    registry_origin: String,
    registry_metadata: String,
    archive: String,
    /// Declaration-only packages whose authenticated bytes the witness program
    /// needs in order to resolve this package's cross-package type references.
    /// Graph nodes carry the same set one level up, on the node request, so a
    /// planning nested inside a graph node must leave this empty.
    #[serde(default)]
    source_dependencies: Vec<ContractCertificationSourceRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationExecutionRequest {
    schema_version: u16,
    #[serde(default)]
    planning: Option<ContractCertificationPlanningRequest>,
    #[serde(default)]
    plannings: Vec<ContractCertificationPlanningRequest>,
    #[serde(default)]
    graph: Option<ContractCertificationGraphRequest>,
    #[serde(default)]
    graphs: Vec<ContractCertificationGraphRequest>,
    #[serde(default)]
    graph_case_set: Option<ContractCertificationGraphCaseSetRequest>,
    typefacts_executable: String,
    issuer_configuration: String,
    catalog_root: String,
    trust_configuration_output: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationGraphRequest {
    root: ContractCertificationGraphNodeRequest,
    dependencies: Vec<ContractCertificationGraphNodeRequest>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationGraphNodeRequest {
    planning: ContractCertificationPlanningRequest,
    lockfile: String,
    lock_locator: String,
    #[serde(default)]
    source_dependencies: Vec<ContractCertificationSourceRequest>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationGraphCaseSetRequest {
    nodes: Vec<ContractCertificationGraphCaseSetNodeRequest>,
    cases: Vec<ContractCertificationGraphCaseSetCase>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationGraphCaseSetNodeRequest {
    key: String,
    planning: ContractCertificationPlanningRequest,
    lockfile: String,
    lock_locator: String,
    #[serde(default)]
    source_dependencies: Vec<ContractCertificationSourceRequest>,
}

impl ContractCertificationGraphCaseSetNodeRequest {
    fn graph_node(self) -> ContractCertificationGraphNodeRequest {
        ContractCertificationGraphNodeRequest {
            planning: self.planning,
            lockfile: self.lockfile,
            lock_locator: self.lock_locator,
            source_dependencies: self.source_dependencies,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationGraphCaseSetCase {
    root: String,
    nodes: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractCertificationSourceRequest {
    package_name: String,
    package_version: String,
    registry_origin: String,
    registry_metadata: String,
    archive: String,
    lockfile: String,
    lock_locator: String,
    installed_package_root: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractEmissionBatchDocument {
    schema_version: u16,
    targets: Vec<ContractEmissionBatchTarget>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractEmissionBatchTarget {
    index: usize,
    output: String,
    plan: String,
    resolution: String,
    entry_file: String,
    source_files: Vec<String>,
}

/// Every input that can change the fact program shared by one private
/// contract-emission batch. This key is deliberately request-local: it is
/// never persisted and cannot cross into certification or fresh-process
/// replay.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ContractEmissionFactContext {
    dialect: String,
    typefacts_project: String,
    typefacts_executable: String,
    typefacts_arguments: Vec<String>,
    generation: u64,
    semantic_demands: SemanticDemandOptions,
    runtime: RuntimeEnvironment,
    presets: Vec<String>,
    enabled_rules: Vec<String>,
}

/// One source is identified both by its independently canonicalized path and
/// by the complete source record consumed by fact construction. The latter
/// binds source bytes, the analyzer-visible spelling, and all Solid compiler
/// options; equal paths alone are never enough to share facts.
#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalContractEmissionSource {
    canonical_path: PathBuf,
    source: SourceFile,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContractEmissionFactProgramKey {
    context: ContractEmissionFactContext,
    sources: Vec<CanonicalContractEmissionSource>,
}

fn contract_emission_target_sources(
    target: &ContractEmissionBatchTarget,
    sources_by_path: &HashMap<PathBuf, SourceFile>,
) -> Result<Vec<CanonicalContractEmissionSource>, Box<dyn std::error::Error>> {
    let mut selected = BTreeSet::new();
    let mut sources = Vec::with_capacity(target.source_files.len());
    for source_file in &target.source_files {
        let canonical_path = Path::new(source_file).canonicalize()?;
        if !selected.insert(canonical_path.clone()) {
            continue;
        }
        let source = sources_by_path
            .get(&canonical_path)
            .ok_or_else(|| {
                format!(
                    "contract emission batch target {} names source outside its configured project: {}",
                    target.index, source_file
                )
            })?
            .clone();
        sources.push(CanonicalContractEmissionSource {
            canonical_path,
            source,
        });
    }
    sources.sort_by(|left, right| left.canonical_path.cmp(&right.canonical_path));
    Ok(sources)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractEmissionBatchOutcome {
    index: usize,
    success: bool,
    duration_ns: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

const CONTRACT_EMISSION_BATCH_TARGET_LIMIT: usize = 1_024;

fn read_contract_emission_batch(
    request: &Request,
) -> Result<ContractEmissionBatchDocument, Box<dyn std::error::Error>> {
    let batch: ContractEmissionBatchDocument =
        serde_json::from_slice(&fs::read(&request.emit_contract_batch)?)?;
    if batch.schema_version != 1 || batch.targets.is_empty() {
        return Err("contract emission batch must use schemaVersion 1 and contain targets".into());
    }
    if batch.targets.len() > CONTRACT_EMISSION_BATCH_TARGET_LIMIT {
        return Err(format!(
            "contract emission batch contains {} targets, exceeding the resource limit of {}",
            batch.targets.len(),
            CONTRACT_EMISSION_BATCH_TARGET_LIMIT
        )
        .into());
    }
    let mut indexes = BTreeSet::new();
    let mut write_paths = BTreeSet::new();
    write_paths.insert(request.contract_batch_results.clone());
    for target in &batch.targets {
        if !indexes.insert(target.index) {
            return Err(format!(
                "contract emission batch contains duplicate target index {}",
                target.index
            )
            .into());
        }
        if target.output.is_empty()
            || target.plan.is_empty()
            || target.resolution.is_empty()
            || target.entry_file.is_empty()
            || target.source_files.is_empty()
        {
            return Err(format!(
                "contract emission batch target {} has an empty required path",
                target.index
            )
            .into());
        }
        if !write_paths.insert(target.output.clone()) || !write_paths.insert(target.plan.clone()) {
            return Err("contract emission batch write paths must be unique".into());
        }
    }
    Ok(batch)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractReviewForCaseSet {
    artifact_cases: Vec<ContractReviewArtifactCaseForCaseSet>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractReviewArtifactCaseForCaseSet {
    id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy2CaseSetEntry {
    artifact_case_id: String,
    importer: String,
    specifier: String,
    resolved_import_root: String,
    semantic_digest: String,
    receipt_digest: String,
    catalog: String,
    catalog_digest: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy2CaseSetDocument {
    format: String,
    case_set_version: u16,
    cases: Vec<Policy2CaseSetEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy2CaseSetPointer {
    format: String,
    case_set_version: u16,
    document: String,
    document_digest: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Policy2CaseCoordinate {
    artifact_case_id: String,
    importer: String,
    specifier: String,
    resolved_import_root: String,
}

impl From<&Policy2CaseSetEntry> for Policy2CaseCoordinate {
    fn from(entry: &Policy2CaseSetEntry) -> Self {
        Self {
            artifact_case_id: entry.artifact_case_id.clone(),
            importer: entry.importer.clone(),
            specifier: entry.specifier.clone(),
            resolved_import_root: entry.resolved_import_root.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReceiptIssuerConfigurationDocument {
    format: String,
    issuer_configuration_version: u16,
    kind: solid_facts_backend::ReceiptIssuerKind,
    scope: String,
    seed: String,
    #[serde(default)]
    revocation_epoch: u64,
}

fn certification_plan_from_request(
    request: ContractCertificationPlanningRequest,
) -> Result<(solid_facts_backend::CertificationPlan, Vec<u8>), Box<dyn std::error::Error>> {
    let mut transaction = solid_facts_backend::CertificationPlanningTransaction::new();
    certification_plan_from_request_in(&mut transaction, request)
}

fn certification_plan_from_request_in(
    transaction: &mut solid_facts_backend::CertificationPlanningTransaction,
    request: ContractCertificationPlanningRequest,
) -> Result<(solid_facts_backend::CertificationPlan, Vec<u8>), Box<dyn std::error::Error>> {
    if request.schema_version != 1 {
        return Err(format!(
            "unsupported certification planning request version {}; expected 1",
            request.schema_version
        )
        .into());
    }
    let proposal = fs::read(&request.proposal).map_err(|error| {
        format!(
            "could not read certification proposal {}: {error}",
            request.proposal
        )
    })?;
    let import_request = solid_facts_backend::ImportRequest {
        specifier: request.resolution.specifier.clone(),
        importer: request.resolution.importer.clone(),
        export_conditions: request.export_conditions,
    };
    let archive = solid_facts_backend::PublishedArchive::new(
        request.registry_origin,
        request.resolution.package_name.clone(),
        request.resolution.package_version.clone(),
        fs::read(&request.registry_metadata).map_err(|error| {
            format!(
                "could not read certification registry metadata {}: {error}",
                request.registry_metadata
            )
        })?,
        fs::read(&request.archive).map_err(|error| {
            format!(
                "could not read certification package archive {}: {error}",
                request.archive
            )
        })?,
    )?;
    // Root-path sources are evidence supply only, so one that cannot even be
    // assembled from its local bytes is dropped for the same reason Rust drops
    // one that will not authenticate: see `plan_contract_document_with_sources`.
    let sources = request
        .source_dependencies
        .into_iter()
        .filter_map(|source| certification_source_request(source).ok())
        .collect();
    let plan = transaction.plan_contract_document_with_sources(
        &proposal,
        import_request,
        request.resolution,
        solid_facts_backend::UntrustedArtifactEnvelope::Published(archive),
        sources,
    )?;
    Ok((plan, proposal))
}

/// Reads every declaration-only source archive named by a request into the
/// authenticated acquisition type. Nothing here consults an installed tree:
/// the archive bytes, the registry metadata, and the lockfile are the only
/// inputs, and Rust replays the lock selection against the archive it decoded.
fn certification_source_request(
    source: ContractCertificationSourceRequest,
) -> Result<solid_facts_backend::PublishedGraphSourceRequest, Box<dyn std::error::Error>> {
    let lock = solid_facts_backend::PublishedGraphLockSelection::from_bun_lock(
        &fs::read(&source.lockfile)?,
        source.lock_locator,
        source.package_name.clone(),
        source.package_version.clone(),
    )?;
    let archive = solid_facts_backend::PublishedArchive::new(
        source.registry_origin,
        source.package_name,
        source.package_version,
        fs::read(source.registry_metadata)?,
        fs::read(source.archive)?,
    )?;
    Ok(solid_facts_backend::PublishedGraphSourceRequest::new(
        archive,
        lock,
        source.installed_package_root,
    ))
}

fn certification_graph_node_from_request(
    request: ContractCertificationGraphNodeRequest,
) -> Result<solid_facts_backend::PublishedGraphNodeRequest, Box<dyn std::error::Error>> {
    let ContractCertificationGraphNodeRequest {
        planning,
        lockfile,
        lock_locator,
        source_dependencies,
    } = request;
    if planning.schema_version != 1 {
        return Err(format!(
            "unsupported graph planning request version {}; expected 1",
            planning.schema_version
        )
        .into());
    }
    // A graph node names its declaration-only sources once, on the node. A
    // second set nested in the planning would be silently ignored here, so
    // refuse it rather than accept two disagreeing declarations of the same
    // authenticated closure.
    if !planning.source_dependencies.is_empty() {
        return Err(
            "a graph node's planning must not carry its own declaration-only source set".into(),
        );
    }
    let proposal = fs::read(&planning.proposal)?;
    let import_request = solid_facts_backend::ImportRequest {
        specifier: planning.resolution.specifier.clone(),
        importer: planning.resolution.importer.clone(),
        export_conditions: planning.export_conditions,
    };
    let archive = solid_facts_backend::PublishedArchive::new(
        planning.registry_origin,
        planning.resolution.package_name.clone(),
        planning.resolution.package_version.clone(),
        fs::read(&planning.registry_metadata)?,
        fs::read(&planning.archive)?,
    )?;
    let lock = solid_facts_backend::PublishedGraphLockSelection::from_bun_lock(
        &fs::read(lockfile)?,
        lock_locator,
        planning.resolution.package_name.clone(),
        planning.resolution.package_version.clone(),
    )?;
    let sources = source_dependencies
        .into_iter()
        .map(certification_source_request)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        solid_facts_backend::PublishedGraphNodeRequest::from_document_with_sources(
            &proposal,
            import_request,
            planning.resolution,
            archive,
            lock,
            sources,
        )?,
    )
}

fn certification_graph_from_request(
    request: ContractCertificationGraphRequest,
) -> Result<solid_facts_backend::PublishedContractGraphPlan, Box<dyn std::error::Error>> {
    let mut transaction = solid_facts_backend::CertificationPlanningTransaction::new();
    certification_graph_from_request_in(&mut transaction, request)
}

fn certification_graph_from_request_in(
    transaction: &mut solid_facts_backend::CertificationPlanningTransaction,
    request: ContractCertificationGraphRequest,
) -> Result<solid_facts_backend::PublishedContractGraphPlan, Box<dyn std::error::Error>> {
    let root = certification_graph_node_from_request(request.root)
        .map_err(|error| format!("root graph planning input failed: {error}"))?;
    let dependencies = request
        .dependencies
        .into_iter()
        .map(certification_graph_node_from_request)
        .collect::<Result<Vec<_>, _>>()?;
    transaction
        .plan_published_contract_graph(root, dependencies)
        .map_err(|error| format!("published graph planning failed: {error}").into())
}

fn write_contract_certification_plan(
    request_path: &Path,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let request: ContractCertificationPlanningRequest =
        serde_json::from_slice(&fs::read(request_path)?)?;
    let (plan, _) = certification_plan_from_request(request)?;
    let graph = plan.demand_graph();
    let snapshot_witnesses = plan
        .artifact_witness_bindings()
        .iter()
        .map(|witness| witness.demand_id())
        .collect::<BTreeSet<_>>();
    let output = serde_json::json!({
        "format": "solid-checker-contract-certification-plan",
        "planVersion": 1,
        "policyDigest": graph.policy_digest().as_str(),
        "selectedArtifactCase": plan.selected_artifact_case_id(),
        "candidateSemanticDigest": graph.candidate_semantic_digest().as_str(),
        "snapshotRoot": graph.snapshot_root().as_str(),
        "provenanceRoot": graph.provenance_root().as_str(),
        "demandGraphRoot": graph.root().as_str(),
        "demands": graph.demands().iter().map(|demand| serde_json::json!({
            "id": demand.id().as_str(),
            "family": demand.family(),
            "owner": certification_demand_owner(demand.family()),
            "satisfiedByArtifactSnapshot": snapshot_witnesses.contains(demand.id().as_str()),
        })).collect::<Vec<_>>(),
    });
    let mut bytes = serde_json::to_vec_pretty(&output)?;
    bytes.push(b'\n');
    fs::write(output_path, bytes)?;
    Ok(())
}

fn execute_contract_certification(request_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let request: ContractCertificationExecutionRequest =
        serde_json::from_slice(&fs::read(request_path).map_err(|error| {
            format!(
                "could not read certification execution request {}: {error}",
                request_path.display()
            )
        })?)?;
    let is_single = request.schema_version == 1
        && request.planning.is_some()
        && request.plannings.is_empty()
        && request.graph.is_none()
        && request.graphs.is_empty()
        && request.graph_case_set.is_none();
    let is_case_set = request.schema_version == 2
        && request.planning.is_none()
        && request.plannings.len() >= 2
        && request.graph.is_none()
        && request.graphs.is_empty()
        && request.graph_case_set.is_none();
    let is_graph = request.schema_version == 3
        && request.planning.is_none()
        && request.plannings.is_empty()
        && request.graph.is_some()
        && request.graphs.is_empty()
        && request.graph_case_set.is_none();
    let is_graph_case_set = request.schema_version == 4
        && request.planning.is_none()
        && request.plannings.is_empty()
        && request.graph.is_none()
        && request.graphs.len() >= 2
        && request.graph_case_set.is_none();
    let is_deduplicated_graph_case_set = request.schema_version == 5
        && request.planning.is_none()
        && request.plannings.is_empty()
        && request.graph.is_none()
        && request.graphs.is_empty()
        && request
            .graph_case_set
            .as_ref()
            .is_some_and(|case_set| case_set.cases.len() >= 2 && !case_set.nodes.is_empty());
    if !is_single
        && !is_case_set
        && !is_graph
        && !is_graph_case_set
        && !is_deduplicated_graph_case_set
    {
        return Err(
            "certification execution version 1 requires one planning; version 2 requires at least two plannings; version 3 requires one finite graph; version 4 requires at least two finite graphs; version 5 requires one deduplicated finite graph case-set"
                .into(),
        );
    }
    let issuer_document: ReceiptIssuerConfigurationDocument =
        serde_json::from_slice(&fs::read(&request.issuer_configuration).map_err(|error| {
            format!(
                "could not read policy-2 issuer configuration {}: {error}",
                request.issuer_configuration
            )
        })?)?;
    if issuer_document.format != "solid-checker-policy2-issuer-configuration"
        || issuer_document.issuer_configuration_version != 1
    {
        return Err("unsupported policy-2 issuer configuration".into());
    }
    let seed = STANDARD
        .decode(issuer_document.seed)
        .map_err(|_| "policy-2 issuer seed is not canonical base64")?;
    let seed: [u8; 32] = seed
        .try_into()
        .map_err(|_| "policy-2 issuer seed must contain exactly 32 bytes")?;
    let issuer = match issuer_document.kind {
        solid_facts_backend::ReceiptIssuerKind::PersistentLocal => {
            solid_facts_backend::ConfiguredReceiptIssuer::persistent_local(
                issuer_document.scope,
                seed,
            )?
        }
        solid_facts_backend::ReceiptIssuerKind::Portable => {
            solid_facts_backend::ConfiguredReceiptIssuer::portable(issuer_document.scope, seed)?
        }
        solid_facts_backend::ReceiptIssuerKind::BuiltIn => {
            return Err("configured certification cannot claim built-in issuer provenance".into());
        }
    };
    let typefacts_path = fs::canonicalize(&request.typefacts_executable).map_err(|error| {
        format!(
            "could not resolve pinned Type Facts executable {}: {error}",
            request.typefacts_executable
        )
    })?;
    let pin = solid_facts_backend::TypeFactsProducerPin::configured(typefacts_path)
        .map_err(|error| format!("Type Facts producer pinning failed: {error}"))?;

    if is_graph {
        return execute_contract_graph_certification(
            request_path,
            request,
            &pin,
            &issuer,
            issuer_document.revocation_epoch,
        );
    }

    if is_graph_case_set || is_deduplicated_graph_case_set {
        return execute_contract_graph_case_set_certification(
            request_path,
            request,
            &pin,
            &issuer,
            issuer_document.revocation_epoch,
        );
    }

    if is_case_set {
        return execute_contract_case_set_certification(
            request_path,
            request,
            &pin,
            &issuer,
            issuer_document.revocation_epoch,
        );
    }

    let planning = request
        .planning
        .ok_or("single-case certification planning disappeared")?;
    let importer = planning.resolution.importer.clone();
    let specifier = planning.resolution.specifier.clone();
    let (plan, proposal) = certification_plan_from_request(planning)
        .map_err(|error| format!("certification planning failed: {error}"))?;
    let finalized = plan
        .certify_value_only(&proposal, &pin, &issuer, issuer_document.revocation_epoch)
        .map_err(|error| format!("policy-2 proof finalization failed: {error}"))?;
    let trust_bytes =
        solid_facts_backend::encode_policy2_trust_configuration(finalized.trust_configuration())
            .map_err(|error| format!("policy-2 trust encoding failed: {error}"))?;
    write_atomic_file(Path::new(&request.trust_configuration_output), &trust_bytes).map_err(
        |error| {
            format!(
                "policy-2 trust publication failed at {}: {error}",
                request.trust_configuration_output
            )
        },
    )?;
    plan.publish_finalized_policy2(Path::new(&request.catalog_root), &finalized)
        .map_err(|error| {
            format!(
                "accepted-contract catalog publication failed at {}: {error}",
                request.catalog_root
            )
        })?;

    let current_executable = std::env::current_exe().map_err(|error| {
        format!("could not locate checker for fresh-process verification: {error}")
    })?;
    let status = Command::new(&current_executable)
        .arg("--verify-policy2-discovery")
        .arg(request_path)
        .status()
        .map_err(|error| {
            format!(
                "could not launch fresh analyzer process {}: {error}",
                current_executable.display()
            )
        })?;
    if !status.success() {
        return Err(format!(
            "fresh analyzer process did not discover and authenticate {specifier:?} from {importer:?}"
        )
        .into());
    }
    Ok(())
}

fn execute_contract_case_set_certification(
    request_path: &Path,
    request: ContractCertificationExecutionRequest,
    pin: &solid_facts_backend::TypeFactsProducerPin,
    issuer: &solid_facts_backend::ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut plans = Vec::with_capacity(request.plannings.len());
    let mut proposal = None::<Vec<u8>>;
    let mut selected_ids = BTreeSet::new();
    let mut resolution_roots = BTreeSet::new();
    let mut row_identity = None::<(String, String, String)>;

    {
        let mut planning_transaction = solid_facts_backend::CertificationPlanningTransaction::new();
        for planning in request.plannings {
            let importer = planning.resolution.importer.clone();
            let specifier = planning.resolution.specifier.clone();
            let package_name = planning.resolution.package_name.clone();
            let package_version = planning.resolution.package_version.clone();
            let current_row_identity = (importer.clone(), package_name, package_version);
            if row_identity
                .as_ref()
                .is_some_and(|expected| expected != &current_row_identity)
            {
                return Err("a policy-2 case set must describe one exact package row".into());
            }
            row_identity.get_or_insert(current_row_identity);
            let resolved_import_root =
                solid_facts_backend::policy2_resolved_import_root(&planning.resolution)
                    .map_err(|error| format!("resolved-import binding failed: {error}"))?;
            if !resolution_roots.insert(resolved_import_root.clone()) {
                return Err("a policy-2 case set contains a duplicate resolved import".into());
            }
            let (plan, current_proposal) =
                certification_plan_from_request_in(&mut planning_transaction, planning)
                    .map_err(|error| format!("case-set certification planning failed: {error}"))?;
            if proposal
                .as_ref()
                .is_some_and(|expected| expected != &current_proposal)
            {
                return Err("a policy-2 case set contains different proposal documents".into());
            }
            proposal.get_or_insert(current_proposal);
            let selected_id = plan.selected_artifact_case_id().to_owned();
            if !selected_ids.insert(selected_id.clone()) {
                return Err(format!(
                    "a policy-2 case set selects artifact case {selected_id:?} more than once"
                )
                .into());
            }
            plans.push((plan, selected_id, resolved_import_root, importer, specifier));
        }
    }

    let proposal = proposal.ok_or("a policy-2 case set has no proposal")?;
    let review: ContractReviewForCaseSet =
        serde_json::from_slice(&review_contract_document(&proposal)?)?;
    let expected_ids = review
        .artifact_cases
        .into_iter()
        .map(|case| case.id)
        .collect::<BTreeSet<_>>();
    if expected_ids != selected_ids {
        let omitted = expected_ids
            .difference(&selected_ids)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = selected_ids
            .difference(&expected_ids)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "case-set planning does not cover the proposal artifact census; omitted={omitted:?}, unexpected={unexpected:?}"
        )
        .into());
    }

    let catalog_root = Path::new(&request.catalog_root);
    fs::create_dir_all(catalog_root)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let stage_root = catalog_root.join(format!(
        ".policy2-case-set.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    fs::create_dir(&stage_root)?;
    let cases_root = stage_root.join("cases");
    fs::create_dir(&cases_root)?;

    let mut entries = Vec::with_capacity(plans.len());
    let mut trust_bytes = None::<Vec<u8>>;
    let plan_refs = plans.iter().map(|(plan, ..)| plan).collect::<Vec<_>>();
    let finalized = solid_facts_backend::certify_value_only_case_set(
        &plan_refs,
        &proposal,
        pin,
        issuer,
        revocation_epoch,
    )
    .map_err(|error| format!("policy-2 case-set finalization failed: {error}"))?;
    for ((plan, artifact_case_id, resolved_import_root, importer, specifier), finalized) in
        plans.into_iter().zip(finalized)
    {
        let current_trust = solid_facts_backend::encode_policy2_trust_configuration(
            finalized.trust_configuration(),
        )
        .map_err(|error| format!("policy-2 trust encoding failed: {error}"))?;
        if trust_bytes
            .as_ref()
            .is_some_and(|expected| expected != &current_trust)
        {
            return Err("case-set finalization produced inconsistent trust configurations".into());
        }
        trust_bytes.get_or_insert(current_trust);

        let case_key = resolved_import_root
            .strip_prefix("sha256:")
            .ok_or("resolved-import root is not canonical sha256")?
            .to_owned();
        let case_root = cases_root.join(&case_key);
        let published = plan
            .publish_finalized_policy2(&case_root, &finalized)
            .map_err(|error| {
                format!(
                    "accepted-contract case publication failed at {}: {error}",
                    case_root.display()
                )
            })?;
        let catalog_bytes = fs::read(&published.catalog_path)?;
        entries.push(Policy2CaseSetEntry {
            artifact_case_id,
            importer,
            specifier,
            resolved_import_root,
            semantic_digest: finalized.bindings().semantic_digest.clone(),
            receipt_digest: finalized.authenticated().receipt_digest().to_owned(),
            catalog: format!("cases/{case_key}/accepted-contracts.json"),
            catalog_digest: sha256_digest(&catalog_bytes),
        });
    }
    let (case_set_bytes, case_set_digest) = canonical_policy2_case_set(entries)?;
    let staged_case_set_path = stage_root.join("accepted-contract-case-set.json");
    write_atomic_file(&staged_case_set_path, &case_set_bytes)?;
    let staged_trust_path = stage_root.join("policy2-trust.json");
    write_atomic_file(
        &staged_trust_path,
        trust_bytes
            .as_deref()
            .ok_or("case-set finalization produced no trust configuration")?,
    )?;

    verify_policy2_case_set_in_fresh_process(request_path, &stage_root, &staged_trust_path)?;

    let case_sets_root = catalog_root.join("case-sets");
    fs::create_dir_all(&case_sets_root)?;
    let case_set_key = case_set_digest
        .strip_prefix("sha256:")
        .ok_or("case-set digest is not canonical sha256")?;
    let final_root = case_sets_root.join(case_set_key);
    if final_root.exists() {
        let existing = fs::read(final_root.join("accepted-contract-case-set.json"))?;
        if existing != case_set_bytes {
            return Err("policy-2 case-set content-address collision".into());
        }
        fs::remove_dir_all(&stage_root)?;
    } else {
        fs::rename(&stage_root, &final_root)?;
        fs::File::open(&case_sets_root)?.sync_all()?;
    }

    // A matching manifest is not enough when a prior content-addressed
    // directory already exists: its referenced catalogs could have been
    // altered after the original publication. Authenticate the committed
    // bytes again before moving either public pointer.
    verify_policy2_case_set_in_fresh_process(
        request_path,
        &final_root,
        &final_root.join("policy2-trust.json"),
    )?;

    write_atomic_file(
        Path::new(&request.trust_configuration_output),
        trust_bytes
            .as_deref()
            .ok_or("case-set finalization produced no trust configuration")?,
    )?;
    let pointer = Policy2CaseSetPointer {
        format: "solid-checker-accepted-contract-case-set-pointer".into(),
        case_set_version: 1,
        document: format!("case-sets/{case_set_key}/accepted-contract-case-set.json"),
        document_digest: case_set_digest,
    };
    let mut pointer_bytes = serde_json::to_vec(&pointer)?;
    pointer_bytes.push(b'\n');
    write_atomic_file(
        &catalog_root.join("accepted-contract-case-set.json"),
        &pointer_bytes,
    )?;
    verify_policy2_case_set_pointer(catalog_root)?;
    Ok(())
}

fn execute_contract_graph_certification(
    request_path: &Path,
    mut request: ContractCertificationExecutionRequest,
    pin: &solid_facts_backend::TypeFactsProducerPin,
    issuer: &solid_facts_backend::ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let graph_request = request
        .graph
        .take()
        .ok_or("graph certification request disappeared")?;
    let graph = certification_graph_from_request(graph_request)?;
    let finalized = graph
        .certify_value_only(pin, issuer, revocation_epoch)
        .map_err(|error| format!("published graph finalization failed: {error}"))?;
    if finalized.graph_root() != graph.graph_root() {
        return Err("published graph finalization changed the graph root".into());
    }

    let trust_bytes = solid_facts_backend::encode_policy2_trust_configuration(
        finalized.root().trust_configuration(),
    )
    .map_err(|error| format!("policy-2 graph trust encoding failed: {error}"))?;
    for node in finalized.nodes() {
        let current = solid_facts_backend::encode_policy2_trust_configuration(
            node.finalized().trust_configuration(),
        )?;
        if current != trust_bytes {
            return Err(
                "published graph finalization produced inconsistent trust configurations".into(),
            );
        }
    }

    let catalog_root = Path::new(&request.catalog_root);
    fs::create_dir_all(catalog_root)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let stage_root = catalog_root.join(format!(
        ".policy2-graph.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    fs::create_dir(&stage_root)?;
    fs::create_dir(stage_root.join("nodes"))?;
    let staged_trust = stage_root.join("policy2-trust.json");
    write_atomic_file(&staged_trust, &trust_bytes)?;

    for node in finalized.nodes() {
        let plan = graph
            .plan(node.identity())
            .ok_or("finalized graph node has no retained opaque plan")?;
        let node_root = if node.identity() == graph.root_identity() {
            stage_root.join("root")
        } else {
            stage_root
                .join("nodes")
                .join(graph_digest_key(node.identity().digest())?)
        };
        plan.publish_finalized_policy2(&node_root, node.finalized())
            .map_err(|error| {
                format!(
                    "published graph node {} could not be staged: {error}",
                    node.identity().digest()
                )
            })?;
    }

    let manifest = serde_json::json!({
        "format": "solid-checker-policy2-published-graph",
        "graphVersion": 1,
        "graphRoot": graph.graph_root(),
        "rootNode": graph.root_identity().digest(),
        "dependencyFirstNodes": graph
            .dependency_first_identities()
            .into_iter()
            .map(solid_facts_backend::CanonicalDependencyNodeIdentity::digest)
            .collect::<Vec<_>>(),
    });
    let mut manifest_bytes = serde_json::to_vec(&manifest)?;
    manifest_bytes.push(b'\n');
    write_atomic_file(&stage_root.join("graph.json"), &manifest_bytes)?;

    // No accepted catalog is public yet. Authenticate the staged root in a
    // fresh process before committing this complete content-addressed graph.
    verify_policy2_case_set_in_fresh_process(
        request_path,
        &stage_root.join("root"),
        &staged_trust,
    )?;

    let graphs_root = catalog_root.join("graphs");
    fs::create_dir_all(&graphs_root)?;
    let final_root = graphs_root.join(graph_digest_key(graph.graph_root())?);
    if final_root.exists() {
        if fs::read(final_root.join("graph.json"))? != manifest_bytes {
            return Err("policy-2 published-graph content-address collision".into());
        }
        fs::remove_dir_all(&stage_root)?;
    } else {
        fs::rename(&stage_root, &final_root)?;
        fs::File::open(&graphs_root)?.sync_all()?;
    }

    // Re-authenticate committed bytes. Only then expose the trust file and
    // root catalog through their ordinary public locations.
    verify_policy2_case_set_in_fresh_process(
        request_path,
        &final_root.join("root"),
        &final_root.join("policy2-trust.json"),
    )?;
    write_atomic_file(Path::new(&request.trust_configuration_output), &trust_bytes)?;
    let root_plan = graph
        .plan(graph.root_identity())
        .ok_or("published graph lost its root plan")?;
    root_plan
        .publish_finalized_policy2(catalog_root, finalized.root())
        .map_err(|error| format!("published graph root publication failed: {error}"))?;
    verify_policy2_case_set_in_fresh_process(
        request_path,
        catalog_root,
        Path::new(&request.trust_configuration_output),
    )?;
    Ok(())
}

fn graph_digest_key(digest: &str) -> Result<&str, Box<dyn std::error::Error>> {
    digest
        .strip_prefix("sha256:")
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .ok_or_else(|| "published graph digest is not canonical SHA-256".into())
}

fn expand_deduplicated_graph_case_set(
    case_set: ContractCertificationGraphCaseSetRequest,
) -> Result<Vec<ContractCertificationGraphRequest>, Box<dyn std::error::Error>> {
    let mut nodes = BTreeMap::new();
    for node in case_set.nodes {
        if node.key.is_empty() || nodes.insert(node.key.clone(), node).is_some() {
            return Err("deduplicated graph case-set has an empty or duplicate node key".into());
        }
    }
    let mut referenced = BTreeSet::new();
    let mut cases = Vec::with_capacity(case_set.cases.len());
    for case in case_set.cases {
        let node_keys = case.nodes.into_iter().collect::<BTreeSet<_>>();
        if !node_keys.contains(&case.root) {
            return Err("deduplicated graph case does not contain its root node".into());
        }
        let root = nodes
            .get(&case.root)
            .cloned()
            .ok_or("deduplicated graph case names an unknown root node")?
            .graph_node();
        let dependencies = node_keys
            .iter()
            .filter(|key| *key != &case.root)
            .map(|key| {
                referenced.insert(key.clone());
                nodes
                    .get(key)
                    .cloned()
                    .map(ContractCertificationGraphCaseSetNodeRequest::graph_node)
                    .ok_or_else(|| {
                        "deduplicated graph case names an unknown dependency node".into()
                    })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        referenced.insert(case.root);
        cases.push(ContractCertificationGraphRequest { root, dependencies });
    }
    if referenced.len() != nodes.len() {
        return Err("deduplicated graph case-set transports an unreachable node".into());
    }
    Ok(cases)
}

fn execute_contract_graph_case_set_certification(
    request_path: &Path,
    request: ContractCertificationExecutionRequest,
    pin: &solid_facts_backend::TypeFactsProducerPin,
    issuer: &solid_facts_backend::ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let graph_requests = match request.graph_case_set {
        Some(case_set) => expand_deduplicated_graph_case_set(case_set)?,
        None => request.graphs,
    };
    let mut graphs = Vec::with_capacity(graph_requests.len());
    let mut case_bindings = Vec::with_capacity(graph_requests.len());
    {
        let mut planning_transaction = solid_facts_backend::CertificationPlanningTransaction::new();
        for graph_request in graph_requests {
            let importer = graph_request.root.planning.resolution.importer.clone();
            let specifier = graph_request.root.planning.resolution.specifier.clone();
            let resolved_import_root = solid_facts_backend::policy2_resolved_import_root(
                &graph_request.root.planning.resolution,
            )?;
            let graph =
                certification_graph_from_request_in(&mut planning_transaction, graph_request)?;
            graphs.push(graph);
            case_bindings.push((importer, specifier, resolved_import_root));
        }
    }
    let finalized = solid_facts_backend::certify_published_contract_graph_case_set(
        &graphs,
        pin,
        issuer,
        revocation_epoch,
    )
    .map_err(|error| format!("published graph case-set finalization failed: {error}"))?;

    let catalog_root = Path::new(&request.catalog_root);
    fs::create_dir_all(catalog_root)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let stage_root = catalog_root.join(format!(
        ".policy2-graph-case-set.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    fs::create_dir(&stage_root)?;
    fs::create_dir(stage_root.join("cases"))?;
    fs::create_dir(stage_root.join("graph-nodes"))?;

    let mut entries = Vec::with_capacity(graphs.len());
    let mut trust_bytes = None::<Vec<u8>>;
    for ((graph, finalized), (importer, specifier, resolved_import_root)) in
        graphs.iter().zip(&finalized).zip(&case_bindings)
    {
        let current_trust = solid_facts_backend::encode_policy2_trust_configuration(
            finalized.root().trust_configuration(),
        )?;
        if trust_bytes
            .as_ref()
            .is_some_and(|expected| expected != &current_trust)
        {
            return Err(
                "published graph case set produced inconsistent trust configurations".into(),
            );
        }
        trust_bytes.get_or_insert(current_trust);
        let case_key = graph_digest_key(resolved_import_root)?;
        let case_root = stage_root.join("cases").join(case_key);
        let hidden_nodes = stage_root.join("graph-nodes").join(case_key);
        fs::create_dir(&hidden_nodes)?;
        let mut published_root = None;
        for node in finalized.nodes() {
            let node_plan = graph
                .plan(node.identity())
                .ok_or("finalized graph case node has no retained opaque plan")?;
            let node_root = if node.identity() == graph.root_identity() {
                case_root.clone()
            } else {
                hidden_nodes.join(graph_digest_key(node.identity().digest())?)
            };
            let published = node_plan.publish_finalized_policy2(&node_root, node.finalized())?;
            if node.identity() == graph.root_identity() {
                published_root = Some(published);
            }
        }
        let published = published_root.ok_or("published graph case has no root catalog")?;
        let catalog_bytes = fs::read(&published.catalog_path)?;
        entries.push(Policy2CaseSetEntry {
            artifact_case_id: graph
                .plan(graph.root_identity())
                .ok_or("published graph case lost its root plan")?
                .selected_artifact_case_id()
                .into(),
            importer: importer.clone(),
            specifier: specifier.clone(),
            resolved_import_root: resolved_import_root.clone(),
            semantic_digest: finalized.root().bindings().semantic_digest.clone(),
            receipt_digest: finalized.root().authenticated().receipt_digest().into(),
            catalog: format!("cases/{case_key}/accepted-contracts.json"),
            catalog_digest: sha256_digest(&catalog_bytes),
        });
    }
    let (case_set_bytes, case_set_digest) = canonical_policy2_case_set(entries)?;
    write_atomic_file(
        &stage_root.join("accepted-contract-case-set.json"),
        &case_set_bytes,
    )?;
    let staged_trust = stage_root.join("policy2-trust.json");
    write_atomic_file(
        &staged_trust,
        trust_bytes
            .as_deref()
            .ok_or("published graph case set produced no trust configuration")?,
    )?;
    verify_policy2_case_set_in_fresh_process(request_path, &stage_root, &staged_trust)?;

    let case_sets_root = catalog_root.join("case-sets");
    fs::create_dir_all(&case_sets_root)?;
    let case_set_key = graph_digest_key(&case_set_digest)?;
    let final_root = case_sets_root.join(case_set_key);
    if final_root.exists() {
        if fs::read(final_root.join("accepted-contract-case-set.json"))? != case_set_bytes {
            return Err("policy-2 published-graph case-set content-address collision".into());
        }
        fs::remove_dir_all(&stage_root)?;
    } else {
        fs::rename(&stage_root, &final_root)?;
        fs::File::open(&case_sets_root)?.sync_all()?;
    }
    verify_policy2_case_set_in_fresh_process(
        request_path,
        &final_root,
        &final_root.join("policy2-trust.json"),
    )?;

    write_atomic_file(
        Path::new(&request.trust_configuration_output),
        trust_bytes
            .as_deref()
            .ok_or("published graph case set produced no trust configuration")?,
    )?;
    let pointer = Policy2CaseSetPointer {
        format: "solid-checker-accepted-contract-case-set-pointer".into(),
        case_set_version: 1,
        document: format!("case-sets/{case_set_key}/accepted-contract-case-set.json"),
        document_digest: case_set_digest,
    };
    let mut pointer_bytes = serde_json::to_vec(&pointer)?;
    pointer_bytes.push(b'\n');
    write_atomic_file(
        &catalog_root.join("accepted-contract-case-set.json"),
        &pointer_bytes,
    )?;
    verify_policy2_case_set_pointer(catalog_root)?;
    Ok(())
}

fn canonical_policy2_case_set(
    mut cases: Vec<Policy2CaseSetEntry>,
) -> Result<(Vec<u8>, String), Box<dyn std::error::Error>> {
    if cases.len() < 2 {
        return Err("a policy-2 case set must contain at least two cases".into());
    }
    cases.sort_by(|left, right| {
        (
            &left.artifact_case_id,
            &left.resolved_import_root,
            &left.importer,
            &left.specifier,
        )
            .cmp(&(
                &right.artifact_case_id,
                &right.resolved_import_root,
                &right.importer,
                &right.specifier,
            ))
    });
    let mut artifact_case_ids = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    let mut resolved_import_roots = BTreeSet::new();
    for case in &cases {
        if !artifact_case_ids.insert(case.artifact_case_id.clone()) {
            return Err(format!(
                "policy-2 case set repeats artifact case {:?}",
                case.artifact_case_id
            )
            .into());
        }
        if !resolved_import_roots.insert(case.resolved_import_root.clone()) {
            return Err(format!(
                "policy-2 case set repeats resolved import {:?}",
                case.resolved_import_root
            )
            .into());
        }
        if !coordinates.insert(Policy2CaseCoordinate::from(case)) {
            return Err("policy-2 case set repeats an exact case coordinate".into());
        }
        for (label, digest) in [
            ("resolved import", &case.resolved_import_root),
            ("semantic", &case.semantic_digest),
            ("receipt", &case.receipt_digest),
            ("catalog", &case.catalog_digest),
        ] {
            if !is_canonical_sha256(digest) {
                return Err(
                    format!("policy-2 case-set {label} digest is not canonical sha256").into(),
                );
            }
        }
        let expected_catalog = policy2_case_catalog_path(&case.resolved_import_root)?;
        if case.catalog != expected_catalog {
            return Err(format!(
                "policy-2 case-set catalog {:?} is not bound to its resolved import root",
                case.catalog
            )
            .into());
        }
    }
    let document = Policy2CaseSetDocument {
        format: "solid-checker-accepted-contract-case-set".into(),
        case_set_version: 1,
        cases,
    };
    let mut bytes = serde_json::to_vec(&document)?;
    bytes.push(b'\n');
    let digest = sha256_digest(&bytes);
    Ok((bytes, digest))
}

fn policy2_case_catalog_path(
    resolved_import_root: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let case_key = resolved_import_root
        .strip_prefix("sha256:")
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or("resolved-import root is not canonical sha256")?;
    if case_key.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err("resolved-import root is not lowercase canonical sha256".into());
    }
    Ok(format!("cases/{case_key}/accepted-contracts.json"))
}

fn verify_policy2_case_set_in_fresh_process(
    request_path: &Path,
    root: &Path,
    trust_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_executable = std::env::current_exe().map_err(|error| {
        format!("could not locate checker for fresh-process verification: {error}")
    })?;
    let status = Command::new(&current_executable)
        .arg("--verify-policy2-discovery")
        .arg(request_path)
        .env("SOLID_CHECKER_POLICY2_DISCOVERY_ROOT", root)
        .env("SOLID_CHECKER_POLICY2_DISCOVERY_TRUST", trust_path)
        .status()
        .map_err(|error| {
            format!(
                "could not launch fresh analyzer process {}: {error}",
                current_executable.display()
            )
        })?;
    if !status.success() {
        return Err(
            "fresh analyzer process did not authenticate the complete policy-2 case set".into(),
        );
    }
    Ok(())
}

fn verify_policy2_case_set_pointer(catalog_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let pointer_bytes = fs::read(catalog_root.join("accepted-contract-case-set.json"))?;
    let pointer: Policy2CaseSetPointer = serde_json::from_slice(&pointer_bytes)?;
    if pointer.format != "solid-checker-accepted-contract-case-set-pointer"
        || pointer.case_set_version != 1
        || !is_canonical_sha256(&pointer.document_digest)
    {
        return Err("unsupported policy-2 case-set pointer".into());
    }
    let document_path = safe_case_set_member(catalog_root, &pointer.document)?;
    let document_bytes = fs::read(document_path)?;
    verify_sha256_digest(
        &document_bytes,
        &pointer.document_digest,
        "policy-2 case-set pointer document",
    )?;
    let document: Policy2CaseSetDocument = serde_json::from_slice(&document_bytes)?;
    if document.format != "solid-checker-accepted-contract-case-set"
        || document.case_set_version != 1
    {
        return Err("policy-2 case-set pointer selects an unsupported document".into());
    }
    let (canonical_bytes, canonical_digest) = canonical_policy2_case_set(document.cases)?;
    if canonical_bytes != document_bytes || canonical_digest != pointer.document_digest {
        return Err("policy-2 case-set pointer selects a noncanonical document".into());
    }
    Ok(())
}

fn verify_policy2_discovery(request_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let request: ContractCertificationExecutionRequest =
        serde_json::from_slice(&fs::read(request_path)?)?;
    let trust_path = std::env::var_os("SOLID_CHECKER_POLICY2_DISCOVERY_TRUST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&request.trust_configuration_output));
    let trust = read_policy2_trust_configuration(&trust_path)?;
    let catalog_root = std::env::var_os("SOLID_CHECKER_POLICY2_DISCOVERY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&request.catalog_root));
    if matches!(request.schema_version, 2 | 4 | 5) {
        return verify_policy2_case_set_discovery(&request, &catalog_root, &trust);
    }
    let planning = match request.schema_version {
        1 => request.planning.as_ref(),
        3 => request.graph.as_ref().map(|graph| &graph.root.planning),
        _ => None,
    }
    .ok_or("single-case or graph discovery request has no root planning")?;
    let catalog = catalog_root.join("accepted-contracts.json");
    let index = read_accepted_contract_catalog_with_trust(&catalog, Some(&trust))?;
    let importer = &planning.resolution.importer;
    let specifier = &planning.resolution.specifier;
    let selected = index
        .semantic_identity()
        .iter()
        .filter(|identity| &identity.importer == importer && &identity.specifier == specifier)
        .count();
    if selected != 1 {
        return Err(format!(
            "ordinary policy-2 discovery selected {selected} entries; expected exactly one"
        )
        .into());
    }
    Ok(())
}

fn verify_policy2_case_set_discovery(
    request: &ContractCertificationExecutionRequest,
    root: &Path,
    trust: &solid_facts_backend::Policy2TrustConfiguration,
) -> Result<(), Box<dyn std::error::Error>> {
    let case_set_path = root.join("accepted-contract-case-set.json");
    let case_set_bytes = fs::read(&case_set_path)?;
    let case_set: Policy2CaseSetDocument = serde_json::from_slice(&case_set_bytes)?;
    if case_set.format != "solid-checker-accepted-contract-case-set"
        || case_set.case_set_version != 1
    {
        return Err("unsupported policy-2 case-set document".into());
    }
    let (canonical_bytes, _) = canonical_policy2_case_set(case_set.cases.clone())?;
    if canonical_bytes != case_set_bytes {
        return Err("ordinary policy-2 discovery found a noncanonical case-set document".into());
    }

    // Reconstruct every opaque plan from authenticated acquisition inputs in
    // this fresh process. Comparing independent exact coordinates prevents an
    // otherwise set-preserving swap between an artifact case and a resolved
    // import root.
    let expected_coordinates = expected_policy2_case_coordinates(request)?;
    validate_policy2_case_coordinates(&expected_coordinates, &case_set.cases)?;

    for case in &case_set.cases {
        let catalog = safe_case_set_member(root, &case.catalog)?;
        let catalog_bytes = fs::read(&catalog)?;
        verify_sha256_digest(
            &catalog_bytes,
            &case.catalog_digest,
            "policy-2 case-set catalog",
        )?;
        let index = read_accepted_contract_catalog_with_trust(&catalog, Some(trust))?;
        let selected =
            index
                .semantic_identity()
                .iter()
                .filter(|identity| {
                    identity.importer == case.importer
                        && identity.specifier == case.specifier
                        && identity.semantics.artifact_case == case.artifact_case_id
                        && identity.semantics.semantic_digest.as_str() == case.semantic_digest
                        && identity.semantics.authentication.as_ref().is_some_and(
                            |authentication| {
                                authentication.receipt_digest.as_str() == case.receipt_digest
                            },
                        )
                })
                .count();
        if selected != 1 || index.semantic_identity().len() != 1 {
            return Err(format!(
                "ordinary policy-2 discovery selected {selected} entries for artifact case {:?}; expected exactly one",
                case.artifact_case_id
            )
            .into());
        }
    }
    Ok(())
}

fn expected_policy2_case_coordinates(
    request: &ContractCertificationExecutionRequest,
) -> Result<BTreeSet<Policy2CaseCoordinate>, Box<dyn std::error::Error>> {
    // This transaction is local to this one fresh-process case-set replay.
    // It can reuse only fully verified immutable archive snapshots; every
    // graph root or ordinary planning still rebuilds and checks its own
    // resolution, closure, demands, Type Facts evidence, and receipt binding.
    let mut planning_transaction = solid_facts_backend::CertificationPlanningTransaction::new();
    if matches!(request.schema_version, 4 | 5) {
        let graph_requests = if request.schema_version == 5 {
            expand_deduplicated_graph_case_set(
                request
                    .graph_case_set
                    .clone()
                    .ok_or("fresh graph case-set request disappeared")?,
            )?
        } else {
            request.graphs.clone()
        };
        let mut coordinates = BTreeSet::new();
        for graph_request in &graph_requests {
            let planning = &graph_request.root.planning;
            let importer = planning.resolution.importer.clone();
            let specifier = planning.resolution.specifier.clone();
            let resolved_import_root =
                solid_facts_backend::policy2_resolved_import_root(&planning.resolution)?;
            // Reconstruct the same authenticated graph that produced the
            // staged case. A graph root can legitimately terminate an export
            // identity in a child receipt, so replaying the root as a
            // standalone package would reject the very composition this
            // fresh-process check is meant to authenticate.
            let graph = certification_graph_from_request_in(
                &mut planning_transaction,
                graph_request.clone(),
            )
            .map_err(|error| format!("fresh graph case planning failed: {error}"))?;
            let plan = graph
                .plan(graph.root_identity())
                .ok_or("fresh graph case planning did not retain its root plan")?;
            if !coordinates.insert(Policy2CaseCoordinate {
                artifact_case_id: plan.selected_artifact_case_id().into(),
                importer,
                specifier,
                resolved_import_root,
            }) {
                return Err(
                    "fresh graph case planning produced a duplicate exact coordinate".into(),
                );
            }
        }
        if coordinates.len() < 2 {
            return Err("fresh graph case planning produced fewer than two cases".into());
        }
        return Ok(coordinates);
    }
    let mut coordinates = BTreeSet::new();
    let mut proposal = None::<Vec<u8>>;
    for planning in &request.plannings {
        let importer = planning.resolution.importer.clone();
        let specifier = planning.resolution.specifier.clone();
        let resolved_import_root =
            solid_facts_backend::policy2_resolved_import_root(&planning.resolution)?;
        let (plan, current_proposal) =
            certification_plan_from_request_in(&mut planning_transaction, planning.clone())
                .map_err(|error| format!("fresh case-set planning failed: {error}"))?;
        if proposal
            .as_ref()
            .is_some_and(|expected| expected != &current_proposal)
        {
            return Err("fresh case-set planning found different proposal documents".into());
        }
        proposal.get_or_insert(current_proposal);
        if !coordinates.insert(Policy2CaseCoordinate {
            artifact_case_id: plan.selected_artifact_case_id().to_owned(),
            importer,
            specifier,
            resolved_import_root,
        }) {
            return Err("fresh case-set planning produced a duplicate exact coordinate".into());
        }
    }
    if coordinates.len() < 2 {
        return Err("fresh case-set planning produced fewer than two cases".into());
    }
    Ok(coordinates)
}

fn validate_policy2_case_coordinates(
    expected: &BTreeSet<Policy2CaseCoordinate>,
    cases: &[Policy2CaseSetEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = cases
        .iter()
        .map(Policy2CaseCoordinate::from)
        .collect::<BTreeSet<_>>();
    if expected != &actual || actual.len() != cases.len() {
        return Err(
            "ordinary policy-2 discovery found an incomplete, duplicate, or transplanted case coordinate"
                .into(),
        );
    }
    Ok(())
}

fn safe_case_set_member(
    root: &Path,
    relative: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = Path::new(relative);
    let spelling = relative.to_string_lossy();
    let windows_absolute = spelling.as_bytes().get(1) == Some(&b':')
        && spelling
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || spelling.starts_with('\\')
        || windows_absolute
        || spelling
            .split(['/', '\\'])
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("policy-2 case-set member path is not a safe relative path".into());
    }
    Ok(root.join(relative))
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn verify_sha256_digest(
    bytes: &[u8],
    expected: &str,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_canonical_sha256(expected) || sha256_digest(bytes) != expected {
        return Err(format!("{label} digest mismatch").into());
    }
    Ok(())
}

fn is_canonical_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn write_atomic_file(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary = parent.join(format!(
        ".{}.tmp-{}-{nonce}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("trust"),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod policy2_case_set_tests {
    use super::*;

    fn digest(label: &str) -> String {
        sha256_digest(label.as_bytes())
    }

    fn case(id: &str, root_label: &str) -> Policy2CaseSetEntry {
        let resolved_import_root = digest(root_label);
        Policy2CaseSetEntry {
            artifact_case_id: id.into(),
            importer: "/project/src/index.ts".into(),
            specifier: "example-package".into(),
            catalog: policy2_case_catalog_path(&resolved_import_root).unwrap(),
            resolved_import_root,
            semantic_digest: digest(&format!("semantic:{id}")),
            receipt_digest: digest(&format!("receipt:{id}")),
            catalog_digest: digest(&format!("catalog:{id}")),
        }
    }

    fn coordinates(cases: &[Policy2CaseSetEntry]) -> BTreeSet<Policy2CaseCoordinate> {
        cases.iter().map(Policy2CaseCoordinate::from).collect()
    }

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "solid-checker-policy2-case-set-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn case_set_identity_is_deterministic_under_input_reordering() {
        let first = case("artifact:a", "root:a");
        let second = case("artifact:b", "root:b");
        let (forward, forward_digest) =
            canonical_policy2_case_set(vec![first.clone(), second.clone()]).unwrap();
        let (reverse, reverse_digest) = canonical_policy2_case_set(vec![second, first]).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward_digest, reverse_digest);
    }

    #[test]
    fn case_set_refuses_omission_duplication_and_coordinate_transplant() {
        let first = case("artifact:a", "root:a");
        let second = case("artifact:b", "root:b");
        let expected = coordinates(&[first.clone(), second.clone()]);
        assert!(
            validate_policy2_case_coordinates(&expected, std::slice::from_ref(&first)).is_err()
        );

        let mut duplicate_id = second.clone();
        duplicate_id.artifact_case_id = first.artifact_case_id.clone();
        assert!(canonical_policy2_case_set(vec![first.clone(), duplicate_id]).is_err());

        let mut duplicate_root = second.clone();
        duplicate_root.resolved_import_root = first.resolved_import_root.clone();
        duplicate_root.catalog = first.catalog.clone();
        assert!(canonical_policy2_case_set(vec![first.clone(), duplicate_root]).is_err());

        let mut transplanted_first = first.clone();
        let mut transplanted_second = second.clone();
        std::mem::swap(
            &mut transplanted_first.resolved_import_root,
            &mut transplanted_second.resolved_import_root,
        );
        transplanted_first.catalog =
            policy2_case_catalog_path(&transplanted_first.resolved_import_root).unwrap();
        transplanted_second.catalog =
            policy2_case_catalog_path(&transplanted_second.resolved_import_root).unwrap();
        assert!(
            canonical_policy2_case_set(vec![
                transplanted_first.clone(),
                transplanted_second.clone()
            ])
            .is_ok()
        );
        assert!(
            validate_policy2_case_coordinates(
                &expected,
                &[transplanted_first, transplanted_second]
            )
            .is_err()
        );
    }

    #[test]
    fn case_set_identity_cannot_replay_under_another_importer() {
        let original = vec![case("artifact:a", "root:a"), case("artifact:b", "root:b")];
        let expected = coordinates(&original);
        let mut replay = original;
        for entry in &mut replay {
            entry.importer = "/another/project/src/index.ts".into();
        }
        assert!(validate_policy2_case_coordinates(&expected, &replay).is_err());
    }

    #[test]
    fn case_set_member_rejects_posix_and_windows_traversal() {
        let root = Path::new("/catalog");
        for member in [
            "",
            "/absolute",
            "../escape",
            "cases/../escape",
            "cases//escape",
            "cases/./escape",
            r"..\escape",
            r"cases\..\escape",
            r"C:\escape",
            r"\\server\share",
        ] {
            assert!(
                safe_case_set_member(root, member).is_err(),
                "unsafe member was accepted: {member:?}"
            );
        }
        assert_eq!(
            safe_case_set_member(root, "case-sets/abc/document.json").unwrap(),
            root.join("case-sets/abc/document.json")
        );
    }

    #[test]
    fn catalog_digest_rejects_mutated_bytes() {
        let bytes = b"authenticated catalog";
        let expected = sha256_digest(bytes);
        verify_sha256_digest(bytes, &expected, "catalog").unwrap();
        assert!(verify_sha256_digest(b"mutated catalog", &expected, "catalog").is_err());
    }

    #[test]
    fn case_set_pointer_never_exposes_uncommitted_or_mutated_document() {
        let root = TestRoot::new("pointer");
        let cases = vec![case("artifact:a", "root:a"), case("artifact:b", "root:b")];
        let (document, document_digest) = canonical_policy2_case_set(cases).unwrap();
        let key = document_digest.strip_prefix("sha256:").unwrap();
        let document_relative = format!("case-sets/{key}/accepted-contract-case-set.json");
        let document_path = root.0.join(&document_relative);
        fs::create_dir_all(document_path.parent().unwrap()).unwrap();
        fs::write(&document_path, &document).unwrap();

        // Staged content is unreachable until the one public pointer exists.
        assert!(verify_policy2_case_set_pointer(&root.0).is_err());
        let pointer = Policy2CaseSetPointer {
            format: "solid-checker-accepted-contract-case-set-pointer".into(),
            case_set_version: 1,
            document: document_relative,
            document_digest,
        };
        let mut pointer_bytes = serde_json::to_vec(&pointer).unwrap();
        pointer_bytes.push(b'\n');
        write_atomic_file(
            &root.0.join("accepted-contract-case-set.json"),
            &pointer_bytes,
        )
        .unwrap();
        verify_policy2_case_set_pointer(&root.0).unwrap();

        fs::write(&document_path, b"mutated after publication\n").unwrap();
        assert!(verify_policy2_case_set_pointer(&root.0).is_err());
    }

    #[test]
    fn case_set_pointer_refuses_traversal() {
        let root = TestRoot::new("traversal");
        let pointer = Policy2CaseSetPointer {
            format: "solid-checker-accepted-contract-case-set-pointer".into(),
            case_set_version: 1,
            document: "../accepted-contract-case-set.json".into(),
            document_digest: digest("document"),
        };
        let mut pointer_bytes = serde_json::to_vec(&pointer).unwrap();
        pointer_bytes.push(b'\n');
        fs::write(
            root.0.join("accepted-contract-case-set.json"),
            pointer_bytes,
        )
        .unwrap();
        assert!(verify_policy2_case_set_pointer(&root.0).is_err());
    }
}

fn certification_demand_owner(
    family: solid_reactive_ir::contract_semantics::certification::ProofFamily,
) -> &'static str {
    use solid_reactive_ir::contract_semantics::certification::ProofFamily;
    match family {
        ProofFamily::PackageIdentity
        | ProofFamily::ManifestEntrypoint
        | ProofFamily::ExportResolution
        | ProofFamily::ArtifactDeclarations
        | ProofFamily::ExportIdentity
        | ProofFamily::ModuleClosure => "package-artifact",
        ProofFamily::SelectedSignature
        | ProofFamily::ArgumentBinding
        | ProofFamily::RestSpreadCoverage
        | ProofFamily::CallablePath
        | ProofFamily::OperationReachability
        | ProofFamily::OperationCardinality
        | ProofFamily::RecursiveValueShape
        | ProofFamily::GuardPartition
        | ProofFamily::DomainExhaustiveness => "type-facts",
        ProofFamily::CompilerReconciliation => "compiler-facts",
        ProofFamily::AcceptedDependencyComposition => "dependency-contract",
        ProofFamily::ProbeConsistency => "probe-gate",
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let started = Instant::now();
    #[cfg(feature = "dialect-v2")]
    {
        let arguments = std::env::args().collect::<Vec<_>>();
        if arguments.len() == 2
            && solid_facts_backend::is_compiler_certification_session_argument(&arguments[1])
        {
            solid_facts_backend::serve_compiler_certification_session()?;
            return Ok(0);
        }
    }
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
    let emits_contract = !request.emit_contract.is_empty();
    let emits_contract_batch = !request.emit_contract_batch.is_empty();
    if emits_contract && emits_contract_batch {
        return Err("--emit-contract and --emit-contract-batch are mutually exclusive".into());
    }
    if emits_contract_batch != !request.contract_batch_results.is_empty() {
        return Err(
            "--emit-contract-batch and --contract-batch-results are required together".into(),
        );
    }
    let mut contract_emission_batch = if emits_contract_batch {
        Some(read_contract_emission_batch(&request)?)
    } else {
        None
    };
    // The inventory attests *the generation run's* program. Asking for one
    // without asking for a contract would hand a caller an attestation with
    // nothing to attest, so it is refused rather than silently written.
    if !request.emit_module_inventory.is_empty() && !emits_contract {
        return Err("--emit-module-inventory requires --emit-contract".into());
    }
    if !request.runtime_module_resolutions.is_empty() && !emits_contract && !emits_contract_batch {
        return Err("--runtime-module-resolutions requires contract emission".into());
    }
    if !request.runtime_module_resolutions.is_empty() && request.contract_package_root.is_empty() {
        return Err("--runtime-module-resolutions requires --contract-package-root".into());
    }
    if emits_contract
        && (request.contract_resolution.is_empty() || request.emit_proposal_plan.is_empty())
    {
        return Err(
            "--emit-contract requires --contract-resolution and --emit-proposal-plan".into(),
        );
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
            validate_contract_document(&fs::read(path)?)?;
        }
        return Ok(0);
    }
    if !request.merge_contract_paths.is_empty() || !request.merge_contract_output.is_empty() {
        if request.merge_contract_paths.is_empty() || request.merge_contract_output.is_empty() {
            return Err(
                "--merge-contract and --merge-contract-output are required together".into(),
            );
        }
        let documents = request
            .merge_contract_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()?;
        let merged = merge_contract_proposals(documents.iter().map(Vec::as_slice), true)?;
        fs::write(&request.merge_contract_output, &merged)?;
        if request.merge_proposal_plan_paths.is_empty()
            || request.merge_proposal_plan_output.is_empty()
        {
            return Err(
                "proposal merge requires --merge-proposal-plan and --merge-proposal-plan-output"
                    .into(),
            );
        }
        if request.merge_proposal_plan_paths.len() != documents.len() {
            return Err(
                "proposal merge requires exactly one source plan for every source document".into(),
            );
        }
        let plans = request
            .merge_proposal_plan_paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()?;
        fs::write(
            &request.merge_proposal_plan_output,
            merge_plans(&merged, documents.into_iter().zip(plans))?,
        )?;
        return Ok(0);
    }
    if !request.review_contract.is_empty() || !request.review_output.is_empty() {
        if request.review_contract.is_empty() || request.review_output.is_empty() {
            return Err("--review-contract and --review-output are required together".into());
        }
        fs::write(
            &request.review_output,
            review_contract_document(&fs::read(&request.review_contract)?)?,
        )?;
        return Ok(0);
    }
    if !request.runtime_probe_proposal.is_empty()
        || !request.runtime_probe_proposal_plan.is_empty()
        || !request.runtime_probe_request.is_empty()
        || !request.runtime_probe_plan_output.is_empty()
        || !request.runtime_probe_runs.is_empty()
        || !request.runtime_probe_evaluation_output.is_empty()
    {
        if [
            &request.runtime_probe_proposal,
            &request.runtime_probe_proposal_plan,
            &request.runtime_probe_request,
            &request.runtime_probe_plan_output,
        ]
        .iter()
        .any(|value| value.is_empty())
        {
            return Err("runtime probe planning requires --runtime-probe-proposal, --runtime-probe-proposal-plan, --runtime-probe-request, and --runtime-probe-plan-output".into());
        }
        if request.runtime_probe_runs.is_empty()
            != request.runtime_probe_evaluation_output.is_empty()
        {
            return Err(
                "--runtime-probe-runs and --runtime-probe-evaluation-output are required together"
                    .into(),
            );
        }
        let planned = solid_facts_backend::plan_runtime_probes(
            &fs::read(&request.runtime_probe_proposal)?,
            &fs::read(&request.runtime_probe_proposal_plan)?,
            &fs::read(&request.runtime_probe_request)?,
        )?;
        fs::write(&request.runtime_probe_plan_output, planned.bytes())?;
        if !request.runtime_probe_runs.is_empty() {
            fs::write(
                &request.runtime_probe_evaluation_output,
                solid_facts_backend::evaluate_runtime_probe_runs(
                    &planned,
                    &fs::read(&request.runtime_probe_runs)?,
                )?,
            )?;
        }
        return Ok(0);
    }
    if !request.plan_contract_certification.is_empty()
        || !request.certification_plan_output.is_empty()
    {
        if request.plan_contract_certification.is_empty()
            || request.certification_plan_output.is_empty()
        {
            return Err("--plan-contract-certification and --certification-plan-output are required together".into());
        }
        write_contract_certification_plan(
            Path::new(&request.plan_contract_certification),
            Path::new(&request.certification_plan_output),
        )?;
        return Ok(0);
    }
    if !request.execute_contract_certification.is_empty() {
        execute_contract_certification(Path::new(&request.execute_contract_certification))?;
        return Ok(0);
    }
    if !request.verify_policy2_discovery.is_empty() {
        verify_policy2_discovery(Path::new(&request.verify_policy2_discovery))?;
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
    let declaration_probe = if request.declaration_probe_plan.is_empty() {
        None
    } else {
        Some(prepare_declaration_probe(Path::new(
            &request.declaration_probe_plan,
        ))?)
    };
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
    if let Some(probe) = declaration_probe {
        emit_declaration_probe_plan(&mut typescript, &probe)?;
        return Ok(0);
    }
    let sidecar_spawn_ns = started.elapsed().as_nanos();
    let mut sources_bytes = 0usize;
    let sources_wire_bytes = 0u64;
    if request.sources.is_empty() {
        request.sources = typescript.configured_sources()?;
        sources_bytes = request.sources.iter().map(|s| s.source.len()).sum();
    }
    let source_setup_ns = started.elapsed().as_nanos();
    let requested_enablement = RequestedRuleEnablement {
        presets: &request.presets,
        rules: &request.enable_rules,
        runtime: request.runtime.clone(),
    };
    let mut semantic_demand_options = if diagnostics {
        semantic_demand_options_for_enablement(
            dialect,
            Path::new(&request.project_id),
            requested_enablement.clone(),
        )?
    } else {
        SemanticDemandOptions::NONE
    };
    semantic_demand_options.contract_probe_parameters =
        !request.emit_contract.is_empty() || !request.emit_contract_batch.is_empty();
    if let Some(batch) = contract_emission_batch.take() {
        // The union project opens one pinned Type Facts program. Before any
        // fact reuse, native code independently canonicalizes every target's
        // complete source program and binds its source bytes and compiler
        // options into the key below. Request-level dialect, runtime, rule,
        // generation, Type Facts, and project inputs are bound as well. Only
        // exact equal keys share the fact build; attestations, catalogs, IR,
        // and entrypoint emission remain per target. The key is local to this
        // process invocation and cannot cross a certification or fresh replay
        // boundary.
        let configured_sources = request.sources.clone();
        let mut sources_by_path = HashMap::new();
        for source in &configured_sources {
            let path = Path::new(&source.path).canonicalize()?;
            if sources_by_path.insert(path, source.clone()).is_some() {
                return Err(
                    "contract emission batch project repeats a canonical source path".into(),
                );
            }
        }
        let package_root = Path::new(&request.contract_package_root).canonicalize()?;
        if !package_root.is_dir() {
            return Err("--contract-package-root must be a package directory".into());
        }
        let project_id = package_root
            .join("tsconfig.json")
            .to_string_lossy()
            .into_owned();
        let fact_context = ContractEmissionFactContext {
            dialect: dialect.id.into(),
            typefacts_project: request.project_id.clone(),
            typefacts_executable: request.typefacts_executable.clone(),
            typefacts_arguments: producer_arguments(&request.typefacts_args),
            generation: request.generation,
            semantic_demands: semantic_demand_options,
            runtime: request.runtime.clone(),
            presets: request.presets.clone(),
            enabled_rules: request.enable_rules.clone(),
        };
        struct FactProgramGroup {
            key: ContractEmissionFactProgramKey,
            targets: Vec<ContractEmissionBatchTarget>,
        }
        let target_order = batch
            .targets
            .iter()
            .map(|target| target.index)
            .collect::<Vec<_>>();
        let mut groups = Vec::<FactProgramGroup>::new();
        let mut outcomes_by_index = HashMap::with_capacity(batch.targets.len());
        let batch_started = Instant::now();
        for target in batch.targets {
            let target_index = target.index;
            let target_started = Instant::now();
            match contract_emission_target_sources(&target, &sources_by_path) {
                Ok(sources) => {
                    let key = ContractEmissionFactProgramKey {
                        context: fact_context.clone(),
                        sources,
                    };
                    match groups.iter_mut().find(|group| group.key == key) {
                        Some(group) => group.targets.push(target),
                        None => groups.push(FactProgramGroup {
                            key,
                            targets: vec![target],
                        }),
                    }
                }
                Err(error) => {
                    outcomes_by_index.insert(
                        target_index,
                        ContractEmissionBatchOutcome {
                            index: target_index,
                            success: false,
                            duration_ns: u64::try_from(target_started.elapsed().as_nanos())
                                .unwrap_or(u64::MAX),
                            error: Some(render_program_error(error.as_ref())),
                        },
                    );
                }
            }
        }
        let fact_programs = groups.len();
        let fact_program_group_sizes = groups
            .iter()
            .map(|group| group.targets.len())
            .collect::<Vec<_>>();
        for group in groups {
            let mut target_sources = group
                .key
                .sources
                .iter()
                .map(|source| source.source.clone())
                .collect::<Vec<_>>();
            target_sources.sort_by(|left, right| left.path.cmp(&right.path));
            let fact_started = Instant::now();
            let fact_result = build_project_native_measured_with_demands(
                dialect,
                request.project_id.clone(),
                request.generation,
                target_sources.clone(),
                &mut typescript,
                semantic_demand_options,
            );
            let shared_fact_duration_ns = fact_started.elapsed().as_nanos()
                / u128::try_from(group.targets.len()).unwrap_or(1).max(1);
            let (mut shared_facts, _) = match fact_result {
                Ok(result) => result,
                Err(error) => {
                    let rendered = render_program_error(&error);
                    for target in group.targets {
                        outcomes_by_index.insert(
                            target.index,
                            ContractEmissionBatchOutcome {
                                index: target.index,
                                success: false,
                                duration_ns: u64::try_from(shared_fact_duration_ns)
                                    .unwrap_or(u64::MAX),
                                error: Some(rendered.clone()),
                            },
                        );
                    }
                    continue;
                }
            };
            shared_facts.project_id.clone_from(&project_id);
            for target in group.targets {
                let target_index = target.index;
                let target_started = Instant::now();
                let outcome = (|| -> Result<(), Box<dyn std::error::Error>> {
                    // ProjectFacts is structurally shared (its file/compiler
                    // tables are immutable Arcs). Mutate only this target's
                    // exact import attestations and runtime redirects.
                    let mut facts = shared_facts.clone();
                    if !request.runtime_module_resolutions.is_empty() {
                        facts.runtime_symbol_redirects = runtime_symbol_redirects(
                            &facts,
                            &mut typescript,
                            &package_root,
                            Path::new(&request.runtime_module_resolutions),
                        )?;
                    }
                    let scope = contract_identity_scope(&facts);
                    if !scope.is_empty() {
                        let (index, _) = attest_import_identities(&mut typescript, &scope)?;
                        facts.resolved_imports = Some(index);
                    }

                    let discovered_catalog = if request.accepted_contract_catalog.is_empty() {
                        let candidate = package_root.join(".solid-checker/accepted-contracts.json");
                        candidate.is_file().then_some(candidate)
                    } else {
                        Some(PathBuf::from(&request.accepted_contract_catalog))
                    };
                    let bundled = bundled_first_party_contract_index(
                        dialect.id,
                        &package_root,
                        &facts,
                        &request.runtime,
                    )?;
                    let trust = (!request.receipt_trust_configuration.is_empty())
                        .then(|| {
                            read_policy2_trust_configuration(Path::new(
                                &request.receipt_trust_configuration,
                            ))
                        })
                        .transpose()?;
                    let contracts = discovered_catalog
                        .as_deref()
                        .map(|path| read_accepted_contract_catalog_with_trust(path, trust.as_ref()))
                        .transpose()?
                        .unwrap_or_default()
                        .with_fallback(bundled);
                    let contracts = if request.proposal_dependency_catalog.is_empty() {
                        contracts
                    } else {
                        if discovered_catalog.is_some() || trust.is_some() {
                            return Err("--proposal-dependencies cannot be combined with accepted-contract receipt authority".into());
                        }
                        read_proposal_dependency_catalog_for_generation(Path::new(
                            &request.proposal_dependency_catalog,
                        ))?
                        .with_fallback(contracts)
                    };
                    let (analysis, _) = analyze_project_accepted_measured_with_enablement(
                        dialect,
                        Path::new(&project_id),
                        &target_sources,
                        &facts,
                        &contracts,
                        requested_enablement.clone(),
                    )?;
                    let mut target_request = request.clone();
                    target_request.sources = target_sources.clone();
                    target_request.emit_contract = target.output;
                    target_request.emit_proposal_plan = target.plan;
                    target_request.contract_resolution = target.resolution;
                    target_request.contract_entry_file = target.entry_file;
                    emit_package_contract(&target_request, &analysis.program, &facts, &contracts)
                })();
                let duration_ns = u64::try_from(
                    shared_fact_duration_ns.saturating_add(target_started.elapsed().as_nanos()),
                )
                .unwrap_or(u64::MAX);
                let result = match outcome {
                    Ok(()) => ContractEmissionBatchOutcome {
                        index: target_index,
                        success: true,
                        duration_ns,
                        error: None,
                    },
                    Err(error) => ContractEmissionBatchOutcome {
                        index: target_index,
                        success: false,
                        duration_ns,
                        error: Some(render_program_error(error.as_ref())),
                    },
                };
                outcomes_by_index.insert(target_index, result);
            }
        }
        let outcomes = target_order
            .into_iter()
            .map(|index| {
                outcomes_by_index.remove(&index).ok_or_else(|| {
                    format!("contract emission batch target {index} produced no outcome")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut bytes = serde_json::to_vec(&outcomes)?;
        bytes.push(b'\n');
        fs::write(&request.contract_batch_results, bytes)?;
        if std::env::var_os("SOLID_CHECKER_TIMINGS").is_some() {
            eprintln!(
                "{}",
                serde_json::json!({
                    "sidecarSpawnNs": sidecar_spawn_ns,
                    "sourceSetupNs": source_setup_ns,
                    "contractBatchNs": batch_started.elapsed().as_nanos(),
                    "contractBatchTargets": outcomes.len(),
                    "contractBatchFactPrograms": fact_programs,
                    "contractBatchFactProgramGroupSizes": fact_program_group_sizes,
                })
            );
        }
        return Ok(0);
    }
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
        let project = Path::new(&facts.project_id);
        let directory = if project.is_dir() {
            project
        } else {
            project.parent().unwrap_or_else(|| Path::new("."))
        };
        let bundled =
            bundled_first_party_contract_index(dialect.id, directory, &facts, &request.runtime)?;
        let catalog = if request.accepted_contract_catalog.is_empty() {
            let candidate = directory.join(".solid-checker/accepted-contracts.json");
            candidate.is_file().then_some(candidate)
        } else {
            Some(PathBuf::from(&request.accepted_contract_catalog))
        };
        let trust = (!request.receipt_trust_configuration.is_empty())
            .then(|| {
                read_policy2_trust_configuration(Path::new(&request.receipt_trust_configuration))
            })
            .transpose()?;
        let contracts = catalog
            .as_deref()
            .map(|path| read_accepted_contract_catalog_with_trust(path, trust.as_ref()))
            .transpose()?
            .unwrap_or_default()
            .with_fallback(bundled);
        let statuses = accepted_package_contract_statuses(dialect, project, &facts, &contracts)?;
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
        let discovered_catalog = if request.accepted_contract_catalog.is_empty() {
            let project = Path::new(&facts.project_id);
            let directory = if project.is_dir() {
                project
            } else {
                project.parent().unwrap_or_else(|| Path::new("."))
            };
            let candidate = directory.join(".solid-checker/accepted-contracts.json");
            candidate.is_file().then_some(candidate)
        } else {
            Some(PathBuf::from(&request.accepted_contract_catalog))
        };
        let project = Path::new(&facts.project_id);
        let directory = if project.is_dir() {
            project
        } else {
            project.parent().unwrap_or_else(|| Path::new("."))
        };
        let bundled =
            bundled_first_party_contract_index(dialect.id, directory, &facts, &request.runtime)?;
        let trust = (!request.receipt_trust_configuration.is_empty())
            .then(|| {
                read_policy2_trust_configuration(Path::new(&request.receipt_trust_configuration))
            })
            .transpose()?;
        let contracts = discovered_catalog
            .as_deref()
            .map(|path| read_accepted_contract_catalog_with_trust(path, trust.as_ref()))
            .transpose()?
            .unwrap_or_default()
            .with_fallback(bundled);
        let contracts = if request.proposal_dependency_catalog.is_empty() {
            contracts
        } else {
            if request.emit_contract.is_empty() && request.emit_contract_batch.is_empty() {
                return Err("--proposal-dependencies is private to contract emission".into());
            }
            if discovered_catalog.is_some() || trust.is_some() {
                return Err(
                    "--proposal-dependencies cannot be combined with accepted-contract receipt authority"
                        .into(),
                );
            }
            read_proposal_dependency_catalog_for_generation(Path::new(
                &request.proposal_dependency_catalog,
            ))?
            .with_fallback(contracts)
        };
        let (analysis, diagnostic_timings) = analyze_project_accepted_measured_with_enablement(
            dialect,
            Path::new(&facts.project_id),
            &request.sources,
            &facts,
            &contracts,
            requested_enablement,
        )?;
        let contract_emission_ns = Cell::new(0_u128);
        let module_inventory_ns = Cell::new(0_u128);
        let report_timings = || {
            if std::env::var_os("SOLID_CHECKER_TIMINGS").is_none() {
                return;
            }
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
                    "contractBindingsBound": analysis.program.contract_binding.bound,
                    "contractBindingsRefused": analysis.program.contract_binding.refused,
                    "irNs": diagnostic_timings.reactive_ir.as_nanos(),
                    "solveAndSnapshotNs": diagnostic_timings.solve_and_snapshot.as_nanos(),
                    "contractEmissionNs": contract_emission_ns.get(),
                    "moduleInventoryNs": module_inventory_ns.get(),
                    "totalNs": started.elapsed().as_nanos(),
                })
            );
        };
        if !request.emit_contract.is_empty() {
            let phase_started = Instant::now();
            emit_package_contract(&request, &analysis.program, &facts, &contracts)?;
            contract_emission_ns.set(phase_started.elapsed().as_nanos());
            // After the contract, not before: a run that cannot emit a
            // contract has no closure record to attest, and writing the
            // inventory first would leave an attestation of a generation that
            // produced nothing.
            if !request.emit_module_inventory.is_empty() {
                let phase_started = Instant::now();
                write_module_inventory(&mut typescript, &request)?;
                module_inventory_ns.set(phase_started.elapsed().as_nanos());
            }
            // Emission normally produces no stdout, and the generator depends
            // on that. `--format json` is a caller explicitly asking for the
            // diagnostics of the same analysis, which is otherwise only
            // obtainable by running the whole project a second time -- and the
            // second run is the one that cannot see which obligations the
            // emitter attributed to which export. The default format is
            // untouched, so the generator's process contract is unchanged.
            if request.format != "json" {
                report_timings();
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
        report_timings();
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
    let mut accepted_contract_catalog = String::new();
    let mut receipt_trust_configuration = String::new();
    let mut proposal_dependency_catalog = String::new();
    let mut presets = Vec::new();
    let mut enable_rules = Vec::new();
    let mut format = "default".to_owned();
    let mut certify = false;
    let mut check_contracts = false;
    let mut validate_contract_paths = Vec::new();
    let mut emit_contract = String::new();
    let mut emit_contract_batch = String::new();
    let mut contract_batch_results = String::new();
    let mut declaration_probe_plan = String::new();
    let mut emit_module_inventory = String::new();
    let mut runtime_module_resolutions = String::new();
    let mut contract_resolution = String::new();
    let mut emit_proposal_plan = String::new();
    let mut merge_contract_paths = Vec::new();
    let mut merge_contract_output = String::new();
    let mut merge_proposal_plan_paths = Vec::new();
    let mut merge_proposal_plan_output = String::new();
    let mut review_contract = String::new();
    let mut review_output = String::new();
    let mut runtime_probe_proposal = String::new();
    let mut runtime_probe_proposal_plan = String::new();
    let mut runtime_probe_request = String::new();
    let mut runtime_probe_plan_output = String::new();
    let mut runtime_probe_runs = String::new();
    let mut runtime_probe_evaluation_output = String::new();
    let mut plan_contract_certification = String::new();
    let mut certification_plan_output = String::new();
    let mut execute_contract_certification = String::new();
    let mut verify_policy2_discovery = String::new();
    let mut package_name = String::new();
    let mut package_version = String::new();
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
        if let Some(value) = argument.strip_prefix("--accepted-contracts=") {
            accepted_contract_catalog = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--proposal-dependencies=") {
            proposal_dependency_catalog = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--receipt-trust-configuration=") {
            receipt_trust_configuration = value.into();
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
        if let Some(value) = argument.strip_prefix("--emit-contract-batch=") {
            emit_contract_batch = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--contract-batch-results=") {
            contract_batch_results = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--declaration-probe-plan=") {
            declaration_probe_plan = value.into();
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
        if let Some(value) = argument.strip_prefix("--contract-resolution=") {
            contract_resolution = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--emit-proposal-plan=") {
            emit_proposal_plan = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-contract=") {
            merge_contract_paths.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-contract-output=") {
            merge_contract_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-proposal-plan=") {
            merge_proposal_plan_paths.push(value.into());
            continue;
        }
        if let Some(value) = argument.strip_prefix("--merge-proposal-plan-output=") {
            merge_proposal_plan_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--review-contract=") {
            review_contract = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--review-output=") {
            review_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--package-name=") {
            package_name = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-proposal=") {
            runtime_probe_proposal = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-proposal-plan=") {
            runtime_probe_proposal_plan = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-request=") {
            runtime_probe_request = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-plan-output=") {
            runtime_probe_plan_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-runs=") {
            runtime_probe_runs = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--runtime-probe-evaluation-output=") {
            runtime_probe_evaluation_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--plan-contract-certification=") {
            plan_contract_certification = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--certification-plan-output=") {
            certification_plan_output = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--execute-contract-certification=") {
            execute_contract_certification = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--verify-policy2-discovery=") {
            verify_policy2_discovery = value.into();
            continue;
        }
        if let Some(value) = argument.strip_prefix("--package-version=") {
            package_version = value.into();
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
            "--accepted-contracts" => {
                accepted_contract_catalog =
                    args.next().ok_or("--accepted-contracts needs a path")?
            }
            "--proposal-dependencies" => {
                proposal_dependency_catalog =
                    args.next().ok_or("--proposal-dependencies needs a path")?
            }
            "--receipt-trust-configuration" => {
                receipt_trust_configuration = args
                    .next()
                    .ok_or("--receipt-trust-configuration needs a path")?
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
            "--emit-contract-batch" => {
                emit_contract_batch = args.next().ok_or("--emit-contract-batch needs a path")?
            }
            "--contract-batch-results" => {
                contract_batch_results =
                    args.next().ok_or("--contract-batch-results needs a path")?
            }
            "--declaration-probe-plan" => {
                declaration_probe_plan =
                    args.next().ok_or("--declaration-probe-plan needs a path")?
            }
            "--emit-module-inventory" => {
                emit_module_inventory = args.next().ok_or("--emit-module-inventory needs a path")?
            }
            "--runtime-module-resolutions" => {
                runtime_module_resolutions = args
                    .next()
                    .ok_or("--runtime-module-resolutions needs a path")?
            }
            "--contract-resolution" => {
                contract_resolution = args.next().ok_or("--contract-resolution needs a path")?
            }
            "--emit-proposal-plan" => {
                emit_proposal_plan = args.next().ok_or("--emit-proposal-plan needs a path")?
            }
            "--merge-contract" => {
                merge_contract_paths.push(args.next().ok_or("--merge-contract needs a path")?)
            }
            "--merge-contract-output" => {
                merge_contract_output = args.next().ok_or("--merge-contract-output needs a path")?
            }
            "--merge-proposal-plan" => merge_proposal_plan_paths
                .push(args.next().ok_or("--merge-proposal-plan needs a path")?),
            "--merge-proposal-plan-output" => {
                merge_proposal_plan_output = args
                    .next()
                    .ok_or("--merge-proposal-plan-output needs a path")?
            }
            "--review-contract" => {
                review_contract = args.next().ok_or("--review-contract needs a path")?
            }
            "--review-output" => {
                review_output = args.next().ok_or("--review-output needs a path")?
            }
            "--runtime-probe-proposal" => {
                runtime_probe_proposal =
                    args.next().ok_or("--runtime-probe-proposal needs a path")?
            }
            "--runtime-probe-proposal-plan" => {
                runtime_probe_proposal_plan = args
                    .next()
                    .ok_or("--runtime-probe-proposal-plan needs a path")?
            }
            "--runtime-probe-request" => {
                runtime_probe_request = args.next().ok_or("--runtime-probe-request needs a path")?
            }
            "--runtime-probe-plan-output" => {
                runtime_probe_plan_output = args
                    .next()
                    .ok_or("--runtime-probe-plan-output needs a path")?
            }
            "--runtime-probe-runs" => {
                runtime_probe_runs = args.next().ok_or("--runtime-probe-runs needs a path")?
            }
            "--runtime-probe-evaluation-output" => {
                runtime_probe_evaluation_output = args
                    .next()
                    .ok_or("--runtime-probe-evaluation-output needs a path")?
            }
            "--plan-contract-certification" => {
                plan_contract_certification = args
                    .next()
                    .ok_or("--plan-contract-certification needs a path")?
            }
            "--certification-plan-output" => {
                certification_plan_output = args
                    .next()
                    .ok_or("--certification-plan-output needs a path")?
            }
            "--execute-contract-certification" => {
                execute_contract_certification = args
                    .next()
                    .ok_or("--execute-contract-certification needs a path")?
            }
            "--verify-policy2-discovery" => {
                verify_policy2_discovery = args
                    .next()
                    .ok_or("--verify-policy2-discovery needs a path")?
            }
            "--package-name" => package_name = args.next().ok_or("--package-name needs a value")?,
            "--package-version" => {
                package_version = args.next().ok_or("--package-version needs a value")?
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
    let project = if !help
        && validate_contract_paths.is_empty()
        && merge_contract_paths.is_empty()
        && merge_contract_output.is_empty()
        && review_contract.is_empty()
        && runtime_probe_proposal.is_empty()
        && plan_contract_certification.is_empty()
        && execute_contract_certification.is_empty()
        && verify_policy2_discovery.is_empty()
    {
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
        accepted_contract_catalog,
        receipt_trust_configuration,
        proposal_dependency_catalog,
        presets,
        enable_rules,
        format,
        certify,
        check_contracts,
        validate_contract_paths,
        emit_contract,
        emit_contract_batch,
        contract_batch_results,
        declaration_probe_plan,
        emit_module_inventory,
        runtime_module_resolutions,
        contract_resolution,
        emit_proposal_plan,
        merge_contract_paths,
        merge_contract_output,
        merge_proposal_plan_paths,
        merge_proposal_plan_output,
        review_contract,
        review_output,
        runtime_probe_proposal,
        runtime_probe_proposal_plan,
        runtime_probe_request,
        runtime_probe_plan_output,
        runtime_probe_runs,
        runtime_probe_evaluation_output,
        plan_contract_certification,
        certification_plan_output,
        execute_contract_certification,
        verify_policy2_discovery,
        package_name,
        package_version,
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
           --accepted-contracts <PATH>  Load a host-acquired catalog of stable-v1\n\
                                        documents, proof receipts, and exact resolved imports\n\
           --receipt-trust-configuration <PATH>\n\
                                        Load policy-2 issuer trust selected outside the project\n\
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
           --contract-resolution <PATH> Full exact package/artifact resolution record used\n\
                                        to bind a stable-v1 proposal. Required by\n\
                                        --emit-contract\n\
           --package-name <NAME>        Package name used by --emit-contract\n\
           --package-version <VERSION>  Exact package version used by --emit-contract\n\
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
    solid_reactive_ir::ContractClaim::Open
}

fn mark_contract_generation_callback_unknown(summary: &mut solid_reactive_ir::ContractExport) {
    summary.callbacks = unknown_contract_claim();
}

fn contract_generation_obligation_target_names(
    entry_joined: bool,
    obligation: &solid_reactive_ir::ContractGenerationObligation,
    exports: &BTreeMap<String, solid_reactive_ir::ContractExport>,
    names_by_identity: &HashMap<String, Vec<String>>,
    names_by_symbol: &HashMap<String, Vec<String>>,
    aliases: &HashMap<String, String>,
) -> Vec<String> {
    if !entry_joined {
        return if exports.contains_key(&obligation.function) {
            vec![obligation.function.clone()]
        } else {
            Vec::new()
        };
    }
    if !obligation.function_symbol.is_empty() {
        let symbol = canonical_symbol(&obligation.function_symbol, aliases);
        return names_by_symbol.get(&symbol).cloned().unwrap_or_default();
    }
    if !obligation.function_identity.is_empty() {
        // No exact symbol: the obligation carries only its owning function's
        // runtime identity. A re-export barrel (`export * from "<dependency>"`)
        // can share one runtime identity across every binding it republishes,
        // and a symbol-less callback owner (an anonymous forwarder) then joins
        // to all of them. The obligation proves a *callback* is forwarded, so
        // its invocation subject is by construction callable; a value export
        // can never be that subject, and opening its callbacks domain only
        // manufactures the "value export cannot have function effects" refusal.
        // Without symbol provenance the identity match is not proof that any
        // particular value sibling is the subject, so it must never reach one.
        return names_by_identity
            .get(&obligation.function_identity)
            .map(|names| {
                names
                    .iter()
                    .filter(|name| {
                        exports
                            .get(name.as_str())
                            .is_none_or(|export| export.kind != "value")
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
    }
    if exports.contains_key(&obligation.function) {
        vec![obligation.function.clone()]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod contract_generation_callback_attribution_tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use super::{
        contract_exports_without_entry_file, contract_generation_obligation_target_names,
        mark_contract_generation_callback_unknown, reconcile_entry_export_kind,
    };
    use solid_reactive_ir::{
        ContractClaim, ContractExport, ContractGenerationObligation, ExportKindProof,
    };

    fn obligation(function_symbol: &str, function_identity: &str) -> ContractGenerationObligation {
        ContractGenerationObligation {
            function: "callableExport".into(),
            function_symbol: function_symbol.into(),
            function_identity: function_identity.into(),
            ..ContractGenerationObligation::default()
        }
    }

    #[test]
    fn no_entry_file_contracts_still_follow_the_declaration_surface() {
        let exports = BTreeMap::from([("declared".to_owned(), 1), ("runtimeOnly".to_owned(), 2)]);
        assert_eq!(
            contract_exports_without_entry_file(
                &exports,
                Some(&BTreeSet::from(["declared".to_owned()])),
            ),
            BTreeMap::from([("declared".to_owned(), 1)])
        );
        assert_eq!(
            contract_exports_without_entry_file(&exports, None),
            exports,
            "a missing declaration census is never proof that an export is absent"
        );
    }

    #[test]
    fn exact_function_symbol_excludes_value_siblings_that_share_runtime_identity() {
        let mut exports = BTreeMap::from([
            (
                "IR".into(),
                ContractExport {
                    kind: "value".into(),
                    ..ContractExport::default()
                },
            ),
            (
                "callableExport".into(),
                ContractExport {
                    kind: "function".into(),
                    ..ContractExport::default()
                },
            ),
        ]);
        let names_by_identity = HashMap::from([(
            "shared-module-identity".into(),
            vec!["IR".into(), "callableExport".into()],
        )]);
        let names_by_symbol =
            HashMap::from([("function-symbol".into(), vec!["callableExport".into()])]);

        let targets = contract_generation_obligation_target_names(
            true,
            &obligation("function-symbol", "shared-module-identity"),
            &exports,
            &names_by_identity,
            &names_by_symbol,
            &HashMap::new(),
        );
        for target in targets {
            mark_contract_generation_callback_unknown(exports.get_mut(&target).unwrap());
        }

        assert_eq!(exports["IR"].callbacks, ContractClaim::Known(Vec::new()));
        assert_eq!(exports["callableExport"].callbacks, ContractClaim::Open);
    }

    #[test]
    fn an_unmatched_exact_function_symbol_never_falls_back_to_runtime_identity() {
        let exports = BTreeMap::from([(
            "IR".into(),
            ContractExport {
                kind: "value".into(),
                ..ContractExport::default()
            },
        )]);
        let names_by_identity =
            HashMap::from([("shared-module-identity".into(), vec!["IR".into()])]);

        assert!(
            contract_generation_obligation_target_names(
                true,
                &obligation("function-symbol", "shared-module-identity"),
                &exports,
                &names_by_identity,
                &HashMap::new(),
                &HashMap::new(),
            )
            .is_empty()
        );
    }

    #[test]
    fn an_empty_function_symbol_never_falls_back_onto_a_value_sibling() {
        // The real wrapper shape: a `export * from "<dependency>"` barrel shares
        // one runtime identity across the module-namespace value export (`IR`)
        // and a callable sibling. A symbol-less callback owner (an anonymous
        // forwarder in the wrapper) reaches the barrel identity, and the pre-fix
        // fallback marked every joined name -- opening `callbacks` on the value
        // export `IR`, which the operation-graph invariant then refuses with
        // "value export .:IR cannot have function effects". The fallback must
        // reach only callable siblings it could plausibly be the subject of.
        let mut exports = BTreeMap::from([
            (
                "IR".into(),
                ContractExport {
                    kind: "value".into(),
                    ..ContractExport::default()
                },
            ),
            (
                "callableExport".into(),
                ContractExport {
                    kind: "function".into(),
                    ..ContractExport::default()
                },
            ),
        ]);
        let names_by_identity = HashMap::from([(
            "shared-barrel-identity".into(),
            vec!["IR".into(), "callableExport".into()],
        )]);

        let targets = contract_generation_obligation_target_names(
            true,
            // Empty function symbol: the owning callback forwarder is anonymous,
            // so only the runtime identity is available.
            &obligation("", "shared-barrel-identity"),
            &exports,
            &names_by_identity,
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(targets, vec!["callableExport".to_string()]);
        for target in targets {
            mark_contract_generation_callback_unknown(exports.get_mut(&target).unwrap());
        }

        assert_eq!(exports["IR"].callbacks, ContractClaim::Known(Vec::new()));
        assert_eq!(exports["callableExport"].callbacks, ContractClaim::Open);
    }

    #[test]
    fn an_exact_invocation_symbol_preserves_a_real_export_kind_conflict() {
        let mut exports = BTreeMap::from([(
            "createGeolocation".into(),
            ContractExport {
                kind: "value".into(),
                ..ContractExport::default()
            },
        )]);
        let names_by_symbol = HashMap::from([(
            "geolocation-function-symbol".into(),
            vec!["createGeolocation".into()],
        )]);
        let targets = contract_generation_obligation_target_names(
            true,
            &obligation("geolocation-function-symbol", "module-identity"),
            &exports,
            &HashMap::new(),
            &names_by_symbol,
            &HashMap::new(),
        );
        for target in targets {
            mark_contract_generation_callback_unknown(exports.get_mut(&target).unwrap());
        }

        let summary = exports.remove("createGeolocation").unwrap();
        assert_eq!(summary.callbacks, ContractClaim::Open);
        assert!(
            reconcile_entry_export_kind(ExportKindProof::NonCallable, summary)
                .unwrap_err()
                .contains("cannot have function effects")
        );
    }
}

#[derive(Clone, Copy)]
struct UnresolvedExportIndex<'a> {
    facts: &'a solid_facts::ProjectFacts,
    aliases: &'a HashMap<String, String>,
    files_by_path: &'a HashMap<&'a str, &'a solid_facts::FileFacts>,
    entities_by_location: &'a HashMap<typefacts::Location, &'a typefacts::EntityFact>,
    entities: &'a [&'a typefacts::EntityFact],
    entity_indexes_by_runtime_identity: &'a HashMap<&'a str, Vec<usize>>,
    entity_indexes_by_symbol: &'a HashMap<String, Vec<usize>>,
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

/// How an open semantic leaf was attributed to the exports it affects.
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
    let symbol = index.entities_by_location.get(&entity_location).copied()?;
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
    index.files_by_path.get(path).copied()
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
    runtime_canonical_symbol_from(index.facts, index.aliases, symbol)
}

fn runtime_canonical_symbol_from(
    facts: &solid_facts::ProjectFacts,
    aliases: &HashMap<String, String>,
    symbol: &str,
) -> String {
    let mut current = canonical_symbol(symbol, aliases);
    let mut seen = HashSet::new();
    while seen.insert(current.clone()) {
        let Some(next) = facts.runtime_symbol_redirects.get(&current) else {
            break;
        };
        current = canonical_symbol(next, aliases);
    }
    current
}

fn matching_entity_indexes(
    index: UnresolvedExportIndex<'_>,
    runtime_identity: &str,
    symbol: &str,
) -> BTreeSet<usize> {
    let mut matches = BTreeSet::new();
    if !runtime_identity.is_empty()
        && let Some(indexes) = index
            .entity_indexes_by_runtime_identity
            .get(runtime_identity)
    {
        matches.extend(indexes.iter().copied());
    }
    if !symbol.is_empty()
        && let Some(indexes) = index.entity_indexes_by_symbol.get(symbol)
    {
        matches.extend(indexes.iter().copied());
    }
    matches
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
    let declaration_location = typefacts::Location {
        path: file.path.to_string().into(),
        start_byte: u64::from(name.span.start),
        end_byte: u64::from(name.span.end),
    };
    let Some(declaration) = index
        .entities_by_location
        .get(&declaration_location)
        .copied()
    else {
        return false;
    };
    let identity = declaration.runtime_identity.as_ref();
    let symbol = runtime_canonical_symbol(index, &declaration.symbol);
    let referenced_elsewhere = matching_entity_indexes(index, identity, &symbol)
        .into_iter()
        .any(|entity_index| {
            index.entities[entity_index].location.path.as_ref() != file.path.as_str()
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
    if let Some(seed) = index.entities_by_location.get(location).copied() {
        let seed_symbol = runtime_canonical_symbol(index, &seed.symbol);
        let seed_identity = seed.runtime_identity.as_ref();
        let mut widened = Vec::new();
        for reference_index in matching_entity_indexes(index, seed_identity, &seed_symbol) {
            let reference = index.entities[reference_index];
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
                summary.reactive_reads.is_open()
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

/// Machine-readable context for a locally unresolved proposal domain.
///
/// The normalized proposal remains the authority; this stderr record explains
/// why attribution widened or found no export when a generation run refuses.
/// It is diagnostic provenance only and is never decoded as contract semantics.
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
    // proposal claim will carry it -- but "the ladder resolved this obligation
    // to no export at all" is a narrowing decision, and the proposal plan is
    // where a narrowing decision has to be visible. Leaving it silent
    // made the interesting case (reachability proving no export reaches the
    // obligation) indistinguishable from the analyzer never having seen the
    // obligation, and the reviewer had nothing to check the narrowing against.
    // would make that refusal indistinguishable from an analysis that never
    // observed the obligation.
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
        "unknownImportPaths": graph
            .unknown_import_paths
            .iter()
            .collect::<Vec<_>>(),
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
    request: &Request,
    program: &solid_reactive_ir::Program,
    facts: &solid_facts::ProjectFacts,
    contracts: &solid_reactive_ir::contract_semantics::AcceptedContractIndex,
) -> Result<(), Box<dyn std::error::Error>> {
    if request.package_name.is_empty() {
        return Err("--package-name is required with --emit-contract".into());
    }
    if request.package_version.is_empty() {
        return Err("--package-version is required with --emit-contract".into());
    }
    let resolution: solid_facts_backend::ResolvedImport =
        serde_json::from_slice(&fs::read(&request.contract_resolution)?)?;
    // SC9 findings are proof obligations, not permission to discard every
    // independently known export. After resolving the requested entrypoint we
    // attribute each one to the narrowest claim domain it can invalidate and
    // keep that exact semantic leaf open. Consumers then fail closed only
    // when they demand that claim. Proven violations remain diagnostics, but
    // they do not alter the package's descriptive runtime contract.
    let output = Path::new(&request.emit_contract);
    let mut files_by_canonical_path = HashMap::new();
    for file in &facts.files {
        files_by_canonical_path
            .entry(PathBuf::from(file.path.as_str()))
            .or_insert(file);
        if let Ok(path) = Path::new(file.path.as_str()).canonicalize() {
            files_by_canonical_path.entry(path).or_insert(file);
        }
    }
    let entities = facts.typescript.entities().collect::<Vec<_>>();
    let entities_by_location = entities
        .iter()
        .map(|entity| (entity.location.clone(), *entity))
        .collect::<HashMap<_, _>>();
    let declaration_export_names =
        (!resolution.declaration_exports.is_empty()).then_some(&resolution.declaration_exports);
    let mut exports = if request.contract_entry_file.is_empty() {
        contract_exports_without_entry_file(
            program.contract_exports.as_ref(),
            declaration_export_names,
        )
    } else {
        contract_exports_for_entry_file(
            facts,
            program,
            Path::new(&request.contract_entry_file),
            &files_by_canonical_path,
            &entities_by_location,
            contracts,
            declaration_export_names,
        )?
    };
    let entry_entities_by_name = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        exports
            .keys()
            .filter_map(|name| {
                entry_export_entity_indexed(
                    facts,
                    &files_by_canonical_path,
                    &entities_by_location,
                    &entry_file,
                    name,
                    &mut HashSet::new(),
                )
                .map(|entity| (name.clone(), entity))
            })
            .collect::<HashMap<_, _>>()
    };
    let exported_names_by_identity = if request.contract_entry_file.is_empty() {
        HashMap::new()
    } else {
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(identity) = entry_entities_by_name
                .get(name)
                .copied()
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
        let mut names = HashMap::<String, Vec<String>>::new();
        for name in exports.keys() {
            let Some(symbol) = entry_entities_by_name
                .get(name)
                .copied()
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
    let mut files_by_path = HashMap::new();
    for file in &facts.files {
        files_by_path.entry(file.path.as_str()).or_insert(file);
    }
    // Type Facts should carry one entity per exact span, but attribution's
    // historical linear `find` chose the first if a producer ever repeated a
    // location. Preserve that fail-closed ordering rather than inheriting the
    // last-write behavior of the entry-export lookup above.
    let mut attribution_entities_by_location = HashMap::new();
    let mut entity_indexes_by_runtime_identity = HashMap::<&str, Vec<usize>>::new();
    let mut entity_indexes_by_symbol = HashMap::<String, Vec<usize>>::new();
    for (entity_index, entity) in entities.iter().copied().enumerate() {
        attribution_entities_by_location
            .entry(entity.location.clone())
            .or_insert(entity);
        if !entity.runtime_identity.is_empty() {
            entity_indexes_by_runtime_identity
                .entry(entity.runtime_identity.as_ref())
                .or_default()
                .push(entity_index);
        }
        let symbol = runtime_canonical_symbol_from(facts, &symbol_aliases, &entity.symbol);
        if !symbol.is_empty() {
            entity_indexes_by_symbol
                .entry(symbol)
                .or_default()
                .push(entity_index);
        }
    }
    let unresolved_export_index = UnresolvedExportIndex {
        facts,
        aliases: &symbol_aliases,
        files_by_path: &files_by_path,
        entities_by_location: &attribution_entities_by_location,
        entities: &entities,
        entity_indexes_by_runtime_identity: &entity_indexes_by_runtime_identity,
        entity_indexes_by_symbol: &entity_indexes_by_symbol,
        names_by_identity: &exported_names_by_identity,
        names_by_symbol: &exported_names_by_symbol,
        entry_joined: !request.contract_entry_file.is_empty(),
        exports_fully_joined: exports
            .keys()
            .all(|name| joined_export_names.contains(name)),
        obligation_reach: &program.obligation_reach,
    };
    for unresolved in &program.contract_generation_obligations {
        let target_names = contract_generation_obligation_target_names(
            !request.contract_entry_file.is_empty(),
            unresolved,
            &exports,
            &exported_names_by_identity,
            &exported_names_by_symbol,
            &symbol_aliases,
        );
        let mut marked = Vec::new();
        for name in target_names {
            let Some(summary) = exports.get_mut(&name) else {
                continue;
            };
            // The obligation proves only that the callback list is
            // incomplete. Preserve every independently known claim and make
            // the uncertainty explicit instead of refusing the whole export.
            mark_contract_generation_callback_unknown(summary);
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
    // Unresolved-claim attribution deliberately enriches the inferred
    // summaries after the initial export-kind pass. Reconcile once more at
    // this final per-export boundary so a closed non-callable value can never
    // acquire function-only open domains and fail later as a generic graph
    // validation error (the published geolocation artifact is the regression
    // case). This is a proof check, not a cleanup: inconsistent summaries are
    // refused with their exact export-kind conflict.
    if !request.contract_entry_file.is_empty() {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        for (name, summary) in &mut exports {
            let current = std::mem::take(summary);
            *summary = promote_entry_callable(
                facts,
                &entry_file,
                name,
                entry_entities_by_name.get(name).copied(),
                current,
            )?;
        }
    }
    let mut external_targets = BTreeSet::new();
    if !request.contract_entry_file.is_empty() {
        let entry_file = Path::new(&request.contract_entry_file).canonicalize()?;
        for name in exports.keys() {
            if accepted_reexport_summary_for_name(
                facts,
                &files_by_canonical_path,
                contracts,
                &entry_file,
                name,
            )?
            .is_none()
            {
                continue;
            }
            if let Some(binding) = resolution.exports.get(name) {
                external_targets.extend([
                    (
                        binding.runtime.module.path.clone(),
                        binding.runtime.module.digest.clone(),
                    ),
                    (
                        binding.declarations.module.path.clone(),
                        binding.declarations.module.digest.clone(),
                    ),
                ]);
            }
        }
    }
    let proposal = encode_inferred_entrypoint_workflow_with_external_targets(
        &request.package_name,
        &request.package_version,
        &resolution.requested_entrypoint,
        exports,
        &resolution,
        &external_targets,
        true,
    )?;
    fs::write(output, proposal.document)?;
    fs::write(&request.emit_proposal_plan, proposal.plan)?;
    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeclarationProbeRequest {
    source_path: String,
    target: String,
    output: String,
    exports: BTreeMap<String, usize>,
}

#[derive(Clone)]
struct DeclarationCall {
    export: String,
    location: typefacts::Location,
}

fn append_declaration_call(
    source: &mut String,
    path: &str,
    export: &str,
    arguments: &[String],
) -> DeclarationCall {
    let start = source.len();
    source.push_str("__solid_checker_package[");
    source.push_str(&serde_json::to_string(export).expect("a string always serializes"));
    source.push_str("](");
    source.push_str(&arguments.join(", "));
    source.push(')');
    let end = source.len();
    source.push_str(";\n");
    DeclarationCall {
        export: export.to_owned(),
        location: typefacts::Location {
            path: path.to_owned().into(),
            start_byte: u64::try_from(start).unwrap_or(u64::MAX),
            end_byte: u64::try_from(end).unwrap_or(u64::MAX),
        },
    }
}

fn declaration_source_prefix(probe: &DeclarationProbeRequest) -> Result<String, serde_json::Error> {
    let source = format!(
        "import * as __solid_checker_package from {};\n",
        serde_json::to_string(&probe.target)?
    );
    Ok(source)
}

fn prepare_declaration_probe(
    path: &Path,
) -> Result<DeclarationProbeRequest, Box<dyn std::error::Error>> {
    let request: DeclarationProbeRequest = serde_json::from_slice(&fs::read(path)?)?;
    if request.exports.len() > 1024 || request.exports.values().any(|arity| *arity > 16) {
        return Err("declaration probe plan exceeds its bounded export/arity limit".into());
    }
    let mut source = declaration_source_prefix(&request)?;
    let source_path = Path::new(&request.source_path);
    for (export, arity) in &request.exports {
        append_declaration_call(
            &mut source,
            &request.source_path,
            export,
            &vec!["null as never".to_owned(); *arity],
        );
    }
    fs::write(source_path, source)?;
    Ok(request)
}

fn object_recipe(shape: &typefacts::ObjectConstructionShape) -> Option<serde_json::Value> {
    let mut properties = serde_json::Map::new();
    for property in shape.required_properties.iter() {
        let witness = match property.witness {
            typefacts::ConstructionWitness::EmptyArray => "empty-array",
            typefacts::ConstructionWitness::EmptyObject => "empty-object",
            typefacts::ConstructionWitness::Unknown => return None,
        };
        properties.insert(
            property.name.to_string(),
            serde_json::Value::String(witness.to_owned()),
        );
    }
    Some(serde_json::json!({ "kind": "object", "properties": properties }))
}

fn recipe_javascript(recipe: &serde_json::Value) -> Option<String> {
    match recipe {
        serde_json::Value::String(value) if value == "empty-array" => Some("[]".into()),
        serde_json::Value::String(value) if value == "empty-object" => Some("{}".into()),
        serde_json::Value::Object(object) if object.get("kind")?.as_str()? == "object" => {
            let properties = object.get("properties")?.as_object()?;
            let mut rendered = Vec::with_capacity(properties.len());
            for (name, value) in properties {
                rendered.push(format!(
                    "{}: {}",
                    serde_json::to_string(name).ok()?,
                    recipe_javascript(value)?
                ));
            }
            Some(format!("{{{}}}", rendered.join(", ")))
        }
        serde_json::Value::Object(object) if object.get("kind")?.as_str()? == "factory" => {
            let export = object.get("export")?.as_str()?;
            let arguments = object.get("arguments")?.as_array()?;
            let arguments = arguments
                .iter()
                .map(recipe_javascript)
                .collect::<Option<Vec<_>>>()?;
            Some(format!(
                "__solid_checker_package[{}]({})",
                serde_json::to_string(export).ok()?,
                arguments.join(", ")
            ))
        }
        _ => None,
    }
}

fn resolved_calls_by_location(
    table: &solid_facts::TypeScriptTable,
) -> HashMap<typefacts::Location, &typefacts::ResolvedCall> {
    table
        .entities()
        .filter_map(|entity| {
            entity
                .resolved_call
                .as_deref()
                .map(|call| (entity.location.clone(), call))
        })
        .collect()
}

fn query_declaration_calls(
    typescript: &mut TypeFactsSession,
    calls: &[DeclarationCall],
    object_shapes: bool,
) -> Result<solid_facts::TypeScriptTable, BackendError> {
    typescript.semantic(
        calls
            .iter()
            .map(|call| typefacts::v3::EntityDemand {
                location: call.location.clone(),
                resolved_call: true,
                parameter_object_shape: object_shapes,
                ..typefacts::v3::EntityDemand::default()
            })
            .collect(),
    )
}

fn emit_declaration_probe_plan(
    typescript: &mut TypeFactsSession,
    probe: &DeclarationProbeRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    if probe.exports.is_empty() {
        let fragment = ProbePlanFragment {
            schema_version: 2,
            source: "typescript-value-domain",
            exports: BTreeMap::new(),
        };
        fs::write(
            &probe.output,
            format!("{}\n", serde_json::to_string_pretty(&fragment)?),
        )?;
        return Ok(());
    }
    let mut discovery_source = declaration_source_prefix(probe)?;
    let mut discovery_calls = Vec::new();
    for (export, arity) in &probe.exports {
        discovery_calls.push(append_declaration_call(
            &mut discovery_source,
            &probe.source_path,
            export,
            &vec!["null as never".to_owned(); *arity],
        ));
    }
    let discovery = query_declaration_calls(typescript, &discovery_calls, true)?;
    let discovered = resolved_calls_by_location(&discovery);
    let mut candidates = Vec::<(String, usize, serde_json::Value)>::new();
    for call in &discovery_calls {
        let Some(resolved) = discovered.get(&call.location) else {
            continue;
        };
        for argument in resolved.arguments.iter() {
            let Some(parameter) = argument.parameter.as_ref() else {
                continue;
            };
            let Some(shape) = parameter.object_shape.as_ref() else {
                continue;
            };
            let Some(recipe) = object_recipe(shape) else {
                continue;
            };
            let Ok(index) = usize::try_from(argument.argument_index) else {
                continue;
            };
            candidates.push((call.export.clone(), index, recipe));
        }
    }

    // Validate each completed candidate through the ordinary TypeScript call
    // validity path. Shape discovery alone is construction input, never proof
    // that generic inference accepts the completed expression.
    let mut validation_source = declaration_source_prefix(probe)?;
    let mut validation_calls = Vec::new();
    for (export, index, recipe) in &candidates {
        let Some(arity) = probe.exports.get(export) else {
            continue;
        };
        if index >= arity {
            continue;
        }
        let mut arguments = vec!["null as never".to_owned(); *arity];
        let Some(rendered) = recipe_javascript(recipe) else {
            continue;
        };
        arguments[*index] = rendered;
        validation_calls.push(append_declaration_call(
            &mut validation_source,
            &probe.source_path,
            export,
            &arguments,
        ));
    }
    typescript.update(vec![typefacts::v3::FileChange {
        path: probe.source_path.clone(),
        source: validation_source.into_bytes(),
        deleted: false,
        version: 1,
    }])?;
    let validation = query_declaration_calls(typescript, &validation_calls, false)?;
    let validated_calls = resolved_calls_by_location(&validation);
    let mut exports = BTreeMap::<String, BTreeMap<usize, Vec<serde_json::Value>>>::new();
    for ((export, index, recipe), call) in candidates.iter().zip(&validation_calls) {
        if validated_calls
            .get(&call.location)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
        {
            exports
                .entry(export.clone())
                .or_default()
                .entry(*index)
                .or_default()
                .push(recipe.clone());
        }
    }

    for parameters in exports.values_mut() {
        for recipes in parameters.values_mut() {
            let mut seen = HashSet::new();
            recipes.retain(|recipe| seen.insert(recipe.to_string()));
        }
    }
    let fragment = ProbePlanFragment {
        schema_version: 2,
        source: "typescript-value-domain",
        exports,
    };
    fs::write(
        &probe.output,
        format!("{}\n", serde_json::to_string_pretty(&fragment)?),
    )?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbePlanFragment {
    schema_version: u32,
    source: &'static str,
    exports: BTreeMap<String, BTreeMap<usize, Vec<serde_json::Value>>>,
}

fn contract_exports_for_entry_file(
    facts: &solid_facts::ProjectFacts,
    program: &solid_reactive_ir::Program,
    entry_file: &Path,
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
    entities_by_location: &HashMap<typefacts::Location, &typefacts::EntityFact>,
    contracts: &solid_reactive_ir::contract_semantics::AcceptedContractIndex,
    declaration_export_names: Option<&BTreeSet<String>>,
) -> Result<BTreeMap<String, solid_reactive_ir::ContractExport>, Box<dyn std::error::Error>> {
    let entry_file = entry_file.canonicalize()?;
    let mut visiting = HashSet::new();
    let mut names = exported_names_for_file(
        facts,
        files_by_canonical_path,
        contracts,
        &entry_file,
        &mut visiting,
    )?;
    // Contracts describe the executable names that are also present on the
    // package's TypeScript surface. Filter before summary attribution: a
    // runtime-only export-star member has no declaration identity, so trying
    // to summarize it would manufacture an IdentityMismatch for a name that
    // TypeScript consumers cannot import. This uses the declaration-axis
    // census, not `resolution.exports`; certification independently replays
    // the census from archive bytes, and an omitted shared binding therefore
    // still reaches the existing exact-identity refusal.
    if let Some(declaration_export_names) = declaration_export_names {
        names.retain(|name| declaration_export_names.contains(name));
    }
    let entry_entities_by_name = names
        .iter()
        .filter_map(|name| {
            entry_export_entity_indexed(
                facts,
                files_by_canonical_path,
                entities_by_location,
                &entry_file,
                name,
                &mut HashSet::new(),
            )
            .map(|entity| (name.clone(), entity))
        })
        .collect::<HashMap<_, _>>();
    let symbol_aliases = canonical_symbol_aliases(facts);
    let generated_owner_requirements =
        generated_owner_requirements_by_symbol(facts, program, &symbol_aliases);
    let entry_facts = files_by_canonical_path
        .get(&entry_file)
        .copied()
        .ok_or_else(|| {
            format!(
                "emit package contract: entry file {} is not part of the TypeScript project",
                entry_file.display()
            )
        })?;
    let mut exports = BTreeMap::new();
    for name in names {
        validate_module_export_precedence(&entry_facts.ast, &entry_file, &name)?;
        let summary = match program.contract_exports.get(&name).cloned() {
            Some(summary) => summary,
            None => accepted_reexport_summary_for_name(
                facts,
                files_by_canonical_path,
                contracts,
                &entry_file,
                &name,
            )?
            .ok_or_else(|| {
                format!(
                    "emit package contract: entry file {} exports {name:?}, but no semantic summary was produced",
                    entry_file.display()
                )
            })?,
        };
        let summary = promote_entry_callable(
            facts,
            &entry_file,
            &name,
            entry_entities_by_name.get(&name).copied(),
            summary,
        )?;
        let summary = attach_generated_owner_requirements(
            facts,
            &symbol_aliases,
            &generated_owner_requirements,
            &entry_file,
            &name,
            entry_entities_by_name.get(&name).copied(),
            summary,
        );
        exports.insert(name, summary);
    }
    unify_runtime_alias_summaries(&entry_entities_by_name, &mut exports);
    if exports.is_empty() {
        return Err(format!(
            "emit package contract: entry file {} has no runtime ESM exports",
            entry_file.display()
        )
        .into());
    }
    Ok(exports)
}

fn contract_exports_without_entry_file<T: Clone>(
    exports: &BTreeMap<String, T>,
    declaration_export_names: Option<&BTreeSet<String>>,
) -> BTreeMap<String, T> {
    let Some(declaration_export_names) = declaration_export_names else {
        return exports.clone();
    };
    exports
        .iter()
        .filter(|(name, _)| declaration_export_names.contains(name.as_str()))
        .map(|(name, export)| (name.clone(), export.clone()))
        .collect()
}

type AcceptedReexportIdentity = (
    solid_reactive_ir::contract_semantics::AcceptedSemanticIdentity,
    solid_reactive_ir::contract_semantics::ExportIdentity,
);

/// Projects an external re-export only when exact accepted-contract identity
/// proves a single runtime origin for the public name.
///
/// `Program::contract_exports` is symbol-indexed and therefore has no local
/// symbol for a bare external `export *`. The accepted contract is the
/// semantic owner of that surface. Keeping this adapter at emission time
/// avoids manufacturing a local symbol while still preserving all receipt,
/// importer, artifact-case, and export-target identity.
fn accepted_reexport_summary_for_name(
    facts: &solid_facts::ProjectFacts,
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
    contracts: &solid_reactive_ir::contract_semantics::AcceptedContractIndex,
    entry_file: &Path,
    name: &str,
) -> Result<Option<solid_reactive_ir::ContractExport>, Box<dyn std::error::Error>> {
    let mut candidates = BTreeMap::new();
    collect_accepted_reexport_candidates(
        facts,
        files_by_canonical_path,
        contracts,
        entry_file,
        name,
        &mut HashSet::new(),
        &mut candidates,
    )?;
    match take_unique_reexport_candidate(candidates) {
        Ok(candidate) => Ok(candidate),
        Err(count) => Err(format!(
            "emit package contract: entry file {} re-exports {name:?} from {count} distinct accepted runtime identities",
            entry_file.display()
        )
        .into()),
    }
}

fn collect_accepted_reexport_candidates(
    facts: &solid_facts::ProjectFacts,
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
    contracts: &solid_reactive_ir::contract_semantics::AcceptedContractIndex,
    path: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
    candidates: &mut BTreeMap<AcceptedReexportIdentity, solid_reactive_ir::ContractExport>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    if !visiting.insert((path.clone(), name.to_owned())) {
        return Ok(());
    }
    let file = files_by_canonical_path.get(&path).copied().ok_or_else(|| {
        format!(
            "emit package contract: entry file {} is not part of the TypeScript project",
            path.display()
        )
    })?;
    let consult_export_stars = validate_module_export_precedence(&file.ast, &path, name)?;

    for export in reexport_entries_for_name(&file.ast, name, consult_export_stars) {
        if is_bare_runtime_export_star(export) {
            if !consult_export_stars {
                continue;
            }
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            if module.starts_with('.') {
                let target = resolve_relative_export(facts, &path, module)?;
                collect_accepted_reexport_candidates(
                    facts,
                    files_by_canonical_path,
                    contracts,
                    &target,
                    name,
                    visiting,
                    candidates,
                )?;
            } else if let Ok(accepted) = contracts.resolve_name(file.path.as_str(), module, name) {
                candidates.insert(
                    (
                        accepted.contract().semantic_identity(),
                        accepted.identity().clone(),
                    ),
                    solid_reactive_ir::project_accepted_export(&accepted),
                );
            }
            continue;
        }

        let Some(specifier) = export
            .specifiers
            .iter()
            .find(|specifier| !specifier.type_only && specifier.exported == name)
        else {
            continue;
        };
        let imported_name = export_specifier_local_name(&file.source, specifier, name);
        if let Some(module) = export.module.as_deref() {
            if module.starts_with('.') {
                let target = resolve_relative_export(facts, &path, module)?;
                collect_accepted_reexport_candidates(
                    facts,
                    files_by_canonical_path,
                    contracts,
                    &target,
                    imported_name,
                    visiting,
                    candidates,
                )?;
            } else if let Ok(accepted) =
                contracts.resolve_name(file.path.as_str(), module, imported_name)
            {
                candidates.insert(
                    (
                        accepted.contract().semantic_identity(),
                        accepted.identity().clone(),
                    ),
                    solid_reactive_ir::project_accepted_export(&accepted),
                );
            }
            continue;
        }

        // `import { child as local } from "dependency"; export { local }`
        // carries the same exact public identity as a direct named re-export,
        // but the export fact itself has no module. Follow only its matching
        // module-level import binding. Unresolved and namespace bindings add
        // no candidate, preserving the fail-closed boundary.
        for import in file.ast.imports.iter().filter(|import| !import.type_only) {
            for binding in import.bindings.iter().filter(|binding| {
                !binding.type_only && file.source_text(binding.local.span) == Some(imported_name)
            }) {
                let dependency_name = match binding.kind {
                    solid_facts::ast::ImportKind::Named => binding.imported.as_deref(),
                    solid_facts::ast::ImportKind::Default => Some("default"),
                    _ => None,
                };
                let Some(dependency_name) = dependency_name else {
                    continue;
                };
                if import.module.starts_with('.') {
                    let target = resolve_relative_export(facts, &path, &import.module)?;
                    collect_accepted_reexport_candidates(
                        facts,
                        files_by_canonical_path,
                        contracts,
                        &target,
                        dependency_name,
                        visiting,
                        candidates,
                    )?;
                } else if let Ok(accepted) = contracts.resolve_name(
                    file.path.as_str(),
                    import.module.as_str(),
                    dependency_name,
                ) {
                    candidates.insert(
                        (
                            accepted.contract().semantic_identity(),
                            accepted.identity().clone(),
                        ),
                        solid_reactive_ir::project_accepted_export(&accepted),
                    );
                }
            }
        }
    }
    Ok(())
}

fn take_unique_reexport_candidate<K: Ord, V>(
    candidates: BTreeMap<K, V>,
) -> Result<Option<V>, usize> {
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.into_values().next()),
        count => Err(count),
    }
}

/// Whether this syntax entry contributes every non-default name from its
/// target module. `export * as namespace` is an explicit namespace binding,
/// never an export-star entry.
fn is_bare_runtime_export_star(export: &solid_facts::ast::ExportFact) -> bool {
    !export.type_only
        && export.kind == solid_facts::ast::ExportKind::All
        && export.namespace.is_none()
}

fn explicit_runtime_export_binding_count(
    export: &solid_facts::ast::ExportFact,
    name: &str,
) -> usize {
    if export.type_only {
        return 0;
    }
    if export.kind == solid_facts::ast::ExportKind::All {
        return usize::from(export.namespace.as_deref() == Some(name));
    }
    let bindings = export
        .specifiers
        .iter()
        .chain(&export.declarations)
        .filter(|specifier| !specifier.type_only && specifier.exported == name)
        .count();
    if bindings == 0 && export.kind == solid_facts::ast::ExportKind::Default && name == "default" {
        1
    } else {
        bindings
    }
}

fn explicitly_exports_runtime_name(export: &solid_facts::ast::ExportFact, name: &str) -> bool {
    explicit_runtime_export_binding_count(export, name) > 0
}

fn reexport_entries_for_name<'a>(
    ast: &'a solid_facts::ast::AstFacts,
    name: &str,
    consult_export_stars: bool,
) -> Vec<&'a solid_facts::ast::ExportFact> {
    ast.module_level_exports()
        .filter(|export| {
            if is_bare_runtime_export_star(export) {
                consult_export_stars
            } else {
                explicitly_exports_runtime_name(export, name)
            }
        })
        .collect()
}

fn export_specifier_local_name<'a>(
    source: &'a str,
    specifier: &solid_facts::ast::ExportSpecifierFact,
    fallback: &'a str,
) -> &'a str {
    source
        .get(specifier.local.span.start as usize..specifier.local.span.end as usize)
        .unwrap_or(fallback)
}

fn is_direct_runtime_namespace_declaration(
    ast: &solid_facts::ast::AstFacts,
    declaration: &solid_facts::ast::ExportSpecifierFact,
) -> bool {
    !declaration.type_only && ast.declares_namespace_at(declaration.local.span)
}

/// Enforces the per-module explicit-before-star half of ECMA-262
/// ResolveExport and returns whether this module's bare star entries should be
/// consulted for `name`.
///
/// Candidate identity cannot enforce explicit-export uniqueness: two invalid
/// explicit entries may point at the same runtime identity and collapse in the
/// candidate map. Refuse that syntax before choosing either a local program
/// summary or an accepted re-export summary.
fn validate_module_export_precedence(
    ast: &solid_facts::ast::AstFacts,
    path: &Path,
    name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // The reviewed TypeScript class+namespace merge publishes one runtime
    // binding through two declaration statements. Do not generalize this by
    // name: illegal duplicate const/class declarations also share a spelling
    // (and may share a recovery symbol on a TypeScript-error program).
    // Specifier/default/namespace-export entries remain separate entries.
    let mut declaration_entries = Vec::new();
    let mut explicit_count = 0;
    for export in ast.module_level_exports() {
        let count = explicit_runtime_export_binding_count(export, name);
        let declarations =
            if export.kind == solid_facts::ast::ExportKind::Named && export.module.is_none() {
                export
                    .declarations
                    .iter()
                    .filter(|declaration| !declaration.type_only && declaration.exported == name)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
        explicit_count += count - declarations.len();
        declaration_entries.extend(declarations);
    }
    let reviewed_class_namespace_merge = match declaration_entries.as_slice() {
        [first, second] => {
            (ast.declares_class_at(first.local.span)
                && is_direct_runtime_namespace_declaration(ast, second))
                || (ast.declares_class_at(second.local.span)
                    && is_direct_runtime_namespace_declaration(ast, first))
        }
        _ => false,
    };
    explicit_count += if reviewed_class_namespace_merge {
        1
    } else {
        declaration_entries.len()
    };
    if explicit_count > 1 {
        return Err(format!(
            "emit package contract: module {} contains {explicit_count} explicit runtime export entries for {name:?}",
            path.display()
        )
        .into());
    }
    Ok(name != "default" && explicit_count == 0)
}

#[cfg(test)]
mod accepted_reexport_precedence_tests {
    use super::{
        explicit_runtime_export_binding_count, export_specifier_local_name,
        is_bare_runtime_export_star, reexport_entries_for_name, take_unique_reexport_candidate,
        validate_module_export_precedence,
    };
    use std::collections::BTreeMap;
    use std::path::Path;

    fn ast(source: &str) -> solid_facts::ast::AstFacts {
        solid_facts::ast::extract("/project/index.mts", source).expect("valid module")
    }

    fn consult_stars(source: &str, name: &str) -> Result<bool, Box<dyn std::error::Error>> {
        validate_module_export_precedence(&ast(source), Path::new("/project/index.mts"), name)
    }

    #[test]
    fn explicit_indirect_entry_suppresses_same_module_stars_regardless_of_order() {
        for source in [
            "export * from 'm'; export { y as x } from 'm';",
            "export { y as x } from 'm'; export * from 'm';",
        ] {
            assert!(
                !consult_stars(source, "x").expect("one explicit entry is valid"),
                "the explicit indirect entry must win in {source}"
            );
        }
    }

    #[test]
    fn distinct_star_sources_remain_eligible_for_ambiguity() {
        let facts = ast("export * from 'm1'; export * from 'm2';");
        let consult =
            validate_module_export_precedence(&facts, Path::new("/project/index.mts"), "x")
                .expect("two stars are resolved by candidate identity");
        let entries = reexport_entries_for_name(&facts, "x", consult);
        assert_eq!(entries.len(), 2, "both star sources must be traversed");

        let distinct = BTreeMap::from([("identity:m1", "first"), ("identity:m2", "second")]);
        assert_eq!(take_unique_reexport_candidate(distinct), Err(2));

        let mut same = BTreeMap::new();
        same.insert("identity:shared", "first path");
        same.insert("identity:shared", "second path");
        assert_eq!(
            take_unique_reexport_candidate(same),
            Ok(Some("second path"))
        );
    }

    #[test]
    fn type_only_entries_do_not_hide_runtime_stars_and_default_never_comes_from_a_star() {
        let source = "export * from 'm'; export { type T as x, y as z } from 'types';";
        assert!(consult_stars(source, "x").expect("type-only x is not a runtime entry"));
        assert!(!consult_stars(source, "z").expect("runtime z is an explicit entry"));
        assert!(!consult_stars(source, "default").expect("default never comes from a star"));
    }

    #[test]
    fn declarations_and_namespace_exports_are_explicit_entries() {
        assert!(
            !consult_stars("export const x = 1; export * from 'm';", "x")
                .expect("the declaration is one explicit entry")
        );
        let namespace = ast("export * from 'm'; export * as x from 'n';");
        assert!(
            !validate_module_export_precedence(&namespace, Path::new("/project/index.mts"), "x")
                .expect("the namespace is one explicit entry")
        );
        let namespace_entry = namespace
            .module_level_exports()
            .find(|export| export.namespace.as_deref() == Some("x"))
            .expect("namespace export fact");
        assert!(!is_bare_runtime_export_star(namespace_entry));
        let only_namespace = ast("export * as ns from 'm';");
        let consult_members = validate_module_export_precedence(
            &only_namespace,
            Path::new("/project/index.mts"),
            "member",
        )
        .expect("a namespace export does not publish target members");
        assert!(consult_members);
        assert!(reexport_entries_for_name(&only_namespace, "member", consult_members).is_empty());
    }

    #[test]
    fn duplicate_explicit_entries_refuse_before_candidate_identity_can_collapse_them() {
        for source in [
            "export { x } from 'm'; export { x } from 'm';",
            "export const x = 1; export { y as x } from 'm';",
            "export { a as x, b as x } from 'm';",
            "export { b as x, a as x } from 'm';",
        ] {
            let error = consult_stars(source, "x")
                .expect_err("duplicate explicit runtime exports must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("2 explicit runtime export entries")
            );
        }
    }

    #[test]
    fn typescript_declaration_merges_are_one_explicit_runtime_binding() {
        for source in [
            r#"
                export * from 'm';
                export class Merged {}
                export namespace Merged { export const marker = 1; }
            "#,
            r#"
                export * from 'm';
                export namespace Merged { export const marker = 1; }
                export class Merged {}
            "#,
        ] {
            assert!(
                !consult_stars(source, "Merged")
                    .expect("a class+namespace merge binds one runtime name")
            );
        }
    }

    #[test]
    fn illegal_duplicate_runtime_declarations_do_not_collapse_by_name() {
        for (source, count) in [
            ("export const x = 1; export const x = 2;", 2),
            ("export class x {} export class x {}", 2),
            ("export const x = 1; export namespace x {}", 2),
            (
                "export class x {} export namespace x {} export const x = 1;",
                3,
            ),
            (
                "export class x {} export function x() { namespace Hidden {} }",
                2,
            ),
            (
                "export class x {} export const [x] = [function () { namespace Hidden {} }];",
                2,
            ),
        ] {
            let error = consult_stars(source, "x")
                .expect_err("illegal duplicate declarations must fail closed");
            assert!(
                error
                    .to_string()
                    .contains(&format!("{count} explicit runtime export entries")),
                "unexpected refusal for {source}: {error}"
            );
        }
    }

    #[test]
    fn anonymous_default_is_one_explicit_binding_and_duplicate_defaults_refuse() {
        let one = ast("export default 1;");
        let default_entry = one
            .module_level_exports()
            .next()
            .expect("default export fact");
        assert_eq!(
            explicit_runtime_export_binding_count(default_entry, "default"),
            1
        );

        let duplicate = ast("export default 1; export default 2;");
        let error = validate_module_export_precedence(
            &duplicate,
            Path::new("/project/index.mts"),
            "default",
        )
        .expect_err("duplicate default bindings must fail closed");
        assert!(
            error
                .to_string()
                .contains("2 explicit runtime export entries")
        );
    }

    #[test]
    fn type_only_stars_never_contribute_runtime_candidates() {
        let only_type = ast("export type * from 'types';");
        assert!(reexport_entries_for_name(&only_type, "x", true).is_empty());

        let mixed = ast("export type * from 'types'; export * from 'runtime';");
        let entries = reexport_entries_for_name(&mixed, "x", true);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].module.as_deref(), Some("runtime"));
    }

    #[test]
    fn module_level_type_only_named_export_does_not_suppress_a_runtime_star() {
        let facts = ast("export * from 'runtime'; export type { T as x } from 'types';");
        let consult =
            validate_module_export_precedence(&facts, Path::new("/project/index.mts"), "x")
                .expect("a module-level type export is not a runtime duplicate");
        assert!(consult);
        let entries = reexport_entries_for_name(&facts, "x", consult);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].module.as_deref(), Some("runtime"));
    }

    #[test]
    fn aliased_indirect_export_resolves_the_source_name_and_excludes_its_star() {
        let source = "export * from 'm'; export { y as x } from 'm';";
        let facts = ast(source);
        let consult =
            validate_module_export_precedence(&facts, Path::new("/project/index.mts"), "x")
                .expect("one explicit alias is valid");
        let entries = reexport_entries_for_name(&facts, "x", consult);
        assert_eq!(entries.len(), 1, "the same-module star must be excluded");
        let specifier = entries[0]
            .specifiers
            .iter()
            .find(|specifier| !specifier.type_only && specifier.exported == "x")
            .expect("aliased explicit export");
        assert_eq!(export_specifier_local_name(source, specifier, "x"), "y");
    }
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
    export_entity: Option<&typefacts::EntityFact>,
    mut summary: solid_reactive_ir::ContractExport,
) -> solid_reactive_ir::ContractExport {
    let operations = export_entity
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
/// `validate_export` bars it from carrying even an open function domain — so it is
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
/// No dependency proposal is consulted here. A proposal is not accepted proof
/// for another proposal, so an external boundary remains a local open claim or
/// an entrypoint refusal until an independently proof-checked document and
/// receipt are supplied to the analyzer.
fn promote_entry_callable(
    facts: &solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
    export_entity: Option<&typefacts::EntityFact>,
    summary: solid_reactive_ir::ContractExport,
) -> Result<solid_reactive_ir::ContractExport, Box<dyn std::error::Error>> {
    let Some(entity) = export_entity else {
        return Ok(summary);
    };
    let refuse = |reason: String| -> Result<_, Box<dyn std::error::Error>> {
        Err(format!(
            "emit package contract: entry file {} exports {name:?}, {reason}; publishing kind \"value\" would certify it invokes no caller-supplied callback",
            entry_file.display()
        )
        .into())
    };
    let proof =
        solid_reactive_ir::export_kind_proof_from_entity(facts, &entity.location, Some(entity));
    match reconcile_entry_export_kind(proof, summary) {
        Ok(summary) => Ok(summary),
        Err(reason) => refuse(reason),
    }
}

fn reconcile_entry_export_kind(
    proof: solid_reactive_ir::ExportKindProof,
    summary: solid_reactive_ir::ContractExport,
) -> Result<solid_reactive_ir::ContractExport, String> {
    match proof {
        // A call signature or a construct signature; either is
        // `typeof === "function"` at runtime, and the type system reads a
        // construct signature as *not* a call signature, so a class arrives
        // here through constructability alone. The raise leaves `callbacks`
        // unknown for both: a summary still saying `value` here is one whose
        // body was never analyzed, so its silence about callbacks is not a
        // claim either. See `solid_reactive_ir::raised_function_export`.
        solid_reactive_ir::ExportKindProof::Callable if summary.kind == "value" => {
            Ok(solid_reactive_ir::raised_function_export(summary))
        }
        solid_reactive_ir::ExportKindProof::Callable => Ok(summary),
        // The exact exported specifier has closed negative callability and
        // constructability facts. That proof outranks an inferred function
        // summary reached through an initializer expression: a call such as a
        // bundler's `__exportAll({...})` has the callee's symbol inside it, but
        // exports the call result. Keeping the callee summary here certified a
        // namespace object as callable (`@kobalte/core`'s `./menubar:t`).
        solid_reactive_ir::ExportKindProof::NonCallable if summary.kind == "function" => {
            Ok(solid_reactive_ir::ContractExport {
                kind: "value".into(),
                ..solid_reactive_ir::ContractExport::default()
            })
        }
        solid_reactive_ir::ExportKindProof::NonCallable
            if summary.kind == "value" && summary.has_function_effects() =>
        {
            Err("whose closed runtime kind is non-callable, but package contract value export summary cannot have function effects".into())
        }
        solid_reactive_ir::ExportKindProof::Unresolvable(callability, constructability) => {
            Err(format!(
                "whose runtime kind no closed type answers ({callability:?}, {constructability:?})"
            ))
        }
        // Demanded and unanswered, not undemanded: `demand_plan` requests both
        // signature facts at every export specifier and every exported
        // declaration name, which are the only spans reaching this decision.
        // Absence is the producer finding no node to classify, so it is
        // missing evidence about this export rather than an answer about its
        // type — and `kind: "value"` is a claim, not a default.
        solid_reactive_ir::ExportKindProof::Unanswered => {
            Err("whose runtime kind no fact covers at all".into())
        }
        solid_reactive_ir::ExportKindProof::NonCallable => Ok(summary),
    }
}

#[cfg(test)]
mod entry_export_kind_reconciliation_tests {
    use super::reconcile_entry_export_kind;
    use solid_reactive_ir::{ContractClaim, ContractExport, ExportKindProof};

    // A closed non-callable proof carrying function domains is a contradiction
    // between two facts, not a cleanup opportunity: the one measured instance
    // (@solid-primitives/geolocation, where the runtime-kind join is wrong and
    // the callback claim is right) shows the conflicting claim can be the true
    // one, so discarding it would publish a false certification. The refusal
    // invariant is pinned by the reconciliation-pass comment above
    // reconcile_entry_export_kind's second caller and by the phase 21 plan's
    // slice 1 ("keep the existing invariant that a closed non-callable proof
    // carrying function domains is refused").
    #[test]
    fn non_callable_value_with_function_effects_is_refused() {
        let summary = ContractExport {
            kind: "value".into(),
            callbacks: ContractClaim::Open,
            ..ContractExport::default()
        };
        let refusal = reconcile_entry_export_kind(ExportKindProof::NonCallable, summary)
            .expect_err(
                "a closed non-callable proof conflicting with invocation claims must refuse",
            );
        assert!(
            refusal.contains("cannot have function effects"),
            "{refusal}"
        );
    }

    #[test]
    fn non_callable_function_summary_demotes_to_a_closed_value() {
        let summary = ContractExport {
            kind: "function".into(),
            callbacks: ContractClaim::Open,
            ..ContractExport::default()
        };
        let reconciled =
            reconcile_entry_export_kind(ExportKindProof::NonCallable, summary).unwrap();
        assert_eq!(reconciled.kind, "value");
        assert!(!reconciled.has_function_effects());
    }
}

fn entry_export_entity<'a>(
    facts: &'a solid_facts::ProjectFacts,
    entry_file: &Path,
    name: &str,
) -> Option<&'a typefacts::EntityFact> {
    entry_export_entity_with_visiting(facts, entry_file, name, &mut HashSet::new())
}

fn entry_export_entity_indexed<'a>(
    facts: &'a solid_facts::ProjectFacts,
    files_by_canonical_path: &HashMap<PathBuf, &'a solid_facts::FileFacts>,
    entities_by_location: &HashMap<typefacts::Location, &'a typefacts::EntityFact>,
    entry_file: &Path,
    name: &str,
    visiting: &mut HashSet<(PathBuf, String)>,
) -> Option<&'a typefacts::EntityFact> {
    let entry_file = entry_file.canonicalize().ok()?;
    if !visiting.insert((entry_file.clone(), name.to_owned())) {
        return None;
    }
    let file = files_by_canonical_path.get(&entry_file).copied()?;
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
            if let Some(entity) = entities_by_location.get(&location) {
                return Some(*entity);
            }
            if let Some(module) = export.module.as_deref()
                && module.starts_with('.')
            {
                let target = resolve_relative_export(facts, &entry_file, module).ok()?;
                let local_name = file.source_text(specifier.local.span).unwrap_or(name);
                if let Some(entity) = entry_export_entity_indexed(
                    facts,
                    files_by_canonical_path,
                    entities_by_location,
                    &target,
                    local_name,
                    visiting,
                ) {
                    return Some(entity);
                }
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && module.starts_with('.')
        {
            let target = resolve_relative_export(facts, &entry_file, module).ok()?;
            if let Some(entity) = entry_export_entity_indexed(
                facts,
                files_by_canonical_path,
                entities_by_location,
                &target,
                name,
                visiting,
            ) {
                return Some(entity);
            }
        }
    }
    None
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
    entry_entities_by_name: &HashMap<String, &typefacts::EntityFact>,
    exports: &mut BTreeMap<String, solid_reactive_ir::ContractExport>,
) {
    let mut names_by_identity = BTreeMap::<String, Vec<String>>::new();
    for name in exports.keys() {
        let Some(identity) = entry_entities_by_name
            .get(name)
            .copied()
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
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.reactive_reads = solid_reactive_ir::ContractClaim::Open;
                }
                (solid_reactive_ir::ContractClaim::Open, _) => {}
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
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.callbacks = solid_reactive_ir::ContractClaim::Open;
                }
                (solid_reactive_ir::ContractClaim::Open, _) => {}
            }
            match (&mut merged.returns, &summary.returns) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_return),
                    solid_reactive_ir::ContractClaim::Known(returned),
                ) if merged_return.is_none() => *merged_return = returned.clone(),
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.returns = solid_reactive_ir::ContractClaim::Open;
                }
                _ => {}
            }
            match (&mut merged.async_behavior, &summary.async_behavior) {
                (
                    solid_reactive_ir::ContractClaim::Known(merged_behavior),
                    solid_reactive_ir::ContractClaim::Known(behavior),
                ) if merged_behavior.is_empty() => *merged_behavior = behavior.clone(),
                (_, solid_reactive_ir::ContractClaim::Open) => {
                    merged.async_behavior = solid_reactive_ir::ContractClaim::Open;
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

/// The machine-readable half of a missing-dependency refusal.
///
/// Contract generation is fail-closed across package boundaries. When this
/// entrypoint re-exports a package with no exact accepted contract, this stable
/// stderr record identifies the unresolved module for the refusal report.
///
/// Parsing a specifier back out of human prose would couple automation to
/// wording, so the boundary emits one stable line in addition to the message.
const UNRESOLVED_DEPENDENCY_MODULE_MARKER: &str = "solid-checker:unresolved-dependency-module=";

/// Refuse emission at a package boundary this process cannot cross, naming the
/// dependency both machine-readably and in prose. A module specifier never
/// contains a newline, so the marker is exactly one line.
///
#[derive(Debug)]
struct UnresolvedDependencyModuleError {
    module: String,
    from: PathBuf,
}

impl std::fmt::Display for UnresolvedDependencyModuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "emit package contract: cannot statically expand external export-all {:?} from {}; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts",
            self.module,
            self.from.display()
        )
    }
}

impl std::error::Error for UnresolvedDependencyModuleError {}

fn render_program_error_with_program(
    error: &(dyn std::error::Error + 'static),
    program: &str,
) -> String {
    let marker = error
        .downcast_ref::<UnresolvedDependencyModuleError>()
        .map(|error| format!("{UNRESOLVED_DEPENDENCY_MODULE_MARKER}{}\n", error.module))
        .unwrap_or_default();
    format!("{marker}{program}: {error}")
}

fn render_program_error(error: &(dyn std::error::Error + 'static)) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "solid-facts-backend".into());
    render_program_error_with_program(error, &program)
}

/// Keep the missing module structured until the process or batch target
/// actually refuses. A recovered batch-target error must not leak a global
/// stderr marker that cannot be associated with that target.
fn refuse_unresolved_dependency_module<T>(
    module: &str,
    from: &Path,
) -> Result<T, Box<dyn std::error::Error>> {
    Err(UnresolvedDependencyModuleError {
        module: module.into(),
        from: from.into(),
    }
    .into())
}

#[cfg(test)]
mod certification_source_request_tests {
    use super::*;

    fn planning(source_dependencies: Vec<ContractCertificationSourceRequest>) -> String {
        serde_json::json!({
            "schemaVersion": 1,
            "proposal": "/does/not/exist/proposal.json",
            "resolution": {
                "specifier": "root-package",
                "importer": "/project/src/app.ts",
                "requestedEntrypoint": ".",
                "packageName": "root-package",
                "packageVersion": "1.0.0",
                "packageIntegrity": "sha512-AA==",
                "packageRoot": "/project/node_modules/root-package",
                "packageManifest": { "path": "/p/package.json", "digest": "sha256:00" },
                "runtime": { "path": "/p/dist/index.js", "digest": "sha256:00" },
                "declarations": { "path": "/p/types/index.d.ts", "digest": "sha256:00" },
                "closure": { "digest": "sha256:00", "entries": [], "dependencies": [], "hazards": [] },
                "authority": "host"
            },
            "exportConditions": ["import"],
            "registryOrigin": "https://registry.npmjs.org",
            "registryMetadata": "/does/not/exist/metadata.json",
            "archive": "/does/not/exist/package.tgz",
            "sourceDependencies": source_dependencies
                .into_iter()
                .map(|source| serde_json::json!({
                    "packageName": source.package_name,
                    "packageVersion": source.package_version,
                    "registryOrigin": source.registry_origin,
                    "registryMetadata": source.registry_metadata,
                    "archive": source.archive,
                    "lockfile": source.lockfile,
                    "lockLocator": source.lock_locator,
                    "installedPackageRoot": source.installed_package_root,
                }))
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn source() -> ContractCertificationSourceRequest {
        serde_json::from_str(
            r#"{
                "packageName": "source-types",
                "packageVersion": "3.0.0",
                "registryOrigin": "https://registry.npmjs.org",
                "registryMetadata": "/does/not/exist/metadata.json",
                "archive": "/does/not/exist/source.tgz",
                "lockfile": "/does/not/exist/bun.lock",
                "lockLocator": "source-types@3.0.0",
                "installedPackageRoot": "/project/node_modules/source-types"
            }"#,
        )
        .unwrap()
    }

    /// A graph node names its declaration-only closure once, on the node. The
    /// nested planning's copy would be silently ignored, so two disagreeing
    /// declarations of one authenticated closure must be refused outright
    /// rather than half-applied.
    #[test]
    fn a_graph_node_refuses_a_planning_that_carries_its_own_source_set() {
        let node: ContractCertificationGraphNodeRequest =
            serde_json::from_value(serde_json::json!({
                "planning": serde_json::from_str::<serde_json::Value>(&planning(vec![source()]))
                    .unwrap(),
                "lockfile": "/does/not/exist/bun.lock",
                "lockLocator": "root-package@1.0.0",
                "sourceDependencies": [],
            }))
            .unwrap();
        let Err(error) = certification_graph_node_from_request(node) else {
            panic!("a nested source set must be refused");
        };
        assert_eq!(
            error.to_string(),
            "a graph node's planning must not carry its own declaration-only source set"
        );
    }

    /// The same node without the nested set gets past the refusal and fails on
    /// its (deliberately absent) proposal bytes instead, which proves the check
    /// above is the sources and not the rest of the request.
    #[test]
    fn a_graph_node_without_a_nested_source_set_reaches_its_artifact_bytes() {
        let node: ContractCertificationGraphNodeRequest =
            serde_json::from_value(serde_json::json!({
                "planning": serde_json::from_str::<serde_json::Value>(&planning(Vec::new()))
                    .unwrap(),
                "lockfile": "/does/not/exist/bun.lock",
                "lockLocator": "root-package@1.0.0",
                "sourceDependencies": [],
            }))
            .unwrap();
        let Err(error) = certification_graph_node_from_request(node) else {
            panic!("this request has no artifact bytes and cannot plan");
        };
        let error = error.to_string();
        assert!(
            !error.contains("declaration-only source set"),
            "unexpected refusal: {error}"
        );
    }
}

#[cfg(test)]
mod unresolved_dependency_refusal_tests {
    use super::*;

    #[test]
    fn batch_refusal_retains_machine_marker_and_native_prefix() {
        let error = refuse_unresolved_dependency_module::<()>(
            "bundled-dependency",
            Path::new("/package/index.ts"),
        )
        .unwrap_err();
        assert_eq!(
            render_program_error_with_program(error.as_ref(), "solid-checker-rust"),
            "solid-checker:unresolved-dependency-module=bundled-dependency\nsolid-checker-rust: emit package contract: cannot statically expand external export-all \"bundled-dependency\" from /package/index.ts; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts"
        );
    }
}

#[cfg(test)]
mod contract_emission_fact_program_tests {
    use super::*;

    fn context() -> ContractEmissionFactContext {
        ContractEmissionFactContext {
            dialect: "solid-v1".into(),
            typefacts_project: "/project/tsconfig.json".into(),
            typefacts_executable: "/bin/solid-typefacts".into(),
            typefacts_arguments: vec!["--protocol=3".into()],
            generation: 7,
            semantic_demands: SemanticDemandOptions {
                array_map_receiver_types: false,
                contract_probe_parameters: true,
            },
            runtime: RuntimeEnvironment::default(),
            presets: vec!["recommended".into()],
            enabled_rules: vec!["solid/reactivity".into()],
        }
    }

    fn source(path: &str, text: &str) -> CanonicalContractEmissionSource {
        CanonicalContractEmissionSource {
            canonical_path: PathBuf::from(path),
            source: SourceFile {
                path: path.into(),
                source: std::sync::Arc::from(text),
                compiler_options: solid_facts::compiler::CompilerOptions::default(),
            },
        }
    }

    #[test]
    fn fact_program_key_binds_canonical_sources_bytes_options_and_context() {
        let key = ContractEmissionFactProgramKey {
            context: context(),
            sources: vec![source("/project/a.ts", "export const a = 1;\n")],
        };
        assert_eq!(key, key.clone());

        let mut changed = key.clone();
        changed.sources[0].canonical_path = PathBuf::from("/project/alias.ts");
        assert_ne!(key, changed);

        let mut changed = key.clone();
        changed.sources[0].source.source = std::sync::Arc::from("export const a = 2;\n");
        assert_ne!(key, changed);

        let mut changed = key.clone();
        changed.sources[0].source.compiler_options.dev = true;
        assert_ne!(key, changed);

        let mut changed = key.clone();
        changed.context.generation += 1;
        assert_ne!(key, changed);

        let mut changed = key.clone();
        changed.context.runtime.conditions.insert("browser".into());
        assert_ne!(key, changed);

        let mut changed = key.clone();
        changed.context.typefacts_arguments.push("--strict".into());
        assert_ne!(key, changed);
    }
}

fn exported_names_for_file(
    facts: &solid_facts::ProjectFacts,
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
    contracts: &solid_reactive_ir::contract_semantics::AcceptedContractIndex,
    path: &Path,
    visiting: &mut HashSet<PathBuf>,
) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    if !visiting.insert(path.clone()) {
        return Ok(BTreeSet::new());
    }
    let file = files_by_canonical_path.get(&path).copied().ok_or_else(|| {
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
                let Ok(external_names) = contracts.public_export_names(file.path.as_str(), module)
                else {
                    return refuse_unresolved_dependency_module(module, &path);
                };
                // ESM `export *` never forwards the dependency's default
                // export. The accepted contract surface contains runtime
                // names only, so no additional type-space filtering is
                // needed at this boundary.
                names.extend(external_names.into_iter().filter(|name| name != "default"));
                continue;
            }
            let target = resolve_relative_export(facts, &path, module)?;
            names.extend(exported_names_for_file(
                facts,
                files_by_canonical_path,
                contracts,
                &target,
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
            if export_is_type_only(
                facts,
                files_by_canonical_path,
                &path,
                &name,
                &mut HashSet::new(),
            ) {
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
/// Walking the relative re-export and import chain, a name whose every export
/// is marked `type_only` somewhere along it binds nothing at runtime, so
/// omitting it is exactly right — and exactly what the marked spelling already
/// gets.
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
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
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
    let Some(file) = files_by_canonical_path.get(&path).copied() else {
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
                        export_is_type_only(
                            facts,
                            files_by_canonical_path,
                            &target,
                            local_name,
                            visiting,
                        )
                    })
                }
                // A bare specifier leaves this project; the dependency's own
                // contract describes its runtime exports and says nothing
                // about its type-only ones, so nothing here is proof.
                Some(_) => false,
                None => local_import_is_type_only(
                    facts,
                    files_by_canonical_path,
                    file,
                    &path,
                    local_name,
                    visiting,
                ),
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
    files_by_canonical_path: &HashMap<PathBuf, &solid_facts::FileFacts>,
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
            return resolve_relative_export(facts, path, &import.module).is_ok_and(|target| {
                export_is_type_only(facts, files_by_canonical_path, &target, imported, visiting)
            });
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
