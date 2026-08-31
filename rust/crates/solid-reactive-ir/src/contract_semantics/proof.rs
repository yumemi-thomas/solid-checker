//! Replayable proof authority for normalized package contracts.
//!
//! A proof replay is bound to one normalized semantic claim and exact artifact
//! identity. Callers may submit raw rule inputs, but cannot construct a
//! [`ReplayedProof`] or [`AcceptedContract`] directly. Acceptance closes only
//! the explicitly proved recursive leaves, rejects any probe contradiction,
//! and derives every receipt digest locally.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    AcceptanceReceipt, AcceptedContract, ContractProposal, Digest, ModelError, NormalizedContract,
    ReceiptAuthenticationIdentity, SemanticClaimId, SemanticClaimPath, SemanticClaimSubject,
    VerifierIdentity,
};

pub const ACCEPTANCE_RECEIPT_VERSION: u16 = 1;
pub const PROOF_POLICY_VERSION: u32 = 1;
const MAX_PROOF_TRANSCRIPT_BYTES: usize = 16 * 1024 * 1024;
const MAX_WIRE_BYTES: usize = 16 * 1024 * 1024;

/// Semantic-model-v1 proof rules. Closure requires every family: evidence that
/// is irrelevant for a simple export is represented by a complete empty
/// census, never by silently omitting the rule.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProofFamily {
    PackageIdentity,
    ManifestEntrypoint,
    ExportResolution,
    ArtifactDeclarations,
    ExportIdentity,
    ModuleClosure,
    SelectedSignature,
    ArgumentBinding,
    RestSpreadCoverage,
    CallablePath,
    OperationReachability,
    OperationCardinality,
    RecursiveValueShape,
    GuardPartition,
    CompilerReconciliation,
    AcceptedDependencyComposition,
    DomainExhaustiveness,
    ProbeConsistency,
}

pub const CLOSURE_PROOF_FAMILIES: [ProofFamily; 18] = [
    ProofFamily::PackageIdentity,
    ProofFamily::ManifestEntrypoint,
    ProofFamily::ExportResolution,
    ProofFamily::ArtifactDeclarations,
    ProofFamily::ExportIdentity,
    ProofFamily::ModuleClosure,
    ProofFamily::SelectedSignature,
    ProofFamily::ArgumentBinding,
    ProofFamily::RestSpreadCoverage,
    ProofFamily::CallablePath,
    ProofFamily::OperationReachability,
    ProofFamily::OperationCardinality,
    ProofFamily::RecursiveValueShape,
    ProofFamily::GuardPartition,
    ProofFamily::CompilerReconciliation,
    ProofFamily::AcceptedDependencyComposition,
    ProofFamily::DomainExhaustiveness,
    ProofFamily::ProbeConsistency,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CensusCompleteness {
    Incomplete,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProofAuthority {
    PackageArtifacts,
    TypeFacts,
    CompilerExecutionFacts,
    AcceptedDependencyContract,
    RuntimeProbe,
}

/// Raw output of one proof-rule replay.
///
/// `enumerated` is the complete rule-local universe and `classified` is the
/// subset for which the rule produced a semantic classification. Equality is
/// required after canonicalization. `unresolved` names aliases, spreads,
/// escapes, compiler sites, dependency edges, or recursive children that the
/// rule could not discharge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRuleInput {
    pub authority: ProofAuthority,
    pub transcript: Vec<u8>,
    pub observed_scope: Digest,
    pub enumerated: Vec<Digest>,
    pub classified: Vec<Digest>,
    pub unresolved: Vec<Digest>,
    pub completeness: CensusCompleteness,
}

/// Successful replay token. Its fields are private so an untrusted proposal
/// cannot substitute rule names or caller-created success flags.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReplayedProof {
    family: ProofFamily,
    authority: ProofAuthority,
    claim: SemanticClaimId,
    subject: SemanticClaimSubject,
    semantic_digest: Digest,
    scope_digest: Digest,
    transcript: Digest,
    census_root: Digest,
}

impl ReplayedProof {
    #[must_use]
    pub const fn family(&self) -> ProofFamily {
        self.family
    }

    #[must_use]
    pub const fn claim(&self) -> &SemanticClaimId {
        &self.claim
    }

