//! Policy-owned package-contract certification.

use std::sync::LazyLock;

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use super::{
    ContractProposal, Digest, ModelError, NormalizedContract, SEMANTIC_MODEL_VERSION,
    SemanticClaimPath, SemanticClaimSubject,
};

const POLICY_VERSION: u32 = 2;
const PROOF_VERSION: u16 = 2;
const RECEIPT_VERSION: u16 = 2;
const POLICY_DIGEST_DOMAIN: &str = "solid-checker:contract-proof-policy:v2";

/// Internal policy-2 authority. The product loader remains on policy 1 until
/// the receipts, catalogs, native loader, and WASM loader move atomically.
pub struct ProofPolicy2;

/// Verifier-derived facts that must survive certification planning.
///
/// The source candidate list is deliberately not an argument. This typestate
/// is constructed only by walking the normalized candidate, so a proof
/// document cannot omit a proposed closure or positive operation from the
/// planner's universe.
pub struct CertificationCandidates {
    candidate_semantic_digest: Digest,
    proposal: NormalizedContract,
    closure_candidates: Vec<SemanticClaimSubject>,
    positive_operations: Vec<SemanticClaimSubject>,
}

impl CertificationCandidates {
    #[must_use]
    pub const fn candidate_semantic_digest(&self) -> &Digest {
        &self.candidate_semantic_digest
    }

    #[must_use]
    pub const fn proposal(&self) -> &NormalizedContract {
        &self.proposal
    }

    #[must_use]
    pub fn closure_candidates(&self) -> &[SemanticClaimSubject] {
        &self.closure_candidates
    }

    #[must_use]
    pub fn positive_operations(&self) -> &[SemanticClaimSubject] {
        &self.positive_operations
    }
}

static POLICY: ProofPolicy2 = ProofPolicy2;
static CANONICAL_MANIFEST: LazyLock<Vec<u8>> = LazyLock::new(|| {
    serde_json::to_vec(&manifest()).expect("the static proof-policy manifest must serialize")
});
static AUDIT_MANIFEST: LazyLock<Vec<u8>> = LazyLock::new(|| {
    serde_json::to_vec_pretty(&manifest())
        .expect("the static proof-policy audit manifest must serialize")
});
static POLICY_DIGEST: LazyLock<Digest> = LazyLock::new(|| {
    let manifest = CANONICAL_MANIFEST.as_slice();
    let mut hash = Sha256::new();
    hash.update((POLICY_DIGEST_DOMAIN.len() as u64).to_be_bytes());
    hash.update(POLICY_DIGEST_DOMAIN.as_bytes());
    hash.update((manifest.len() as u64).to_be_bytes());
    hash.update(manifest);
    Digest::from_sha256(hash.finalize().into())
});

#[must_use]
pub fn proof_policy_2() -> &'static ProofPolicy2 {
    &POLICY
}

impl ProofPolicy2 {
    #[must_use]
    pub const fn policy_version(&self) -> u32 {
        POLICY_VERSION
    }

    #[must_use]
    pub const fn proof_version(&self) -> u16 {
        PROOF_VERSION
    }

    #[must_use]
    pub const fn receipt_version(&self) -> u16 {
        RECEIPT_VERSION
    }

    #[must_use]
    pub const fn semantic_model_version(&self) -> u16 {
        SEMANTIC_MODEL_VERSION
    }

