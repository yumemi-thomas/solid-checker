//! Backend adaptation for semantic proof replay and acceptance.
//!
//! Proposal planning and runtime probes live in this crate, while the actual
//! accepted typestate constructor remains in `solid-reactive-ir`. This adapter
//! derives the exact closure set from the Phase 8 plan and converts Phase 10
//! contradiction records without exposing either representation to analyzer
//! consumers.

use solid_reactive_ir::contract_semantics::{
    AcceptedContract, VerifierIdentity,
    proof::{
        AcceptanceRequest, ClosureVerificationRequest, ProofContradiction, ProofError,
        ReplayedProof, verify_and_accept, verify_closure,
    },
};

use crate::{
    contract_document_v2::{self, SidecarDigests},
    contract_interface::ContractFailure,
    proposal_generation::PlannedProposal,
    runtime_probes::ProbeContradictionRecord,
};

pub struct ProposalProofRequest {
    pub proposal: PlannedProposal,
    pub selected_artifact_case: String,
    pub wire_bytes: Vec<u8>,
    pub proofs: Vec<ReplayedProof>,
    pub contradictions: Vec<ProbeContradictionRecord>,
    pub verifier: VerifierIdentity,
}

pub struct ProposalVerificationRequest {
    pub proposal: PlannedProposal,
    pub selected_artifact_case: String,
    pub proofs: Vec<ReplayedProof>,
    pub contradictions: Vec<ProbeContradictionRecord>,
    pub verifier: VerifierIdentity,
    pub sidecars: SidecarDigests,
    pub pretty: bool,
}

pub struct VerifiedContractArtifact {
    pub contract: AcceptedContract,
    pub document: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProposalVerificationError {
    #[error(transparent)]
    Proof(#[from] ProofError),
    #[error(transparent)]
    Document(#[from] ContractFailure),
}

/// Verifies and finalizes exactly the selected artifact case's planned local
/// closure. Naturally unresolved claims never enter this list and therefore
/// cannot be turned into negative proof by omission.
pub fn verify_planned_proposal(
    request: ProposalProofRequest,
) -> Result<AcceptedContract, ProofError> {
    let closed_claims = request
        .proposal
        .plan()
        .closure_candidates()
        .iter()
        .filter(|claim| claim.artifact_case == request.selected_artifact_case)
        .map(|claim| claim.semantic_subject())
        .collect();
    let contradictions = request
        .contradictions
        .into_iter()
        .map(|record| ProofContradiction {
            claim: record.claim_id,
            transcript: record.transcript,
        })
        .collect();
    verify_and_accept(AcceptanceRequest {
        contract: request.proposal.contract().clone(),
        selected_artifact_case: request.selected_artifact_case,
        wire_bytes: request.wire_bytes,
        closed_claims,
        proofs: request.proofs,
        contradictions,
        verifier: request.verifier,
    })
}

/// Proof-first production path. Closure is finalized before encoding, and the
/// receipt is then issued over the encoder's exact temporary-v2 bytes.
pub fn verify_and_encode_planned_proposal(
    request: ProposalVerificationRequest,
) -> Result<VerifiedContractArtifact, ProposalVerificationError> {
    let closed_claims = request
        .proposal
        .plan()
        .closure_candidates()
        .iter()
        .filter(|claim| claim.artifact_case == request.selected_artifact_case)
        .map(|claim| claim.semantic_subject())
        .collect();
    let contradictions = request
        .contradictions
        .into_iter()
        .map(|record| ProofContradiction {
            claim: record.claim_id,
            transcript: record.transcript,
        })
        .collect();
    let verified = verify_closure(ClosureVerificationRequest {
        contract: request.proposal.contract().clone(),
        selected_artifact_case: request.selected_artifact_case,
        closed_claims,
        proofs: request.proofs,
        contradictions,
        verifier: request.verifier,
    })?;
    let document =
        contract_document_v2::encode(verified.contract(), &request.sidecars, request.pretty)?;
    let contract = verified.issue(&document)?;
    Ok(VerifiedContractArtifact { contract, document })
}
