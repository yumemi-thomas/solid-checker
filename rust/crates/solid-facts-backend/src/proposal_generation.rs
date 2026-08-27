//! Construction of unaccepted package-contract proposals.
//!
//! Acquisition supplies exact artifact cases and analysis supplies semantic
//! candidates. This module owns the accuracy-critical transition from those
//! candidates to an open proposal: every local closure candidate is retained
//! as a proof obligation and withdrawn from the emitted semantics. Node may
//! orchestrate these stages, but never merges summaries or edits semantic
//! claims.

use std::collections::BTreeSet;

use serde::Serialize;
use solid_reactive_ir::contract_semantics::{
    ArtifactCase, BehaviorStrength, ClaimPath, ContractProposal, Digest, ModelError,
    NormalizedContract, OperationKind, PackageIdentity,
};
use thiserror::Error;

use crate::{
    artifact_resolution::{ResolvedImport, select_and_bind},
    contract_interface::ContractFailure,
};

const PROPOSAL_FORMAT: &str = "solid-checker-contract-proposal";
const PROPOSAL_VERSION: u16 = 1;

/// Exact semantic output of the analysis stage. Artifact identities have
/// already been acquired and bound; nothing here is a wire summary or alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalAnalysis {
    pub package: PackageIdentity,
    pub artifact_cases: Vec<ArtifactCase>,
    /// Independently acquired Phase 7 identities. Every analyzed case must be
    /// selected exactly once through these records before proposal semantics
    /// can be constructed.
    pub resolutions: Vec<ResolvedImport>,
}