    #[must_use]
    pub const fn subject(&self) -> &SemanticClaimSubject {
        &self.subject
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofContradiction {
    pub claim: SemanticClaimId,
    pub transcript: Digest,
}

pub struct AcceptanceRequest {
    pub contract: NormalizedContract,
    pub selected_artifact_case: String,
    pub wire_bytes: Vec<u8>,
    pub closed_claims: Vec<SemanticClaimSubject>,
    pub proofs: Vec<ReplayedProof>,
    pub contradictions: Vec<ProofContradiction>,
    pub verifier: VerifierIdentity,
}

/// Proof inputs before any main-document bytes exist. Verification closes the
/// authorized leaves and returns a typestate that can be encoded but cannot be
/// consumed by analysis until those exact bytes receive a receipt.
pub struct ClosureVerificationRequest {
    pub contract: NormalizedContract,
    pub selected_artifact_case: String,
    pub closed_claims: Vec<SemanticClaimSubject>,
    pub proofs: Vec<ReplayedProof>,
    pub contradictions: Vec<ProofContradiction>,
    pub verifier: VerifierIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedContract {
    contract: NormalizedContract,
    selected_artifact_case: String,
    proof_root: Digest,
    closed_claims_root: Digest,
    verifier: VerifierIdentity,
}

impl VerifiedContract {
    #[must_use]
    pub const fn contract(&self) -> &NormalizedContract {
        &self.contract
    }

    /// Issues the receipt only after the proof-finalized model has been
    /// encoded. This ordering removes the old caller-supplied-final-bytes
    /// cycle while keeping [`AcceptedContract`] proof-only.
    pub fn issue(self, wire_bytes: &[u8]) -> Result<AcceptedContract, ProofError> {
        if wire_bytes.is_empty() || wire_bytes.len() > MAX_WIRE_BYTES {
            return Err(ProofError::InvalidWireDocument);
        }
        let package = self.contract.package().clone();
        let selected_case = self
            .contract
            .artifact_case(&self.selected_artifact_case)
            .expect("verified artifact case survives normalization")
            .clone();
        let receipt = AcceptanceReceipt {
            receipt_version: ACCEPTANCE_RECEIPT_VERSION,
            wire_digest: Digest::from_sha256(Sha256::digest(wire_bytes).into()),
            semantic_model_version: self.contract.semantic_model_version(),
            semantic_digest: self.contract.semantic_digest().clone(),
            artifacts_digest: artifacts_digest(&package, &selected_case),
            closure_digest: selected_case.dependency_closure.clone(),
            proof_root: self.proof_root,
            closed_claims_root: self.closed_claims_root,
            verifier: self.verifier,
            authentication: None,
        };
        Ok(AcceptedContract {
            package,
            selected_case,
            receipt,
        })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProofError {
    #[error("proof names an invalid semantic claim: {0}")]
    Claim(String),
    #[error("proof artifact scope does not match the normalized claim")]
    ScopeMismatch,
    #[error("proof rule {family:?} did not certify a complete census")]
    IncompleteCensus { family: ProofFamily },
    #[error("proof rule {family:?} left {count} premise(s) unresolved")]
    UnresolvedPremises { family: ProofFamily, count: usize },
    #[error("proof rule {family:?} classified a different site census")]
    CensusMismatch { family: ProofFamily },
    #[error("proof rule {family:?} transcript is empty or exceeds the resource limit")]
    InvalidTranscript { family: ProofFamily },
    #[error("proof rule {family:?} requires {expected:?}, not {actual:?}")]
    WrongAuthority {
        family: ProofFamily,
        expected: ProofAuthority,
        actual: ProofAuthority,
    },
    #[error("acceptance selects missing artifact case {artifact_case}")]
    MissingArtifactCase { artifact_case: String },
    #[error("acceptance claim is outside selected artifact case {artifact_case}")]
    WrongArtifactCase { artifact_case: String },
    #[error("acceptance may close domain claims only")]
    OperationIsNotClosure,
    #[error("acceptance repeats closed claim {claim}")]
    DuplicateClosedClaim { claim: String },
    #[error("acceptance has no local closure claims")]
    NoClosedClaims,
    #[error("acceptance wire document is empty or exceeds the resource limit")]
    InvalidWireDocument,
    #[error("proof replay is stale or belongs to another normalized contract")]
    StaleReplay,
    #[error("proof replay is not planned for closed claim {claim}")]
    OrphanProof { claim: String },
    #[error("proof family {family:?} is repeated for claim {claim}")]
    DuplicateProof { claim: String, family: ProofFamily },
    #[error("closed claim {claim} is missing proof family {family:?}")]
    MissingProof { claim: String, family: ProofFamily },
    #[error("runtime probe transcript {transcript} contradicts closed claim {claim}")]
    ProbeContradiction { claim: String, transcript: String },
    #[error("verifier build identity must not be empty")]
    EmptyVerifierBuild,
    #[error("proof policy {actual} is below required policy {required}")]
    PolicyDowngrade { required: u32, actual: u32 },
    #[error("verified local closure is invalid: {0}")]
    Model(#[from] ModelError),
}

/// Refusal from replaying an already issued receipt against the final compact
/// document selected for one actual import. Raw proof material is deliberately
/// absent: the receipt is the ordinary-analysis authority, while every binding
/// that can drift after issuance is recomputed from normalized meaning.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ReceiptValidationError {
    #[error("unsupported acceptance receipt version {actual}; expected {expected}")]
    ReceiptVersion { expected: u16, actual: u16 },
    #[error("receipt verifier build identity must not be empty")]
    EmptyVerifierBuild,
    #[error("unsupported proof policy {actual}; expected {expected}")]
    ProofPolicy { expected: u32, actual: u32 },
    #[error("receipt selects missing artifact case {artifact_case}")]
    MissingArtifactCase { artifact_case: String },
    #[error("acceptance receipt has no locally closed semantic claim")]
    NoClosedClaims,
    #[error("acceptance receipt does not bind the selected contract: {field}")]
    Mismatch { field: &'static str },
    #[error("accepted semantic claim is invalid: {0}")]
    Claim(String),
}

/// Canonical expected scope for a rule. Static fact and proof producers must
/// derive the same value from their independently acquired identities.
pub fn proof_scope_digest(
    contract: &NormalizedContract,
    family: ProofFamily,
    subject: &SemanticClaimSubject,
) -> Result<Digest, ProofError> {
    let claim = contract
        .claim_id(subject)
        .map_err(|error| ProofError::Claim(error.to_string()))?;
    let case = contract
        .artifact_case(&subject.artifact_case)
        .ok_or_else(|| ProofError::MissingArtifactCase {
            artifact_case: subject.artifact_case.clone(),
        })?;
    let export = case.exports.get(&subject.export).ok_or_else(|| {
        ProofError::Claim(format!(
            "missing export {} in artifact case {}",
            subject.export, subject.artifact_case
        ))
    })?;
    let mut hash = CanonicalHash::new(b"solid-checker-proof-scope-v1");
    hash.u8(family_code(family));
    hash.text(contract.semantic_digest().as_str());
    hash.text(claim.as_str());
    hash.text(&contract.package().name);
    hash.text(&contract.package().version);
    hash.text(&contract.package().integrity);
    hash.text(&contract.package().manifest.path);
    hash.text(contract.package().manifest.digest.as_str());
    hash.text(&case.id);
    hash.text(&case.entrypoint);
    hash.usize(case.resolution_trace.len());
    for step in &case.resolution_trace {
        hash.text(&step.condition);
        hash.text(&step.target);
    }
    hash.text(&case.runtime.path);
    hash.text(case.runtime.digest.as_str());
    hash.text(&case.declarations.path);
    hash.text(case.declarations.digest.as_str());
    hash.text(case.dependency_closure.as_str());
    hash.optional_artifact(case.transform.as_ref());
    hash.text(&export.identity.public_name);
    hash.text(&export.identity.runtime.module.path);
    hash.text(export.identity.runtime.module.digest.as_str());
    hash.text(&export.identity.runtime.export_name);
    hash.text(&export.identity.declarations.module.path);
    hash.text(export.identity.declarations.module.digest.as_str());
    hash.text(&export.identity.declarations.export_name);
    Ok(hash.finish())
}

/// Replays one proof family and returns an opaque success token only for a
/// complete, contradiction-free local census bound to the exact claim.
pub fn replay_proof_rule(
    contract: &NormalizedContract,
    family: ProofFamily,
    subject: SemanticClaimSubject,
    mut input: ProofRuleInput,
) -> Result<ReplayedProof, ProofError> {
    if input.transcript.is_empty() || input.transcript.len() > MAX_PROOF_TRANSCRIPT_BYTES {
        return Err(ProofError::InvalidTranscript { family });
    }
    let expected_authority = family_authority(family);
    if input.authority != expected_authority {
        return Err(ProofError::WrongAuthority {
            family,
            expected: expected_authority,
            actual: input.authority,
        });
    }
    let expected_scope = proof_scope_digest(contract, family, &subject)?;
    if input.observed_scope != expected_scope {
        return Err(ProofError::ScopeMismatch);
    }
    if input.completeness != CensusCompleteness::Complete {
        return Err(ProofError::IncompleteCensus { family });
    }
    if !input.unresolved.is_empty() {
        return Err(ProofError::UnresolvedPremises {
            family,
            count: input.unresolved.len(),
        });
    }
    canonicalize_digests(&mut input.enumerated);
    canonicalize_digests(&mut input.classified);
    if input.enumerated != input.classified {
        return Err(ProofError::CensusMismatch { family });
    }
    let claim = contract
        .claim_id(&subject)
        .map_err(|error| ProofError::Claim(error.to_string()))?;
    let census_root = census_root(&input.enumerated);
    Ok(ReplayedProof {
        family,
        authority: input.authority,
        claim,
        subject,
        semantic_digest: contract.semantic_digest().clone(),
        scope_digest: expected_scope,
        transcript: Digest::from_sha256(Sha256::digest(&input.transcript).into()),
        census_root,
    })
}

/// Replays every closure proof and returns the selected, proof-finalized
/// one-case model for wire emission.
pub fn verify_closure(request: ClosureVerificationRequest) -> Result<VerifiedContract, ProofError> {
    if request.verifier.build.is_empty() {
        return Err(ProofError::EmptyVerifierBuild);
    }
    if request.verifier.policy < PROOF_POLICY_VERSION {
        return Err(ProofError::PolicyDowngrade {
            required: PROOF_POLICY_VERSION,
            actual: request.verifier.policy,
        });
    }
    if request.closed_claims.is_empty() {
        return Err(ProofError::NoClosedClaims);
    }
    if request
        .contract
        .artifact_case(&request.selected_artifact_case)
        .is_none()
    {
        return Err(ProofError::MissingArtifactCase {
            artifact_case: request.selected_artifact_case,
        });
    }

    let mut closed = BTreeMap::new();
    for subject in request.closed_claims {
        if subject.artifact_case != request.selected_artifact_case {
            return Err(ProofError::WrongArtifactCase {
                artifact_case: subject.artifact_case,
            });
        }
        if !matches!(subject.path, SemanticClaimPath::Domain(_)) {
            return Err(ProofError::OperationIsNotClosure);
        }
        let claim = request
            .contract
            .claim_id(&subject)
            .map_err(|error| ProofError::Claim(error.to_string()))?;
        if closed.insert(claim.clone(), subject).is_some() {
            return Err(ProofError::DuplicateClosedClaim {
                claim: claim.as_str().into(),
            });
        }
    }

    for contradiction in request.contradictions {
        if closed.contains_key(&contradiction.claim) {
            return Err(ProofError::ProbeContradiction {
                claim: contradiction.claim.as_str().into(),
                transcript: contradiction.transcript.as_str().into(),
            });
        }
    }

    let mut proofs = request.proofs;
    proofs.sort();
    let mut coverage = BTreeSet::new();
    for proof in &proofs {
        if proof.semantic_digest != *request.contract.semantic_digest()
            || proof.scope_digest
                != proof_scope_digest(&request.contract, proof.family, &proof.subject)?
            || request.contract.claim_id(&proof.subject).ok().as_ref() != Some(&proof.claim)
        {
            return Err(ProofError::StaleReplay);
        }
        if !closed.contains_key(&proof.claim) {
            return Err(ProofError::OrphanProof {
                claim: proof.claim.as_str().into(),
            });
        }
        if !coverage.insert((proof.claim.clone(), proof.family)) {
            return Err(ProofError::DuplicateProof {
                claim: proof.claim.as_str().into(),
                family: proof.family,
            });
        }
    }
    for claim in closed.keys() {
        for family in CLOSURE_PROOF_FAMILIES {
            if !coverage.contains(&(claim.clone(), family)) {
                return Err(ProofError::MissingProof {
                    claim: claim.as_str().into(),
                    family,
                });
            }
        }
    }

    let package = request.contract.package().clone();
    let mut cases = request.contract.artifact_cases().to_vec();
    for (claim, subject) in &closed {
        let case = cases
            .iter_mut()
            .find(|case| case.id == subject.artifact_case)
            .expect("validated artifact case");
        let export = case
            .exports
            .get_mut(&subject.export)
            .expect("validated export");
        let SemanticClaimPath::Domain(path) = &subject.path else {
            unreachable!("validated closure path")
        };
        export
            .close_verified_claim(path)
            .map_err(|error| ProofError::Claim(format!("{}: {error}", claim.as_str())))?;
    }
    let finalized = ContractProposal::new(package.clone(), cases).normalize()?;
    let selected_case = finalized
        .artifact_case(&request.selected_artifact_case)
        .expect("selected case survives normalization")
        .clone();
    let selected = ContractProposal::new(package, vec![selected_case]).normalize()?;
    Ok(VerifiedContract {
        contract: selected,
        selected_artifact_case: request.selected_artifact_case,
        proof_root: proof_root(&proofs),
        closed_claims_root: closed_claims_root(closed.keys()),
        verifier: request.verifier,
    })
}

/// Compatibility wrapper for callers that already have final wire bytes. New
/// producers must call [`verify_closure`], encode `VerifiedContract::contract`,
/// and then call [`VerifiedContract::issue`].
pub fn verify_and_accept(request: AcceptanceRequest) -> Result<AcceptedContract, ProofError> {
    let verified = verify_closure(ClosureVerificationRequest {
        contract: request.contract,
        selected_artifact_case: request.selected_artifact_case,
        closed_claims: request.closed_claims,
        proofs: request.proofs,
        contradictions: request.contradictions,
        verifier: request.verifier,
    })?;
    verified.issue(&request.wire_bytes)
}

/// Validates a stored verifier-issued receipt and exposes accepted typestate.
///
/// This is the only ordinary-analysis constructor. It intentionally accepts a
/// one-case normalized contract: artifact selection and exact export rebinding
/// must already have happened at the backend normalization seam. The proof
/// root remains opaque receipt authority; all identities derivable without raw
/// sidecars are recomputed here and must match exactly.
pub fn validate_receipt_and_accept(
    contract: NormalizedContract,
    selected_artifact_case: &str,
    receipt: AcceptanceReceipt,
) -> Result<AcceptedContract, ReceiptValidationError> {
    if receipt.receipt_version != ACCEPTANCE_RECEIPT_VERSION {
        return Err(ReceiptValidationError::ReceiptVersion {
            expected: ACCEPTANCE_RECEIPT_VERSION,
            actual: receipt.receipt_version,
        });
    }
    if receipt.verifier.build.is_empty() {
        return Err(ReceiptValidationError::EmptyVerifierBuild);
    }
    if receipt.verifier.policy != PROOF_POLICY_VERSION {
        return Err(ReceiptValidationError::ProofPolicy {
            expected: PROOF_POLICY_VERSION,
            actual: receipt.verifier.policy,
        });
    }
    if receipt.semantic_model_version != contract.semantic_model_version() {
        return Err(ReceiptValidationError::Mismatch {
            field: "semanticModelVersion",
        });
    }
    if receipt.semantic_digest != *contract.semantic_digest() {
        return Err(ReceiptValidationError::Mismatch {
            field: "semanticDigest",
        });
    }
    let selected_case = contract
        .artifact_case(selected_artifact_case)
        .ok_or_else(|| ReceiptValidationError::MissingArtifactCase {
            artifact_case: selected_artifact_case.into(),
        })?
        .clone();
    if contract.artifact_cases().len() != 1 {
        return Err(ReceiptValidationError::Mismatch {
            field: "selectedArtifactCase",
        });
    }
    if receipt.artifacts_digest != artifacts_digest(contract.package(), &selected_case) {
        return Err(ReceiptValidationError::Mismatch {
            field: "artifactsDigest",
        });
    }
    if receipt.closure_digest != selected_case.dependency_closure {
        return Err(ReceiptValidationError::Mismatch {
            field: "closureDigest",
        });
    }

    let mut closed = BTreeSet::new();
    for (export_name, export) in &selected_case.exports {
        for path in super::validate::closed_claims(export) {
            let subject = SemanticClaimSubject {
                artifact_case: selected_case.id.clone(),
                export: export_name.clone(),
                path: SemanticClaimPath::Domain(path),
            };
            let claim = contract
                .claim_id(&subject)
                .map_err(|error| ReceiptValidationError::Claim(error.to_string()))?;
            closed.insert(claim);
        }
    }
    if closed.is_empty() {
        return Err(ReceiptValidationError::NoClosedClaims);
    }
    if receipt.closed_claims_root != closed_claims_root(closed.iter()) {
        return Err(ReceiptValidationError::Mismatch {
            field: "closedClaimsRoot",
        });
    }

    Ok(AcceptedContract {
        package: contract.package().clone(),
        selected_case,
        receipt,
    })
}

fn export_has_positive_semantics(export: &super::ExportSemantics) -> bool {
    !matches!(export.shape, super::ValueShape::Unknown)
        || !export.call.operations.is_empty()
        || !export.call.edges.is_empty()
        || !export.call.resources.is_empty()
        || !export.call.claims().callbacks.items().is_empty()
        || !export.call.guards.cases.items().is_empty()
}

/// Receipt fields that remain authoritative only after the backend has
/// authenticated a policy-2 issuer and its complete signed payload.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPolicy2Acceptance {
    pub main_digest: Digest,
    pub semantic_digest: Digest,
    pub receipt_digest: Digest,
    pub policy_digest: Digest,
    pub closed_claims_root: Digest,
    pub verifier_build_digest: Digest,
    pub trust_store_digest: Digest,
    pub revocation_epoch: u64,
}

/// Constructs analyzer typestate only from a backend-authenticated policy-2
/// identity, while recomputing every contract-derived identity locally.
#[doc(hidden)]
pub fn accept_authenticated_policy2(
    contract: NormalizedContract,
    selected_artifact_case: &str,
    authenticated: AuthenticatedPolicy2Acceptance,
) -> Result<AcceptedContract, ReceiptValidationError> {
    if authenticated.semantic_digest != *contract.semantic_digest() {
        return Err(ReceiptValidationError::Mismatch {
            field: "semanticDigest",
        });
    }
    let selected_case = contract
        .artifact_case(selected_artifact_case)
        .ok_or_else(|| ReceiptValidationError::MissingArtifactCase {
            artifact_case: selected_artifact_case.into(),
        })?
        .clone();
    if contract.artifact_cases().len() != 1 {
        return Err(ReceiptValidationError::Mismatch {
            field: "selectedArtifactCase",
        });
    }
    let actual_closed_claims_root = derive_closed_claims_root(&contract, &selected_case)?;
    if authenticated.closed_claims_root != actual_closed_claims_root {
        return Err(ReceiptValidationError::Mismatch {
            field: "closedClaimsRoot",
        });
    }
    let receipt = AcceptanceReceipt {
        receipt_version: 2,
        wire_digest: authenticated.main_digest,
        semantic_model_version: contract.semantic_model_version(),
        semantic_digest: authenticated.semantic_digest,
        artifacts_digest: artifacts_digest(contract.package(), &selected_case),
        closure_digest: selected_case.dependency_closure.clone(),
        proof_root: authenticated.receipt_digest.clone(),
        closed_claims_root: authenticated.closed_claims_root,
        verifier: VerifierIdentity {
            build: authenticated.verifier_build_digest.as_str().into(),
            policy: 2,
        },
        authentication: Some(ReceiptAuthenticationIdentity {
            receipt_digest: authenticated.receipt_digest,
            policy_digest: authenticated.policy_digest,
            trust_store_digest: authenticated.trust_store_digest,
            revocation_epoch: authenticated.revocation_epoch,
        }),
    };
    Ok(AcceptedContract {
        package: contract.package().clone(),
        selected_case,
        receipt,
    })
}

/// Projects one normalized proposal into the semantic query shape used only
/// while generating another open proposal in the same native graph
/// transaction. This is deliberately not receipt authority: the synthetic
/// identity uses receipt version and verifier policy zero and carries no
/// authentication. Ordinary contract discovery must never call this helper;
/// the backend graph planner independently replays and certifies every
/// proposal before any projected semantics can become analyzer input.
#[doc(hidden)]
pub fn project_untrusted_proposal_for_generation(
    contract: NormalizedContract,
    selected_artifact_case: &str,
) -> Result<AcceptedContract, ReceiptValidationError> {
    let selected_case = contract
        .artifact_case(selected_artifact_case)
        .ok_or_else(|| ReceiptValidationError::MissingArtifactCase {
            artifact_case: selected_artifact_case.into(),
        })?
        .clone();
    if contract.artifact_cases().len() != 1 {
        return Err(ReceiptValidationError::Mismatch {
            field: "selectedArtifactCase",
        });
    }
    let closed_claims_root = derive_closed_claims_root(&contract, &selected_case)?;
    let mut projection = Sha256::new();
    projection.update(b"solid-checker:untrusted-generation-projection:v1\0");
    projection.update(contract.semantic_digest().as_str().as_bytes());
    projection.update(selected_artifact_case.as_bytes());
    let projection = Digest::from_sha256(projection.finalize().into());
    Ok(AcceptedContract {
        package: contract.package().clone(),
        selected_case,
        receipt: AcceptanceReceipt {
            receipt_version: 0,
            wire_digest: projection.clone(),
            semantic_model_version: contract.semantic_model_version(),
            semantic_digest: contract.semantic_digest().clone(),
            artifacts_digest: artifacts_digest(
                contract.package(),
                contract
                    .artifact_case(selected_artifact_case)
                    .expect("selected case exists"),
            ),
            closure_digest: contract
                .artifact_case(selected_artifact_case)
                .expect("selected case exists")
                .dependency_closure
                .clone(),
            proof_root: projection,
            closed_claims_root,
            verifier: VerifierIdentity {
                build: "untrusted-generation-projection-v1".into(),
                policy: 0,
            },
            authentication: None,
        },
    })
}

/// Recomputes the closed-claim identity that a policy-2 receipt must bind.
#[doc(hidden)]
pub fn policy2_closed_claims_root(
    contract: &NormalizedContract,
    selected_artifact_case: &str,
) -> Result<Digest, ReceiptValidationError> {
    let selected_case = contract
        .artifact_case(selected_artifact_case)
        .ok_or_else(|| ReceiptValidationError::MissingArtifactCase {
            artifact_case: selected_artifact_case.into(),
        })?;
    derive_closed_claims_root(contract, selected_case)
}

fn derive_closed_claims_root(
    contract: &NormalizedContract,
    selected_case: &super::ArtifactCase,
) -> Result<Digest, ReceiptValidationError> {
    let mut closed = BTreeSet::new();
    for (export_name, export) in &selected_case.exports {
        for path in super::validate::closed_claims(export) {
            let subject = SemanticClaimSubject {
                artifact_case: selected_case.id.clone(),
                export: export_name.clone(),
                path: SemanticClaimPath::Domain(path),
            };
            let claim = contract
                .claim_id(&subject)
                .map_err(|error| ReceiptValidationError::Claim(error.to_string()))?;
            closed.insert(claim);
        }
        if export_has_positive_semantics(export) {
            let mut hash = CanonicalHash::new(b"solid-checker-positive-export-claim-v1");
            hash.text(contract.semantic_digest().as_str());
            hash.text(&selected_case.id);
            hash.text(export_name);
            closed.insert(
                SemanticClaimId::parse(format!("claim:v1:{}", hash.finish().as_str()))
                    .expect("canonical positive-export claim digest is valid"),
            );
        }
    }
    if closed.is_empty() {
        return Err(ReceiptValidationError::NoClosedClaims);
    }
    Ok(closed_claims_root(closed.iter()))
}

fn canonicalize_digests(values: &mut Vec<Digest>) {
    values.sort();
    values.dedup();
}

fn census_root(values: &[Digest]) -> Digest {
    let mut hash = CanonicalHash::new(b"solid-checker-proof-census-v1");
    hash.usize(values.len());
    for value in values {
        hash.text(value.as_str());
    }
    hash.finish()
}

fn proof_root(proofs: &[ReplayedProof]) -> Digest {
    let mut hash = CanonicalHash::new(b"solid-checker-proof-root-v1");
    hash.usize(proofs.len());
    for proof in proofs {
        hash.u8(family_code(proof.family));
        hash.u8(authority_code(proof.authority));
        hash.text(proof.claim.as_str());
        hash.text(proof.scope_digest.as_str());
        hash.text(proof.transcript.as_str());
        hash.text(proof.census_root.as_str());
    }
    hash.finish()
}

fn closed_claims_root<'a>(claims: impl IntoIterator<Item = &'a SemanticClaimId>) -> Digest {
    let claims = claims.into_iter().collect::<Vec<_>>();
    let mut hash = CanonicalHash::new(b"solid-checker-closed-claims-root-v1");
    hash.usize(claims.len());
    for claim in claims {
        hash.text(claim.as_str());
    }
    hash.finish()
}

fn artifacts_digest(package: &super::PackageIdentity, case: &super::ArtifactCase) -> Digest {
    let mut hash = CanonicalHash::new(b"solid-checker-artifacts-root-v1");
    hash.text(&package.name);
    hash.text(&package.version);
    hash.text(&package.integrity);
    hash.text(&package.manifest.path);
    hash.text(package.manifest.digest.as_str());
    hash.text(&case.id);
    hash.text(&case.entrypoint);
    hash.text(&case.runtime.path);
    hash.text(case.runtime.digest.as_str());
    hash.text(&case.declarations.path);
    hash.text(case.declarations.digest.as_str());
    hash.usize(case.resolution_trace.len());
    for step in &case.resolution_trace {
        hash.text(&step.condition);
        hash.text(&step.target);
    }
    hash.optional_artifact(case.transform.as_ref());
    hash.usize(case.exports.len());
    for (name, export) in &case.exports {
        hash.text(name);
        hash.text(&export.identity.runtime.module.path);
        hash.text(export.identity.runtime.module.digest.as_str());
        hash.text(&export.identity.runtime.export_name);
        hash.text(&export.identity.declarations.module.path);
        hash.text(export.identity.declarations.module.digest.as_str());
        hash.text(&export.identity.declarations.export_name);
    }
    hash.finish()
}

const fn family_code(family: ProofFamily) -> u8 {
    match family {
        ProofFamily::PackageIdentity => 1,
        ProofFamily::ManifestEntrypoint => 2,
        ProofFamily::ExportResolution => 3,
        ProofFamily::ArtifactDeclarations => 4,
        ProofFamily::ExportIdentity => 5,
        ProofFamily::ModuleClosure => 6,
        ProofFamily::SelectedSignature => 7,
        ProofFamily::ArgumentBinding => 8,
        ProofFamily::RestSpreadCoverage => 9,
        ProofFamily::CallablePath => 10,
        ProofFamily::OperationReachability => 11,
        ProofFamily::OperationCardinality => 12,
        ProofFamily::RecursiveValueShape => 13,
        ProofFamily::GuardPartition => 14,
        ProofFamily::CompilerReconciliation => 15,
        ProofFamily::AcceptedDependencyComposition => 16,
        ProofFamily::DomainExhaustiveness => 17,
        ProofFamily::ProbeConsistency => 18,
    }
}

#[must_use]
pub const fn family_authority(family: ProofFamily) -> ProofAuthority {
    match family {
        ProofFamily::PackageIdentity
        | ProofFamily::ManifestEntrypoint
        | ProofFamily::ExportResolution
        | ProofFamily::ArtifactDeclarations
        | ProofFamily::ExportIdentity
        | ProofFamily::ModuleClosure => ProofAuthority::PackageArtifacts,
        ProofFamily::SelectedSignature
        | ProofFamily::ArgumentBinding
        | ProofFamily::RestSpreadCoverage
        | ProofFamily::CallablePath
        | ProofFamily::OperationReachability
        | ProofFamily::OperationCardinality
        | ProofFamily::RecursiveValueShape
        | ProofFamily::GuardPartition
        | ProofFamily::DomainExhaustiveness => ProofAuthority::TypeFacts,
        ProofFamily::CompilerReconciliation => ProofAuthority::CompilerExecutionFacts,
        ProofFamily::AcceptedDependencyComposition => ProofAuthority::AcceptedDependencyContract,
        ProofFamily::ProbeConsistency => ProofAuthority::RuntimeProbe,
    }
}

const fn authority_code(authority: ProofAuthority) -> u8 {
    match authority {
        ProofAuthority::PackageArtifacts => 1,
        ProofAuthority::TypeFacts => 2,
        ProofAuthority::CompilerExecutionFacts => 3,
        ProofAuthority::AcceptedDependencyContract => 4,
        ProofAuthority::RuntimeProbe => 5,
    }
}

struct CanonicalHash(Sha256);

impl CanonicalHash {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update((domain.len() as u64).to_be_bytes());
        hash.update(domain);
        Self(hash)
    }

    fn u8(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn usize(&mut self, value: usize) {
        self.0.update((value as u64).to_be_bytes());
    }

    fn text(&mut self, value: &str) {
        self.0.update((value.len() as u64).to_be_bytes());
        self.0.update(value.as_bytes());
    }

    fn optional_artifact(&mut self, value: Option<&super::ArtifactIdentity>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.text(&value.path);
                self.text(value.digest.as_str());
            }
            None => self.u8(0),
        }
    }

    fn finish(self) -> Digest {
        Digest::from_sha256(self.0.finalize().into())
    }
}

#[cfg(test)]
mod tests;
