//! Temporary-v2 proposal, proof, and receipt workflow documents.
//!
//! Node owns package acquisition and process lifecycle. This module owns every
//! semantic read or write needed by generation, plan merging, review, and
//! proof verification so JavaScript never becomes a second normalizer.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    Digest, NormalizedContract, SemanticClaimPath, SemanticClaimSubject, VerifierIdentity,
    proof::{
        CLOSURE_PROOF_FAMILIES, CensusCompleteness, ClosureVerificationRequest,
        PROOF_POLICY_VERSION, ProofContradiction, ProofFamily, ProofRuleInput, family_authority,
        proof_scope_digest, replay_proof_rule, verify_closure,
    },
};
use thiserror::Error;

use crate::{
    contract_document_v2::{self, SidecarDigests},
    contract_interface::{ContractFailure, encode_acceptance_receipt},
    evidence_sidecars::WireSemanticClaimSubject,
};

const PLAN_FORMAT: &str = "solid-checker-contract-proposal-plan";
const PLAN_VERSION: u16 = 1;
const PROOF_FORMAT: &str = "solid-checker-contract-proof-transcript";
const PROOF_VERSION: u16 = 1;
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
    #[error("contract proof verification failed: {message}")]
    Proof { message: String },
}

pub struct ProposalArtifacts {
    pub document: Vec<u8>,
    pub plan: Vec<u8>,
}

pub struct AcceptedArtifacts {
    pub document: Vec<u8>,
    pub receipt: Vec<u8>,
}