/// One recursive semantic leaf, scoped to the exact artifact case and export.
/// Phase 9 will assign position-independent claim IDs; this structured subject
/// is deliberately not such an ID.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LocalProposalClaim {
    pub artifact_case: String,
    pub export: String,
    pub claim: ClaimPath,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PositiveOperationCandidate {
    pub artifact_case: String,
    pub export: String,
    pub operation: String,
    pub kind: OperationKind,
    pub strength: BehaviorStrength,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProofObligationKind {
    ProveClosure,
    ResolveOpenClaim,
    ProvePositiveOperation,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofObligation {
    pub kind: ProofObligationKind,
    pub subject: ProposalSubject,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProposalSubject {
    Claim(LocalProposalClaim),
    Operation(PositiveOperationCandidate),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProbeCandidate {
    pub operation: PositiveOperationCandidate,
}

/// Rust-owned semantic construction. The normalized contract is always open;
/// closure candidates survive only in the separate planning collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedProposal {
    contract: NormalizedContract,
    closure_candidates: Vec<LocalProposalClaim>,
}

impl ConstructedProposal {
    #[must_use]
    pub const fn contract(&self) -> &NormalizedContract {
        &self.contract
    }

    #[must_use]
    pub fn closure_candidates(&self) -> &[LocalProposalClaim] {
        &self.closure_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofPlannedProposal {
    proposal: ConstructedProposal,
    unresolved_edges: Vec<LocalProposalClaim>,
    positive_operations: Vec<PositiveOperationCandidate>,
    proof_obligations: Vec<ProofObligation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedProposal {
    proposal: ConstructedProposal,
    plan: ProposalPlan,
}

impl PlannedProposal {
    #[must_use]
    pub const fn contract(&self) -> &NormalizedContract {
        self.proposal.contract()
    }

    #[must_use]
    pub const fn plan(&self) -> &ProposalPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalPlan {
    semantic_digest: Digest,
    closure_candidates: Vec<LocalProposalClaim>,
    unresolved_edges: Vec<LocalProposalClaim>,
    positive_operations: Vec<PositiveOperationCandidate>,
    proof_obligations: Vec<ProofObligation>,
    probe_candidates: Vec<ProbeCandidate>,
}

impl ProposalPlan {
    #[must_use]
    pub const fn semantic_digest(&self) -> &Digest {
        &self.semantic_digest
    }

    #[must_use]
    pub fn closure_candidates(&self) -> &[LocalProposalClaim] {
        &self.closure_candidates
    }

    #[must_use]
    pub fn unresolved_edges(&self) -> &[LocalProposalClaim] {
        &self.unresolved_edges
    }

    #[must_use]
    pub fn positive_operations(&self) -> &[PositiveOperationCandidate] {
        &self.positive_operations
    }

    #[must_use]
    pub fn proof_obligations(&self) -> &[ProofObligation] {
        &self.proof_obligations
    }

    #[must_use]
    pub fn probe_candidates(&self) -> &[ProbeCandidate] {
        &self.probe_candidates
    }

    /// Monotone union used by acquisition/analysis retries. Repeating a round
    /// or changing round order reaches the same plan; a round for different
    /// semantic meaning is refused rather than mixed into the fixed point.
    pub fn fixed_point(
        rounds: impl IntoIterator<Item = Self>,
    ) -> Result<Self, ProposalGenerationError> {
        let mut rounds = rounds.into_iter();
        let Some(first) = rounds.next() else {
            return Err(ProposalGenerationError::NoRounds);
        };
        let semantic_digest = first.semantic_digest.clone();
        let mut closure_candidates = first
            .closure_candidates
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut unresolved_edges = first.unresolved_edges.into_iter().collect::<BTreeSet<_>>();
        let mut positive_operations = first
            .positive_operations
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut proof_obligations = first.proof_obligations.into_iter().collect::<BTreeSet<_>>();
        let mut probe_candidates = first.probe_candidates.into_iter().collect::<BTreeSet<_>>();
        for round in rounds {
            if round.semantic_digest != semantic_digest {
                return Err(ProposalGenerationError::MixedSemanticDigests);
            }
            closure_candidates.extend(round.closure_candidates);
            unresolved_edges.extend(round.unresolved_edges);
            positive_operations.extend(round.positive_operations);
            proof_obligations.extend(round.proof_obligations);
            probe_candidates.extend(round.probe_candidates);
        }
        Ok(Self {
            semantic_digest,
            closure_candidates: closure_candidates.into_iter().collect(),
            unresolved_edges: unresolved_edges.into_iter().collect(),
            positive_operations: positive_operations.into_iter().collect(),
            proof_obligations: proof_obligations.into_iter().collect(),
            probe_candidates: probe_candidates.into_iter().collect(),
        })
    }
}

#[derive(Debug, Error)]
pub enum ProposalGenerationError {
    #[error("proposal semantic model is invalid: {0}")]
    Model(#[from] ModelError),
    #[error("proposal artifact binding failed: {0}")]
    Artifact(#[from] ContractFailure),
    #[error("proposal analysis has no exact artifact resolutions")]
    MissingArtifactResolutions,
    #[error("exact resolutions cover {selected} of {analyzed} analyzed artifact cases")]
    IncompleteArtifactCoverage { analyzed: usize, selected: usize },
    #[error("multiple exact resolutions select artifact case {artifact_case}")]
    DuplicateArtifactResolution { artifact_case: String },
    #[error("proposal fixed point requires at least one analysis round")]
    NoRounds,
    #[error("proposal fixed point cannot mix different semantic digests")]
    MixedSemanticDigests,
    #[error("proposal emission failed: {0}")]
    Emission(#[from] serde_json::Error),
}

/// Proposal construction stage. Complete-positive knowledge becomes partial;
/// complete-negative knowledge becomes unknown. No unrelated claim is opened.
pub fn construct_proposal(
    analysis: ProposalAnalysis,
) -> Result<ConstructedProposal, ProposalGenerationError> {
    let normalized =
        ContractProposal::new(analysis.package, analysis.artifact_cases).normalize()?;
    if analysis.resolutions.is_empty() {
        return Err(ProposalGenerationError::MissingArtifactResolutions);
    }
    let analyzed = normalized.artifact_cases().len();
    let mut cases = Vec::new();
    let mut selected_ids = BTreeSet::new();
    for resolution in &analysis.resolutions {
        let selected = select_and_bind(&normalized, resolution)?;
        for artifact_case in selected.artifact_cases() {
            if !selected_ids.insert(artifact_case.id.clone()) {
                return Err(ProposalGenerationError::DuplicateArtifactResolution {
                    artifact_case: artifact_case.id.clone(),
                });
            }
            cases.push(artifact_case.clone());
        }
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));
    if cases.len() != analyzed {
        return Err(ProposalGenerationError::IncompleteArtifactCoverage {
            analyzed,
            selected: cases.len(),
        });
    }
    let package = normalized.package().clone();
    let mut closure_candidates = Vec::new();
    for artifact_case in &mut cases {
        for (export_name, export) in &mut artifact_case.exports {
            closure_candidates.extend(export.open_proposed_closure().into_iter().map(|claim| {
                LocalProposalClaim {
                    artifact_case: artifact_case.id.clone(),
                    export: export_name.clone(),
                    claim,
                }
            }));
        }
    }
    closure_candidates.sort();
    closure_candidates.dedup();
    let contract = ContractProposal::new(package, cases).normalize()?;
    Ok(ConstructedProposal {
        contract,
        closure_candidates,
    })
}

/// Proof-planning stage. Natural unknowns and withdrawn closure candidates are
/// separate obligations, so one incomplete leaf cannot erase a provable
/// sibling candidate.
#[must_use]
pub fn plan_proofs(proposal: ConstructedProposal) -> ProofPlannedProposal {
    let closure = proposal
        .closure_candidates
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut unresolved_edges = BTreeSet::new();
    let mut positive_operations = BTreeSet::new();
    for artifact_case in proposal.contract.artifact_cases() {
        for (export_name, export) in &artifact_case.exports {
            unresolved_edges.extend(export.unresolved_claims().into_iter().map(|claim| {
                LocalProposalClaim {
                    artifact_case: artifact_case.id.clone(),
                    export: export_name.clone(),
                    claim,
                }
            }));
            positive_operations.extend(export.call.operations.iter().map(|operation| {
                PositiveOperationCandidate {
                    artifact_case: artifact_case.id.clone(),
                    export: export_name.clone(),
                    operation: operation.id.0.clone(),
                    kind: operation.kind,
                    strength: operation.cardinality.strength(),
                }
            }));
        }
    }
    let mut obligations = BTreeSet::new();
    obligations.extend(closure.iter().cloned().map(|claim| ProofObligation {
        kind: ProofObligationKind::ProveClosure,
        subject: ProposalSubject::Claim(claim),
    }));
    obligations.extend(
        unresolved_edges
            .iter()
            .filter(|claim| !closure.contains(*claim))
            .cloned()
            .map(|claim| ProofObligation {
                kind: ProofObligationKind::ResolveOpenClaim,
                subject: ProposalSubject::Claim(claim),
            }),
    );
    obligations.extend(
        positive_operations
            .iter()
            .cloned()
            .map(|operation| ProofObligation {
                kind: ProofObligationKind::ProvePositiveOperation,
                subject: ProposalSubject::Operation(operation),
            }),
    );
    ProofPlannedProposal {
        proposal,
        unresolved_edges: unresolved_edges.into_iter().collect(),
        positive_operations: positive_operations.into_iter().collect(),
        proof_obligations: obligations.into_iter().collect(),
    }
}

/// Probe-planning stage. Probes can witness possible positives but cannot
/// establish closure, a finite maximum, or complete-negative behavior.
#[must_use]
pub fn plan_probes(planned: ProofPlannedProposal) -> PlannedProposal {
    let probe_candidates = planned
        .positive_operations
        .iter()
        .filter(|operation| operation.strength == BehaviorStrength::Possible)
        .cloned()
        .map(|operation| ProbeCandidate { operation })
        .collect();
    let plan = ProposalPlan {
        semantic_digest: planned.proposal.contract.semantic_digest().clone(),
        closure_candidates: planned.proposal.closure_candidates.clone(),
        unresolved_edges: planned.unresolved_edges,
        positive_operations: planned.positive_operations,
        proof_obligations: planned.proof_obligations,
        probe_candidates,
    };
    PlannedProposal {
        proposal: planned.proposal,
        plan,
    }
}

/// Deterministic proposal-only emission. It intentionally contains no main
/// package-contract document, receipt, evidence sidecar, or accepted closure
/// field. Phase 14 will switch public generators after proof machinery exists.
pub fn emit_proposal(proposal: &PlannedProposal) -> Result<Vec<u8>, ProposalGenerationError> {
    let document = ProposalEmission::from(proposal);
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposalEmission<'a> {
    format: &'static str,
    proposal_version: u16,
    semantic_model_version: u16,
    acceptance: &'static str,
    semantic_digest: &'a str,
    closure_candidates: Vec<ClaimEmission<'a>>,
    unresolved_edges: Vec<ClaimEmission<'a>>,
    positive_operations: Vec<OperationEmission<'a>>,
    proof_obligations: Vec<ObligationEmission<'a>>,
    probe_plan: Vec<OperationEmission<'a>>,
}

impl<'a> From<&'a PlannedProposal> for ProposalEmission<'a> {
    fn from(value: &'a PlannedProposal) -> Self {
        let plan = value.plan();
        Self {
            format: PROPOSAL_FORMAT,
            proposal_version: PROPOSAL_VERSION,
            semantic_model_version: value.contract().semantic_model_version(),
            acceptance: "unaccepted",
            semantic_digest: plan.semantic_digest.as_str(),
            closure_candidates: plan
                .closure_candidates
                .iter()
                .map(ClaimEmission::from)
                .collect(),
            unresolved_edges: plan
                .unresolved_edges
                .iter()
                .map(ClaimEmission::from)
                .collect(),
            positive_operations: plan
                .positive_operations
                .iter()
                .map(OperationEmission::from)
                .collect(),
            proof_obligations: plan
                .proof_obligations
                .iter()
                .map(ObligationEmission::from)
                .collect(),
            probe_plan: plan
                .probe_candidates
                .iter()
                .map(|probe| OperationEmission::from(&probe.operation))
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimEmission<'a> {
    artifact_case: &'a str,
    export: &'a str,
    claim: String,
}

impl<'a> From<&'a LocalProposalClaim> for ClaimEmission<'a> {
    fn from(value: &'a LocalProposalClaim) -> Self {
        Self {
            artifact_case: &value.artifact_case,
            export: &value.export,
            claim: format!("{:?}", value.claim),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationEmission<'a> {
    artifact_case: &'a str,
    export: &'a str,
    operation: &'a str,
    kind: String,
    strength: String,
}

impl<'a> From<&'a PositiveOperationCandidate> for OperationEmission<'a> {
    fn from(value: &'a PositiveOperationCandidate) -> Self {
        Self {
            artifact_case: &value.artifact_case,
            export: &value.export,
            operation: &value.operation,
            kind: format!("{:?}", value.kind),
            strength: format!("{:?}", value.strength),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObligationEmission<'a> {
    kind: String,
    subject: ObligationSubjectEmission<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum ObligationSubjectEmission<'a> {
    Claim(ClaimEmission<'a>),
    Operation(OperationEmission<'a>),
}

impl<'a> From<&'a ProofObligation> for ObligationEmission<'a> {
    fn from(value: &'a ProofObligation) -> Self {
        Self {
            kind: format!("{:?}", value.kind),
            subject: match &value.subject {
                ProposalSubject::Claim(claim) => {
                    ObligationSubjectEmission::Claim(ClaimEmission::from(claim))
                }
                ProposalSubject::Operation(operation) => {
                    ObligationSubjectEmission::Operation(OperationEmission::from(operation))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
