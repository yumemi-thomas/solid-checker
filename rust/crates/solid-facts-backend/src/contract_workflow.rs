//! Temporary-v2 proposal planning and review documents.
//!
//! Node owns package acquisition and process lifecycle. This module owns every
//! semantic read or write needed by generation, plan merging, review, and
//! certification scheduling so JavaScript never becomes a second normalizer.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use solid_reactive_ir::contract_semantics::{
    NormalizedContract, SemanticClaimPath, SemanticClaimSubject,
};
use thiserror::Error;

use crate::{
    contract_document::{self, SidecarDigests},
    contract_interface::ContractFailure,
    evidence_sidecars::WireSemanticClaimSubject,
};

const PLAN_FORMAT: &str = "solid-checker-contract-proposal-plan";
const PLAN_VERSION: u16 = 1;
const MAX_WORKFLOW_BYTES: usize = 16 * 1024 * 1024;
const MAX_CLAIMS: usize = 65_536;
const MAX_WORKFLOW_DEPTH: usize = 128;
const MAX_WORKFLOW_NODES: usize = 1_000_000;
const MAX_WORKFLOW_STRING_BYTES: usize = 16 * 1024;

#[derive(Debug, Error)]
pub enum ContractWorkflowError {
    #[error(transparent)]
    Contract(#[from] ContractFailure),
    #[error("contract workflow document cannot be decoded: {message}")]
    Decode { message: String },
    #[error("contract workflow document is invalid: {reason}")]
    Invalid { reason: String },
}

pub struct ProposalArtifacts {
    pub document: Vec<u8>,
    pub plan: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCorpusMeasurements {
    pub proposal_bytes: usize,
    pub plan_bytes: usize,
    pub proof_bytes: usize,
    pub generation_ns: u128,
    pub verification_ns: u128,
}

pub(crate) struct CanonicalProposal {
    pub contract: NormalizedContract,
    pub document: Vec<u8>,
    pub closure_candidates: Vec<SemanticClaimSubject>,
}

pub(crate) fn canonicalize_proposal(
    contract: &NormalizedContract,
    closure_candidates: impl IntoIterator<Item = SemanticClaimSubject>,
    pretty: bool,
) -> Result<CanonicalProposal, ContractWorkflowError> {
    let document = contract_document::encode(contract, &SidecarDigests::default(), pretty)?;
    let canonical = contract_document::decode(&document)?.normalize()?;
    let closure_candidates = closure_candidates
        .into_iter()
        .map(|subject| rebind_subject(contract, &canonical, subject))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    Ok(CanonicalProposal {
        contract: canonical,
        document,
        closure_candidates,
    })
}

pub(crate) fn encode_proposal_artifacts(
    contract: &NormalizedContract,
    closure_candidates: impl IntoIterator<Item = SemanticClaimSubject>,
    pretty: bool,
) -> Result<ProposalArtifacts, ContractWorkflowError> {
    let canonical = canonicalize_proposal(contract, closure_candidates, pretty)?;
    Ok(ProposalArtifacts {
        plan: encode_plan(&canonical.contract, canonical.closure_candidates)?,
        document: canonical.document,
    })
}

fn rebind_subject(
    original: &NormalizedContract,
    canonical: &NormalizedContract,
    mut subject: SemanticClaimSubject,
) -> Result<Option<SemanticClaimSubject>, ContractWorkflowError> {
    let original_case = original
        .artifact_case(&subject.artifact_case)
        .ok_or_else(|| invalid_error("proposal candidate names an unknown artifact case"))?;
    let canonical_case = canonical
        .artifact_cases()
        .iter()
        .find(|artifact| artifact_key(artifact) == artifact_key(original_case))
        .ok_or_else(|| invalid_error("canonical proposal lost a closure candidate artifact"))?;
    let original_export = original_case
        .exports
        .get(&subject.export)
        .ok_or_else(|| invalid_error("proposal candidate names an unknown export"))?;
    let canonical_export = canonical_case
        .exports
        .get(&subject.export)
        .ok_or_else(|| invalid_error("canonical proposal lost a closure candidate export"))?;
    let operations = original_export
        .call
        .operations
        .iter()
        .zip(&canonical_export.call.operations)
        .map(|(left, right)| (left.id.clone(), right.id.clone()))
        .collect::<BTreeMap<_, _>>();
    let resources = original_export
        .call
        .resources
        .iter()
        .zip(&canonical_export.call.resources)
        .map(|(left, right)| (left.id.clone(), right.id.clone()))
        .collect::<BTreeMap<_, _>>();
    if operations.len() != original_export.call.operations.len()
        || operations.len() != canonical_export.call.operations.len()
        || resources.len() != original_export.call.resources.len()
        || resources.len() != canonical_export.call.resources.len()
    {
        return Ok(None);
    }
    if !rebind_path(&mut subject.path, &operations, &resources) {
        return Ok(None);
    }
    subject.artifact_case = canonical_case.id.clone();
    if canonical.claim_id(&subject).is_err() {
        // Normalization can legitimately elide a proposed local closure when
        // its owning operation/resource disappears. An absent subject is not
        // a proof obligation and must not be rebound to a neighboring fact.
        return Ok(None);
    }
    Ok(Some(subject))
}

fn rebind_path(
    path: &mut SemanticClaimPath,
    operations: &BTreeMap<
        solid_reactive_ir::contract_semantics::OperationId,
        solid_reactive_ir::contract_semantics::OperationId,
    >,
    resources: &BTreeMap<
        solid_reactive_ir::contract_semantics::ResourceId,
        solid_reactive_ir::contract_semantics::ResourceId,
    >,
) -> bool {
    use solid_reactive_ir::contract_semantics::{ClaimPath, ValueRoot};
    let operation = |id: &mut solid_reactive_ir::contract_semantics::OperationId| {
        let Some(rebound) = operations.get(id).cloned() else {
            return false;
        };
        *id = rebound;
        true
    };
    match path {
        SemanticClaimPath::Domain(ClaimPath::Value { root, .. }) => match root {
            ValueRoot::Export => {}
            ValueRoot::OperationInput { operation: id, .. }
            | ValueRoot::OperationOutput { operation: id } => {
                if !operation(id) {
                    return false;
                }
            }
        },
        SemanticClaimPath::Domain(ClaimPath::Operation { operation: id, .. })
        | SemanticClaimPath::Operation(id) => {
            if !operation(id) {
                return false;
            }
        }
        SemanticClaimPath::Domain(ClaimPath::Resource { resource, .. }) => {
            let Some(rebound) = resources.get(resource).cloned() else {
                return false;
            };
            *resource = rebound;
        }
        SemanticClaimPath::Domain(ClaimPath::Call(_) | ClaimPath::GuardPartition) => {}
    }
    true
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanDocument {
    format: String,
    plan_version: u16,
    semantic_model_version: u16,
    semantic_digest: String,
    closure_candidates: Vec<PlanClaim>,
    unresolved_claims: Vec<PlanClaim>,
    positive_operations: Vec<PlanClaim>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanClaim {
    claim_id: String,
    artifact: PlanArtifact,
    subject: WireSemanticClaimSubject,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanArtifact {
    entrypoint: String,
    runtime_path: String,
    runtime_digest: String,
    declarations_path: String,
    declarations_digest: String,
    closure_digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewDocument {
    format: &'static str,
    schema_version: u16,
    semantic_model_version: u16,
    semantic_digest: String,
    package: ReviewPackage,
    artifact_cases: Vec<ReviewArtifactCase>,
    unresolved_claims: Vec<PlanClaim>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewPackage {
    name: String,
    version: String,
    integrity: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewArtifactCase {
    id: String,
    entrypoint: String,
    exports: Vec<String>,
}

pub(crate) struct PlannedProbeSubjects {
    pub possible_operations: BTreeMap<String, SemanticClaimSubject>,
    pub closure_candidates: BTreeMap<String, SemanticClaimSubject>,
}

/// Expands only the claim identities the runtime-probe boundary may target.
/// The compact plan remains private to this module; callers receive normalized
/// semantic subjects after contract/digest/claim-ID validation.
pub(crate) fn planned_probe_subjects(
    contract: &NormalizedContract,
    plan_bytes: &[u8],
) -> Result<PlannedProbeSubjects, ContractWorkflowError> {
    let plan = decode_plan(plan_bytes)?;
    validate_identity(
        contract,
        plan.semantic_model_version,
        &plan.semantic_digest,
        "proposal plan",
    )?;
    Ok(PlannedProbeSubjects {
        possible_operations: validated_plan_claims(contract, plan.positive_operations)?,
        closure_candidates: validated_plan_claims(contract, plan.closure_candidates)?,
    })
}

fn validated_plan_claims(
    contract: &NormalizedContract,
    claims: Vec<PlanClaim>,
) -> Result<BTreeMap<String, SemanticClaimSubject>, ContractWorkflowError> {
    let mut result = BTreeMap::new();
    for claim in claims {
        let subject = SemanticClaimSubject::try_from(claim.subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        let actual = contract
            .claim_id(&subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        if actual.as_str() != claim.claim_id {
            return invalid("proposal plan claim ID does not match its subject");
        }
        if artifact_key(
            contract
                .artifact_case(&subject.artifact_case)
                .expect("claim validation retained the artifact case"),
        ) != claim.artifact
        {
            return invalid("proposal plan claim names stale artifact identity");
        }
        if result.insert(claim.claim_id.clone(), subject).is_some() {
            return invalid(format!("proposal plan repeats claim {}", claim.claim_id));
        }
    }
    Ok(result)
}

pub fn encode_plan(
    contract: &NormalizedContract,
    closure_candidates: impl IntoIterator<Item = SemanticClaimSubject>,
) -> Result<Vec<u8>, ContractWorkflowError> {
    let closure_candidates = claims(contract, closure_candidates)?;
    let closure_ids = closure_candidates
        .iter()
        .map(|claim| claim.claim_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut unresolved = Vec::new();
    let mut operations = Vec::new();
    for artifact in contract.artifact_cases() {
        for (export_name, export) in &artifact.exports {
            unresolved.extend(export.unresolved_claims().into_iter().map(|path| {
                SemanticClaimSubject {
                    artifact_case: artifact.id.clone(),
                    export: export_name.clone(),
                    path: SemanticClaimPath::Domain(path),
                }
            }));
            operations.extend(export.call.operations.iter().map(|operation| {
                SemanticClaimSubject {
                    artifact_case: artifact.id.clone(),
                    export: export_name.clone(),
                    path: SemanticClaimPath::Operation(operation.id.clone()),
                }
            }));
        }
    }
    let mut unresolved_claims = claims(contract, unresolved)?;
    unresolved_claims.retain(|claim| !closure_ids.contains(claim.claim_id.as_str()));
    emit(&PlanDocument {
        format: PLAN_FORMAT.into(),
        plan_version: PLAN_VERSION,
        semantic_model_version: contract.semantic_model_version(),
        semantic_digest: contract.semantic_digest().as_str().into(),
        closure_candidates,
        unresolved_claims,
        positive_operations: claims(contract, operations)?,
    })
}

pub fn merge_plans(
    merged_document: &[u8],
    proposal_artifacts: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
) -> Result<Vec<u8>, ContractWorkflowError> {
    let contract = contract_document::decode(merged_document)?.normalize()?;
    let mut candidates = BTreeSet::new();
    for (document, bytes) in proposal_artifacts {
        let source = contract_document::decode(&document)?.normalize()?;
        let plan = decode_plan(&bytes)?;
        validate_identity(
            &source,
            plan.semantic_model_version,
            &plan.semantic_digest,
            "source proposal plan",
        )?;
        for subject in validated_plan_claims(&source, plan.closure_candidates)?.into_values() {
            if let Some(rebound) = rebind_subject(&source, &contract, subject)? {
                candidates.insert(rebound);
            }
        }
    }
    encode_plan(&contract, candidates)
}

pub fn review(document: &[u8]) -> Result<Vec<u8>, ContractWorkflowError> {
    let contract = contract_document::decode(document)?.normalize()?;
    let mut unresolved = Vec::new();
    for artifact in contract.artifact_cases() {
        for (export_name, export) in &artifact.exports {
            unresolved.extend(export.unresolved_claims().into_iter().map(|path| {
                SemanticClaimSubject {
                    artifact_case: artifact.id.clone(),
                    export: export_name.clone(),
                    path: SemanticClaimPath::Domain(path),
                }
            }));
        }
    }
    emit(&ReviewDocument {
        format: "solid-checker-contract-review",
        schema_version: 2,
        semantic_model_version: contract.semantic_model_version(),
        semantic_digest: contract.semantic_digest().as_str().into(),
        package: ReviewPackage {
            name: contract.package().name.clone(),
            version: contract.package().version.clone(),
            integrity: contract.package().integrity.clone(),
        },
        artifact_cases: contract
            .artifact_cases()
            .iter()
            .map(|artifact| ReviewArtifactCase {
                id: artifact.id.clone(),
                entrypoint: artifact.entrypoint.clone(),
                exports: artifact.exports.keys().cloned().collect(),
            })
            .collect(),
        unresolved_claims: claims(&contract, unresolved)?,
    })
}

fn claims(
    contract: &NormalizedContract,
    subjects: impl IntoIterator<Item = SemanticClaimSubject>,
) -> Result<Vec<PlanClaim>, ContractWorkflowError> {
    let mut unique = BTreeSet::new();
    for subject in subjects {
        let claim_id = contract
            .claim_id(&subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        unique.insert((claim_id.as_str().to_owned(), subject));
    }
    if unique.len() > MAX_CLAIMS {
        return invalid("contract workflow claim count exceeds the resource limit");
    }
    Ok(unique
        .into_iter()
        .map(|(claim_id, subject)| PlanClaim {
            claim_id,
            artifact: artifact_key(
                contract
                    .artifact_case(&subject.artifact_case)
                    .expect("claim identity validated artifact case"),
            ),
            subject: WireSemanticClaimSubject::from(&subject),
        })
        .collect())
}

fn artifact_key(artifact: &solid_reactive_ir::contract_semantics::ArtifactCase) -> PlanArtifact {
    PlanArtifact {
        entrypoint: artifact.entrypoint.clone(),
        runtime_path: artifact.runtime.path.clone(),
        runtime_digest: artifact.runtime.digest.as_str().into(),
        declarations_path: artifact.declarations.path.clone(),
        declarations_digest: artifact.declarations.digest.as_str().into(),
        closure_digest: artifact.dependency_closure.as_str().into(),
    }
}

fn decode_plan(bytes: &[u8]) -> Result<PlanDocument, ContractWorkflowError> {
    let plan: PlanDocument = decode(bytes)?;
    if plan.format != PLAN_FORMAT || plan.plan_version != PLAN_VERSION {
        return invalid(format!(
            "proposal plan must use format {PLAN_FORMAT:?} version {PLAN_VERSION}"
        ));
    }
    if plan.closure_candidates.len() > MAX_CLAIMS
        || plan.unresolved_claims.len() > MAX_CLAIMS
        || plan.positive_operations.len() > MAX_CLAIMS
    {
        return invalid("proposal plan claim count exceeds the resource limit");
    }
    Ok(plan)
}

fn validate_identity(
    contract: &NormalizedContract,
    semantic_model_version: u16,
    semantic_digest: &str,
    label: &str,
) -> Result<(), ContractWorkflowError> {
    if semantic_model_version != contract.semantic_model_version()
        || semantic_digest != contract.semantic_digest().as_str()
    {
        invalid(format!(
            "{label} does not bind the exact proposal semantics"
        ))
    } else {
        Ok(())
    }
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ContractWorkflowError> {
    crate::bounded_json::decode(
        bytes,
        crate::bounded_json::Limits {
            bytes: MAX_WORKFLOW_BYTES,
            depth: MAX_WORKFLOW_DEPTH,
            nodes: MAX_WORKFLOW_NODES,
            string_bytes: MAX_WORKFLOW_STRING_BYTES,
        },
    )
    .map_err(decode_error)
}

fn emit(value: &impl Serialize) -> Result<Vec<u8>, ContractWorkflowError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(decode_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn decode_error(error: impl std::fmt::Display) -> ContractWorkflowError {
    ContractWorkflowError::Decode {
        message: error.to_string(),
    }
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, ContractWorkflowError> {
    Err(invalid_error(reason))
}

fn invalid_error(reason: impl Into<String>) -> ContractWorkflowError {
    ContractWorkflowError::Invalid {
        reason: reason.into(),
    }
}