    /// Canonical bytes hashed into every policy-2 demand and receipt.
    ///
    /// The checked-in JSON is an audit rendering of these Rust-owned values;
    /// it is never loaded as runtime policy.
    #[must_use]
    pub fn canonical_manifest(&self) -> &'static [u8] {
        CANONICAL_MANIFEST.as_slice()
    }

    /// Human-readable rendering generated from the same Rust-owned policy.
    #[must_use]
    pub fn audit_manifest(&self) -> &'static [u8] {
        AUDIT_MANIFEST.as_slice()
    }

    #[must_use]
    pub fn digest(&self) -> &'static Digest {
        &POLICY_DIGEST
    }

    /// Rebuilds the certification candidate universe from normalized meaning.
    ///
    /// Closure is withdrawn in the returned proposal; known positive
    /// operations remain present and independently inventoried for later
    /// witness demands.
    pub fn inspect_candidates(
        &self,
        candidate: &NormalizedContract,
    ) -> Result<CertificationCandidates, ModelError> {
        let package = candidate.package().clone();
        let mut artifact_cases = candidate.artifact_cases().to_vec();
        let mut closure_candidates = Vec::new();
        let mut positive_operations = Vec::new();

        for artifact in &mut artifact_cases {
            for (export_name, export) in &mut artifact.exports {
                positive_operations.extend(export.call.operations.iter().map(|operation| {
                    SemanticClaimSubject {
                        artifact_case: artifact.id.clone(),
                        export: export_name.clone(),
                        path: SemanticClaimPath::Operation(operation.id.clone()),
                    }
                }));
                closure_candidates.extend(export.open_proposed_closure().into_iter().map(|path| {
                    SemanticClaimSubject {
                        artifact_case: artifact.id.clone(),
                        export: export_name.clone(),
                        path: SemanticClaimPath::Domain(path),
                    }
                }));
            }
        }
        closure_candidates.sort();
        closure_candidates.dedup();
        positive_operations.sort();
        positive_operations.dedup();

        Ok(CertificationCandidates {
            candidate_semantic_digest: candidate.semantic_digest().clone(),
            proposal: ContractProposal::new(package, artifact_cases).normalize()?,
            closure_candidates,
            positive_operations,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyManifest {
    format: &'static str,
    policy_version: u32,
    proof_version: u16,
    receipt_version: u16,
    semantic_model_version: u16,
    status: &'static str,
    canonical_encoding: CanonicalEncoding,
    digests: &'static [DigestRule],
    coverage: Coverage,
    applicability: Applicability,
    producer_constraints: ProducerConstraints,
    resource_budgets: ResourceBudgets,
    receipt_trust: ReceiptTrust,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalEncoding {
    name: &'static str,
    version: u16,
}

#[derive(Serialize)]
struct DigestRule {
    purpose: &'static str,
    algorithm: &'static str,
    domain: &'static str,
    framing: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Coverage {
    positive_facts: &'static str,
    closures: &'static str,
    inapplicability: &'static str,
    unproved_positive: &'static str,
    unproved_closure: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Applicability {
    artifact_prerequisites: &'static [DemandRule],
    claim_families: &'static [DemandRule],
    compiler_reconciliation: DemandRule,
    dependency_composition: DemandRule,
    probe_consistency: ProbeRule,
}

#[derive(Clone, Copy, Serialize)]
struct DemandRule {
    family: ProofFamily,
    authority: ProofAuthority,
    #[serde(rename = "appliesTo")]
    applies_to: DemandTarget,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeRule {
    family: ProofFamily,
    authority: ProofAuthority,
    mode: &'static str,
    applies_to: DemandTarget,
    missing_required_probe: &'static str,
    absence_proves_negative: bool,
    successful_observation: &'static str,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProofFamily {
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

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ProofAuthority {
    PackageArtifacts,
    TypeFacts,
    CompilerExecutionFacts,
    AcceptedDependencyContract,
    RuntimeProbe,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DemandTarget {
    ArtifactCase,
    SelectedCallSignature,
    ArgumentOrCallbackBinding,
    RestOrSpreadSite,
    CallableValuePath,
    OperationOrOperationEdge,
    OperationCardinality,
    RecursiveValueItem,
    GuardOrPartition,
    ProposedDomainClosure,
    CompilerOwnedSite,
    RelevantExternalDependencyEdge,
    VerifierScheduledProbe,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProducerConstraints {
    package_artifacts: ArtifactProducerConstraints,
    type_facts: ProcessProducerConstraints,
    compiler_execution_facts: ProcessProducerConstraints,
    accepted_dependency_contract: DependencyProducerConstraints,
    runtime_probe: ProbeProducerConstraints,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactProducerConstraints {
    session: &'static str,
    registry_or_lock_provenance_required: bool,
    lifecycle_scripts_allowed: bool,
    caller_serialized_authority_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessProducerConstraints {
    session: &'static str,
    executable_digest_required: bool,
    runtime_digest_required: bool,
    build_and_protocol_identity_required: bool,
    caller_serialized_authority_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyProducerConstraints {
    receipt: &'static str,
    exact_dependency_edge_binding_required: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeProducerConstraints {
    harness: &'static str,
    harness_digest_required: bool,
    environment_identity_required: bool,
    contradiction_is_veto: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceBudgets {
    contract_document_bytes: usize,
    proof_document_bytes: usize,
    receipt_bytes: usize,
    json_depth: usize,
    json_nodes: usize,
    string_bytes: usize,
    demands: usize,
    witness_items_per_demand: usize,
    archive_bytes: usize,
    expanded_archive_bytes: usize,
    archive_members: usize,
    package_path_bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptTrust {
    canonical_main_encoding_required: bool,
    policy_digest_constraint_required: bool,
    receipt_carried_key_is_trust_root: bool,
    built_in: BuiltInReceiptTrust,
    persistent_local: SignedReceiptTrust,
    portable: PortableReceiptTrust,
}

#[derive(Serialize)]
struct BuiltInReceiptTrust {
    provenance: &'static str,
    #[serde(rename = "externalSignatureRequired")]
    external_signature_required: bool,
}

#[derive(Serialize)]
struct SignedReceiptTrust {
    algorithm: &'static str,
    #[serde(rename = "configuredIssuerAndTrustRootRequired")]
    configured_issuer_and_trust_root_required: bool,
}

#[derive(Serialize)]
struct PortableReceiptTrust {
    algorithm: &'static str,
    #[serde(rename = "explicitTrustStoreChainRequired")]
    explicit_trust_store_chain_required: bool,
}

const DIGESTS: [DigestRule; 4] = [
    DigestRule {
        purpose: "policy",
        algorithm: "sha256",
        domain: POLICY_DIGEST_DOMAIN,
        framing: "u64be-length-prefixed-domain-and-payload",
    },
    DigestRule {
        purpose: "demand-id",
        algorithm: "sha256",
        domain: "solid-checker:contract-proof-demand:v2",
        framing: "typed-length-prefixed-fields",
    },
    DigestRule {
        purpose: "evidence-root",
        algorithm: "sha256",
        domain: "solid-checker:contract-proof-evidence:v2",
        framing: "typed-length-prefixed-fields",
    },
    DigestRule {
        purpose: "receipt-payload",
        algorithm: "sha256",
        domain: "solid-checker:acceptance-receipt:v2",
        framing: "typed-length-prefixed-fields",
    },
];

const ARTIFACT_PREREQUISITES: [DemandRule; 6] = [
    artifact_rule(ProofFamily::PackageIdentity),
    artifact_rule(ProofFamily::ManifestEntrypoint),
    artifact_rule(ProofFamily::ExportResolution),
    artifact_rule(ProofFamily::ArtifactDeclarations),
    artifact_rule(ProofFamily::ExportIdentity),
    artifact_rule(ProofFamily::ModuleClosure),
];

const CLAIM_FAMILIES: [DemandRule; 9] = [
    type_facts_rule(
        ProofFamily::SelectedSignature,
        DemandTarget::SelectedCallSignature,
    ),
    type_facts_rule(
        ProofFamily::ArgumentBinding,
        DemandTarget::ArgumentOrCallbackBinding,
    ),
    type_facts_rule(
        ProofFamily::RestSpreadCoverage,
        DemandTarget::RestOrSpreadSite,
    ),
    type_facts_rule(ProofFamily::CallablePath, DemandTarget::CallableValuePath),
    type_facts_rule(
        ProofFamily::OperationReachability,
        DemandTarget::OperationOrOperationEdge,
    ),
    type_facts_rule(
        ProofFamily::OperationCardinality,
        DemandTarget::OperationCardinality,
    ),
    type_facts_rule(
        ProofFamily::RecursiveValueShape,
        DemandTarget::RecursiveValueItem,
    ),
    type_facts_rule(ProofFamily::GuardPartition, DemandTarget::GuardOrPartition),
    type_facts_rule(
        ProofFamily::DomainExhaustiveness,
        DemandTarget::ProposedDomainClosure,
    ),
];

const fn artifact_rule(family: ProofFamily) -> DemandRule {
    DemandRule {
        family,
        authority: ProofAuthority::PackageArtifacts,
        applies_to: DemandTarget::ArtifactCase,
    }
}

const fn type_facts_rule(family: ProofFamily, applies_to: DemandTarget) -> DemandRule {
    DemandRule {
        family,
        authority: ProofAuthority::TypeFacts,
        applies_to,
    }
}

const fn manifest() -> PolicyManifest {
    PolicyManifest {
        format: "solid-checker-contract-proof-policy",
        policy_version: POLICY_VERSION,
        proof_version: PROOF_VERSION,
        receipt_version: RECEIPT_VERSION,
        semantic_model_version: SEMANTIC_MODEL_VERSION,
        status: "internal-not-active",
        canonical_encoding: CanonicalEncoding {
            name: "utf8-minified-ordered-json",
            version: 1,
        },
        digests: &DIGESTS,
        coverage: Coverage {
            positive_facts: "all-analyzer-visible-candidates",
            closures: "all-proposed-closures",
            inapplicability: "verifier-derived",
            unproved_positive: "remove-or-refuse",
            unproved_closure: "leave-open-or-refuse",
        },
        applicability: Applicability {
            artifact_prerequisites: &ARTIFACT_PREREQUISITES,
            claim_families: &CLAIM_FAMILIES,
            compiler_reconciliation: DemandRule {
                family: ProofFamily::CompilerReconciliation,
                authority: ProofAuthority::CompilerExecutionFacts,
                applies_to: DemandTarget::CompilerOwnedSite,
            },
            dependency_composition: DemandRule {
                family: ProofFamily::AcceptedDependencyComposition,
                authority: ProofAuthority::AcceptedDependencyContract,
                applies_to: DemandTarget::RelevantExternalDependencyEdge,
            },
            probe_consistency: ProbeRule {
                family: ProofFamily::ProbeConsistency,
                authority: ProofAuthority::RuntimeProbe,
                mode: "separate-veto",
                applies_to: DemandTarget::VerifierScheduledProbe,
                missing_required_probe: "refuse",
                absence_proves_negative: false,
                successful_observation: "possible-positive-only",
            },
        },
        producer_constraints: ProducerConstraints {
            package_artifacts: ArtifactProducerConstraints {
                session: "immutable-snapshot",
                registry_or_lock_provenance_required: true,
                lifecycle_scripts_allowed: false,
                caller_serialized_authority_accepted: false,
            },
            type_facts: ProcessProducerConstraints {
                session: "direct-fresh-process",
                executable_digest_required: true,
                runtime_digest_required: true,
                build_and_protocol_identity_required: true,
                caller_serialized_authority_accepted: false,
            },
            compiler_execution_facts: ProcessProducerConstraints {
                session: "direct-fresh-process",
                executable_digest_required: true,
                runtime_digest_required: true,
                build_and_protocol_identity_required: true,
                caller_serialized_authority_accepted: false,
            },
            accepted_dependency_contract: DependencyProducerConstraints {
                receipt: "authenticated-policy-2",
                exact_dependency_edge_binding_required: true,
            },
            runtime_probe: ProbeProducerConstraints {
                harness: "isolated-bounded",
                harness_digest_required: true,
                environment_identity_required: true,
                contradiction_is_veto: true,
            },
        },
        resource_budgets: ResourceBudgets {
            contract_document_bytes: 1024 * 1024,
            proof_document_bytes: 16 * 1024 * 1024,
            receipt_bytes: 64 * 1024,
            json_depth: 128,
            json_nodes: 1_000_000,
            string_bytes: 16 * 1024,
            demands: 65_536,
            witness_items_per_demand: 16_384,
            archive_bytes: 256 * 1024 * 1024,
            expanded_archive_bytes: 1024 * 1024 * 1024,
            archive_members: 100_000,
            package_path_bytes: 4 * 1024,
        },
        receipt_trust: ReceiptTrust {
            canonical_main_encoding_required: true,
            policy_digest_constraint_required: true,
            receipt_carried_key_is_trust_root: false,
            built_in: BuiltInReceiptTrust {
                provenance: "binary-embedded",
                external_signature_required: false,
            },
            persistent_local: SignedReceiptTrust {
                algorithm: "ed25519",
                configured_issuer_and_trust_root_required: true,
            },
            portable: PortableReceiptTrust {
                algorithm: "ed25519",
                explicit_trust_store_chain_required: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::proof_policy_2;
    use crate::contract_semantics::{
        KnowledgeState, SEMANTIC_MODEL_VERSION, SemanticClaimPath,
        proof::{ACCEPTANCE_RECEIPT_VERSION, PROOF_POLICY_VERSION},
        solid2_rc3::conformance_corpus,
    };

    const AUDIT_MANIFEST: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/package-contract-v2/phase19/proof-policy-v2.json"
    ));

    #[test]
    fn policy_2_is_canonical_rust_owned_and_inactive() {
        let policy = proof_policy_2();

        assert_eq!(policy.policy_version(), 2);
        assert_eq!(policy.proof_version(), 2);
        assert_eq!(policy.receipt_version(), 2);
        assert_eq!(policy.semantic_model_version(), SEMANTIC_MODEL_VERSION);
        assert_eq!(
            policy.digest().as_str(),
            "sha256:a272f8aa3db479a45fabe8a6fcc3272b59a337b67a60c9d7e673a4095fbc507d"
        );
        assert_eq!(PROOF_POLICY_VERSION, 1);
        assert_eq!(ACCEPTANCE_RECEIPT_VERSION, 1);

        assert_eq!(
            policy.audit_manifest(),
            AUDIT_MANIFEST.strip_suffix(b"\n").unwrap_or(AUDIT_MANIFEST)
        );
    }

    #[test]
    fn candidate_inventory_is_verifier_derived_and_preserves_partial_positives() {
        let complete = conformance_corpus()
            .into_iter()
            .next()
            .unwrap()
            .proposal
            .normalize()
            .unwrap();

        let candidates = proof_policy_2().inspect_candidates(&complete).unwrap();

        assert_eq!(
            candidates.candidate_semantic_digest(),
            complete.semantic_digest()
        );
        assert_eq!(candidates.positive_operations().len(), 6);
        assert!(candidates.positive_operations().iter().any(|subject| {
            subject.export == "createEffect"
                && matches!(
                    &subject.path,
                    SemanticClaimPath::Operation(operation) if operation.0 == "initial-compute"
                )
        }));
        assert!(
            candidates
                .closure_candidates()
                .iter()
                .all(|subject| matches!(subject.path, SemanticClaimPath::Domain(_)))
        );

        let reopened = candidates
            .proposal()
            .artifact_case("solid-browser-development")
            .unwrap()
            .exports
            .get("createEffect")
            .unwrap();
        assert_eq!(
            reopened.claim_state(crate::contract_semantics::ClaimDomain::Callbacks),
            KnowledgeState::PartialPositive
        );
        assert_eq!(reopened.call.operations.len(), 6);
        assert_ne!(
            candidates.proposal().semantic_digest(),
            complete.semantic_digest()
        );
    }
}