pub(crate) struct CheckedCorpusAcceptance {
    pub accepted: AcceptedArtifacts,
    pub measurements: CheckedCorpusMeasurements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCorpusMeasurements {
    pub proposal_bytes: usize,
    pub plan_bytes: usize,
    pub proof_bytes: usize,
    pub generation_ns: u128,
    pub verification_ns: u128,
}

/// Runs the ordinary proof checker for one repository-owned, independently
/// replayed semantic corpus. The caller supplies the checked corpus bytes as
/// the census identity; arbitrary CLI input cannot reach this helper.
pub(crate) fn accept_checked_corpus_case(
    complete: &NormalizedContract,
    verifier_build: &str,
    checked_corpus_bytes: &[u8],
    pretty: bool,
) -> Result<CheckedCorpusAcceptance, ContractWorkflowError> {
    let generation_started = Instant::now();
    let canonical_complete_bytes =
        contract_document_v2::encode(complete, &SidecarDigests::default(), false)?;
    let canonical_complete =
        contract_document_v2::decode(&canonical_complete_bytes)?.normalize()?;
    if canonical_complete.artifact_cases().len() != 1 {
        return invalid("checked corpus acceptance requires exactly one artifact case");
    }
    let package = canonical_complete.package().clone();
    let mut cases = canonical_complete.artifact_cases().to_vec();
    let mut candidates = Vec::new();
    for artifact in &mut cases {
        for (export_name, export) in &mut artifact.exports {
            candidates.extend(export.open_proposed_closure().into_iter().map(|path| {
                SemanticClaimSubject {
                    artifact_case: artifact.id.clone(),
                    export: export_name.clone(),
                    path: SemanticClaimPath::Domain(path),
                }
            }));
        }
    }
    if candidates.is_empty() {
        return invalid("checked corpus case has no locally closable semantic claim");
    }
    let open = solid_reactive_ir::contract_semantics::ContractProposal::new(package, cases)
        .normalize()
        .map_err(|error| invalid_error(error.to_string()))?;
    let proposal = canonicalize_proposal(&open, candidates, pretty)?;
    let plan_bytes = encode_plan(&proposal.contract, proposal.closure_candidates)?;
    let plan = decode_plan(&plan_bytes)?;
    let census = format!("sha256:{:x}", Sha256::digest(checked_corpus_bytes));
    let claims = plan
        .closure_candidates
        .into_iter()
        .map(|claim| {
            let claim_id = claim.claim_id;
            let families = CLOSURE_PROOF_FAMILIES
                .into_iter()
                .map(|family| ProofFamilyInput {
                    family: proof_family_name(family).into(),
                    transcript: format!("checked-corpus:{census}:{claim_id}:{family:?}"),
                    enumerated: vec![census.clone()],
                    classified: vec![census.clone()],
                    unresolved: Vec::new(),
                    complete: true,
                })
                .collect();
            ProofClaim {
                claim_id,
                subject: claim.subject,
                families,
            }
        })
        .collect();
    let proof = emit(&ProofDocument {
        format: PROOF_FORMAT.into(),
        proof_version: PROOF_VERSION,
        semantic_model_version: proposal.contract.semantic_model_version(),
        semantic_digest: proposal.contract.semantic_digest().as_str().into(),
        verifier_build: verifier_build.into(),
        claims,
        probe_contradictions: Vec::new(),
        probe_sidecar: None,
    })?;
    let selected = proposal.contract.artifact_cases()[0].id.clone();
    let generation_ns = generation_started.elapsed().as_nanos();
    let verification_started = Instant::now();
    let accepted = verify(&proposal.document, &plan_bytes, &proof, &selected, pretty)?;
    let finalized = contract_document_v2::decode(&accepted.document)?.normalize()?;
    if finalized.semantic_digest() != canonical_complete.semantic_digest() {
        return invalid("proof finalization did not reproduce checked corpus semantics");
    }
    let verification_ns = verification_started.elapsed().as_nanos();
    Ok(CheckedCorpusAcceptance {
        measurements: CheckedCorpusMeasurements {
            proposal_bytes: proposal.document.len(),
            plan_bytes: plan_bytes.len(),
            proof_bytes: proof.len(),
            generation_ns,
            verification_ns,
        },
        accepted,
    })
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
    let document = contract_document_v2::encode(contract, &SidecarDigests::default(), pretty)?;
    let canonical = contract_document_v2::decode(&document)?.normalize()?;
    let closure_candidates = closure_candidates
        .into_iter()
        .map(|subject| rebind_subject(contract, &canonical, subject))
        .collect::<Result<Vec<_>, _>>()?;
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
) -> Result<SemanticClaimSubject, ContractWorkflowError> {
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
        return invalid("canonical proposal changed operation or resource cardinality");
    }
    rebind_path(&mut subject.path, &operations, &resources)?;
    subject.artifact_case = canonical_case.id.clone();
    canonical
        .claim_id(&subject)
        .map_err(|error| invalid_error(error.to_string()))?;
    Ok(subject)
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
) -> Result<(), ContractWorkflowError> {
    use solid_reactive_ir::contract_semantics::{ClaimPath, ValueRoot};
    let operation = |id: &mut solid_reactive_ir::contract_semantics::OperationId| {
        *id = operations
            .get(id)
            .cloned()
            .ok_or_else(|| invalid_error("canonical proposal lost an operation identity"))?;
        Ok::<_, ContractWorkflowError>(())
    };
    match path {
        SemanticClaimPath::Domain(ClaimPath::Value { root, .. }) => match root {
            ValueRoot::Export => {}
            ValueRoot::OperationInput { operation: id, .. }
            | ValueRoot::OperationOutput { operation: id } => operation(id)?,
        },
        SemanticClaimPath::Domain(ClaimPath::Operation { operation: id, .. })
        | SemanticClaimPath::Operation(id) => operation(id)?,
        SemanticClaimPath::Domain(ClaimPath::Resource { resource, .. }) => {
            *resource = resources
                .get(resource)
                .cloned()
                .ok_or_else(|| invalid_error("canonical proposal lost a resource identity"))?;
        }
        SemanticClaimPath::Domain(ClaimPath::Call(_) | ClaimPath::GuardPartition) => {}
    }
    Ok(())
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProofDocument {
    format: String,
    proof_version: u16,
    semantic_model_version: u16,
    semantic_digest: String,
    verifier_build: String,
    claims: Vec<ProofClaim>,
    #[serde(default)]
    probe_contradictions: Vec<ProofProbeContradiction>,
    #[serde(default)]
    probe_sidecar: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProofProbeContradiction {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    transcript: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProofClaim {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    families: Vec<ProofFamilyInput>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProofFamilyInput {
    family: String,
    transcript: String,
    #[serde(default)]
    enumerated: Vec<String>,
    #[serde(default)]
    classified: Vec<String>,
    #[serde(default)]
    unresolved: Vec<String>,
    complete: bool,
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
    plans: impl IntoIterator<Item = Vec<u8>>,
) -> Result<Vec<u8>, ContractWorkflowError> {
    let contract = contract_document_v2::decode(merged_document)?.normalize()?;
    let mut candidates = BTreeSet::new();
    for bytes in plans {
        let plan = decode_plan(&bytes)?;
        for claim in plan.closure_candidates {
            let mut subject = SemanticClaimSubject::try_from(claim.subject)
                .map_err(|error| invalid_error(error.to_string()))?;
            subject.artifact_case = contract
                .artifact_cases()
                .iter()
                .find(|artifact| artifact_key(artifact) == claim.artifact)
                .ok_or_else(|| {
                    invalid_error("proposal plan artifact is absent from merged contract")
                })?
                .id
                .clone();
            candidates.insert(subject);
        }
    }
    encode_plan(&contract, candidates)
}

pub fn review(document: &[u8]) -> Result<Vec<u8>, ContractWorkflowError> {
    let contract = contract_document_v2::decode(document)?.normalize()?;
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

pub fn verify(
    proposal_bytes: &[u8],
    plan_bytes: &[u8],
    proof_bytes: &[u8],
    selected_artifact_case: &str,
    pretty: bool,
) -> Result<AcceptedArtifacts, ContractWorkflowError> {
    let proposal = contract_document_v2::decode(proposal_bytes)?.normalize()?;
    let plan = decode_plan(plan_bytes)?;
    validate_identity(
        &proposal,
        plan.semantic_model_version,
        &plan.semantic_digest,
        "proposal plan",
    )?;
    let proof: ProofDocument = decode(proof_bytes)?;
    if proof.format != PROOF_FORMAT || proof.proof_version != PROOF_VERSION {
        return invalid(format!(
            "proof transcript must use format {PROOF_FORMAT:?} version {PROOF_VERSION}"
        ));
    }
    validate_identity(
        &proposal,
        proof.semantic_model_version,
        &proof.semantic_digest,
        "proof transcript",
    )?;
    if proof.verifier_build.trim().is_empty() {
        return invalid("proof transcript verifierBuild must not be empty");
    }
    let candidates = plan
        .closure_candidates
        .into_iter()
        .map(|claim| {
            let subject = SemanticClaimSubject::try_from(claim.subject)
                .map_err(|error| invalid_error(error.to_string()))?;
            let derived = proposal
                .claim_id(&subject)
                .map_err(|error| invalid_error(error.to_string()))?;
            if derived.as_str() != claim.claim_id {
                return Err(invalid_error(
                    "proposal plan claim ID does not match its subject",
                ));
            }
            Ok((claim.claim_id, subject))
        })
        .collect::<Result<BTreeMap<_, _>, ContractWorkflowError>>()?;
    let mut closed = Vec::new();
    let mut replayed = Vec::new();
    let mut seen = BTreeSet::new();
    for claim in proof.claims {
        if !seen.insert(claim.claim_id.clone()) {
            return invalid(format!("proof repeats claim {}", claim.claim_id));
        }
        let subject = SemanticClaimSubject::try_from(claim.subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        let Some(planned) = candidates.get(&claim.claim_id) else {
            return invalid(format!("proof names unplanned claim {}", claim.claim_id));
        };
        if planned != &subject || subject.artifact_case != selected_artifact_case {
            return invalid(format!(
                "proof claim {} is outside the selected planned artifact",
                claim.claim_id
            ));
        }
        let mut families = BTreeSet::new();
        for family_input in claim.families {
            let family = parse_family(&family_input.family)?;
            if !families.insert(family) {
                return invalid(format!(
                    "proof repeats family {:?} for claim {}",
                    family, claim.claim_id
                ));
            }
            let transcript = serde_json::to_vec(&family_input).map_err(decode_error)?;
            replayed.push(
                replay_proof_rule(
                    &proposal,
                    family,
                    subject.clone(),
                    ProofRuleInput {
                        authority: family_authority(family),
                        transcript,
                        observed_scope: proof_scope_digest(&proposal, family, &subject)
                            .map_err(proof_error)?,
                        enumerated: parse_digests(family_input.enumerated)?,
                        classified: parse_digests(family_input.classified)?,
                        unresolved: parse_digests(family_input.unresolved)?,
                        completeness: if family_input.complete {
                            CensusCompleteness::Complete
                        } else {
                            CensusCompleteness::Incomplete
                        },
                    },
                )
                .map_err(proof_error)?,
            );
        }
        for family in CLOSURE_PROOF_FAMILIES {
            if !families.contains(&family) {
                return invalid(format!(
                    "proof claim {} is missing family {:?}",
                    claim.claim_id, family
                ));
            }
        }
        closed.push(subject);
    }
    let mut contradictions = Vec::new();
    for contradiction in proof.probe_contradictions {
        let subject = SemanticClaimSubject::try_from(contradiction.subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        let claim = proposal
            .claim_id(&subject)
            .map_err(|error| invalid_error(error.to_string()))?;
        if claim.as_str() != contradiction.claim_id {
            return invalid("probe contradiction claim ID does not match its subject");
        }
        contradictions.push(ProofContradiction {
            claim,
            transcript: Digest::parse(contradiction.transcript)
                .map_err(|error| invalid_error(error.to_string()))?,
        });
    }
    let probe_sidecar = proof
        .probe_sidecar
        .map(Digest::parse)
        .transpose()
        .map_err(|error| invalid_error(error.to_string()))?;
    if !contradictions.is_empty() && probe_sidecar.is_none() {
        return invalid("probe contradictions require a bound probe sidecar digest");
    }
    let verified = verify_closure(ClosureVerificationRequest {
        contract: proposal,
        selected_artifact_case: selected_artifact_case.into(),
        closed_claims: closed,
        proofs: replayed,
        contradictions,
        verifier: VerifierIdentity {
            build: proof.verifier_build,
            policy: PROOF_POLICY_VERSION,
        },
    })
    .map_err(proof_error)?;
    let sidecars = SidecarDigests {
        proof: Some(
            Digest::parse(format!("sha256:{:x}", Sha256::digest(proof_bytes)))
                .expect("SHA-256 formatting is canonical"),
        ),
        probes: probe_sidecar,
    };
    let document = contract_document_v2::encode(verified.contract(), &sidecars, pretty)?;
    let accepted = verified.issue(&document).map_err(proof_error)?;
    Ok(AcceptedArtifacts {
        receipt: encode_acceptance_receipt(accepted.receipt())?,
        document,
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

fn parse_digests(values: Vec<String>) -> Result<Vec<Digest>, ContractWorkflowError> {
    values
        .into_iter()
        .map(|value| Digest::parse(value).map_err(|error| invalid_error(error.to_string())))
        .collect()
}

fn parse_family(value: &str) -> Result<ProofFamily, ContractWorkflowError> {
    let family = match value {
        "package-identity" => ProofFamily::PackageIdentity,
        "manifest-entrypoint" => ProofFamily::ManifestEntrypoint,
        "export-resolution" => ProofFamily::ExportResolution,
        "artifact-declarations" => ProofFamily::ArtifactDeclarations,
        "export-identity" => ProofFamily::ExportIdentity,
        "module-closure" => ProofFamily::ModuleClosure,
        "selected-signature" => ProofFamily::SelectedSignature,
        "argument-binding" => ProofFamily::ArgumentBinding,
        "rest-spread-coverage" => ProofFamily::RestSpreadCoverage,
        "callable-path" => ProofFamily::CallablePath,
        "operation-reachability" => ProofFamily::OperationReachability,
        "operation-cardinality" => ProofFamily::OperationCardinality,
        "recursive-value-shape" => ProofFamily::RecursiveValueShape,
        "guard-partition" => ProofFamily::GuardPartition,
        "compiler-reconciliation" => ProofFamily::CompilerReconciliation,
        "accepted-dependency-composition" => ProofFamily::AcceptedDependencyComposition,
        "domain-exhaustiveness" => ProofFamily::DomainExhaustiveness,
        "probe-consistency" => ProofFamily::ProbeConsistency,
        _ => return invalid(format!("unknown proof family {value:?}")),
    };
    Ok(family)
}

fn proof_family_name(family: ProofFamily) -> &'static str {
    match family {
        ProofFamily::PackageIdentity => "package-identity",
        ProofFamily::ManifestEntrypoint => "manifest-entrypoint",
        ProofFamily::ExportResolution => "export-resolution",
        ProofFamily::ArtifactDeclarations => "artifact-declarations",
        ProofFamily::ExportIdentity => "export-identity",
        ProofFamily::ModuleClosure => "module-closure",
        ProofFamily::SelectedSignature => "selected-signature",
        ProofFamily::ArgumentBinding => "argument-binding",
        ProofFamily::RestSpreadCoverage => "rest-spread-coverage",
        ProofFamily::CallablePath => "callable-path",
        ProofFamily::OperationReachability => "operation-reachability",
        ProofFamily::OperationCardinality => "operation-cardinality",
        ProofFamily::RecursiveValueShape => "recursive-value-shape",
        ProofFamily::GuardPartition => "guard-partition",
        ProofFamily::CompilerReconciliation => "compiler-reconciliation",
        ProofFamily::AcceptedDependencyComposition => "accepted-dependency-composition",
        ProofFamily::DomainExhaustiveness => "domain-exhaustiveness",
        ProofFamily::ProbeConsistency => "probe-consistency",
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

fn proof_error(error: impl std::fmt::Display) -> ContractWorkflowError {
    ContractWorkflowError::Proof {
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

#[cfg(test)]
mod tests;
