//! Opaque policy-2 finalization for the first value-only cohort.

use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    certification::{ProofFamily, proof_policy_2},
    proof::policy2_closed_claims_root,
};
use thiserror::Error;

use super::{
    AuthenticatedPolicy2Receipt, CertificationPlan, ConfiguredReceiptIssuer,
    DependencyReceiptCompositionError, Policy2ReceiptBindings, Policy2ReceiptError,
    Policy2ReceiptProvenance, Policy2TrustConfiguration, TypeFactsCertificationError,
    TypeFactsProducerPin, VerifiedDependencyComposition, VerifiedTypeFactsEvidence,
    authenticate_policy2_receipt, issue_policy2_receipt, policy2_main_semantic_digest,
    policy2_trust_configuration_for_issuer,
};

pub struct FinalizedPolicy2Contract {
    canonical_main: Vec<u8>,
    receipt: Vec<u8>,
    bindings: Policy2ReceiptBindings,
    authenticated: AuthenticatedPolicy2Receipt,
    trust_configuration: Policy2TrustConfiguration,
}

impl FinalizedPolicy2Contract {
    #[must_use]
    pub fn canonical_main(&self) -> &[u8] {
        &self.canonical_main
    }

    #[must_use]
    pub fn receipt(&self) -> &[u8] {
        &self.receipt
    }

    #[must_use]
    pub const fn bindings(&self) -> &Policy2ReceiptBindings {
        &self.bindings
    }

    #[must_use]
    pub const fn authenticated(&self) -> &AuthenticatedPolicy2Receipt {
        &self.authenticated
    }

    #[must_use]
    pub const fn trust_configuration(&self) -> &Policy2TrustConfiguration {
        &self.trust_configuration
    }
}

pub(super) fn finalize_value_only(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    type_facts: &VerifiedTypeFactsEvidence,
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
    finalize_value_only_with_dependencies(
        plan,
        proposal_document,
        Some(type_facts),
        None,
        pin,
        issuer,
        revocation_epoch,
    )
}

pub(super) fn finalize_value_only_with_dependencies(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    type_facts: Option<&VerifiedTypeFactsEvidence>,
    dependencies: Option<&VerifiedDependencyComposition>,
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
    let allowed = [
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
        ProofFamily::AcceptedDependencyComposition,
    ];
    if let Some(demand) = plan
        .demand_graph
        .demands()
        .iter()
        .find(|demand| !allowed.contains(&demand.family()))
    {
        return Err(Policy2FinalizationError::UnsupportedDemand {
            family: format!("{:?}", demand.family()),
        });
    }
    let requires_dependencies = !plan.verified_closure.manifest().dependencies.is_empty();
    match (requires_dependencies, dependencies) {
        (true, Some(dependencies)) => dependencies.verify_plan(plan)?,
        (true, None) => return Err(Policy2FinalizationError::DependenciesRequired),
        (false, Some(dependencies)) if dependencies.has_semantic_dependencies() => {
            return Err(Policy2FinalizationError::UnexpectedDependencies);
        }
        (false, Some(dependencies)) => dependencies.verify_plan(plan)?,
        (false, None) => {}
    }
    let requires_type_facts = plan.demand_graph.demands().iter().any(|demand| {
        matches!(
            demand.family(),
            ProofFamily::SelectedSignature
                | ProofFamily::ArgumentBinding
                | ProofFamily::RestSpreadCoverage
                | ProofFamily::CallablePath
                | ProofFamily::OperationReachability
                | ProofFamily::OperationCardinality
                | ProofFamily::RecursiveValueShape
        )
    });
    if requires_type_facts && type_facts.is_none() {
        return Err(Policy2FinalizationError::TypeFactsRequired);
    }
    let probe_schedule = plan.probe_gate_schedule()?;
    if !probe_schedule.gates().is_empty() {
        return Err(Policy2FinalizationError::ProbeAuthorityRequired);
    }

    let mut witnesses = plan.artifact_witnesses.clone();
    if let Some(type_facts) = type_facts {
        witnesses.extend(type_facts.witness_bindings().iter().cloned());
    }
    if let Some(dependencies) = dependencies {
        witnesses.extend(dependencies.witnesses().iter().cloned());
    }
    let coverage = plan.demand_graph.verify_witness_coverage(witnesses)?;
    // Planning independently selects and rebinds exactly one artifact case.
    // The open proposal can therefore be semantically broader than the
    // analyzer-visible candidate that the evidence witnessed. Preserve only
    // its separately validated sidecar digests; encode the retained selected
    // candidate as the final canonical main.
    let decoded_proposal = crate::contract_document::decode(proposal_document)?;
    let sidecars = decoded_proposal.sidecar_digests()?;
    let canonical_main =
        crate::contract_document::encode(&plan.selected_candidate, &sidecars, false)?;
    let semantic_digest = policy2_main_semantic_digest(&canonical_main)?;
    if semantic_digest != plan.demand_graph.candidate_semantic_digest().as_str() {
        return Err(Policy2FinalizationError::ReplanningRequired {
            planned: plan
                .demand_graph
                .candidate_semantic_digest()
                .as_str()
                .to_owned(),
            finalized: semantic_digest,
        });
    }
    let normalized = crate::contract_document::decode(&canonical_main)?.normalize()?;
    if normalized.artifact_cases().len() != 1 {
        return Err(Policy2FinalizationError::ArtifactCaseCount);
    }
    let selected_case = &normalized.artifact_cases()[0];
    let closed_claims_root = policy2_closed_claims_root(&normalized, &selected_case.id)?
        .as_str()
        .to_owned();
    let witness_roots = coverage
        .family_evidence_roots()
        .into_iter()
        .map(|(family, root)| (family, root.as_str().to_owned()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let family = |name: &str| {
        witness_roots
            .get(name)
            .expect("receipt witness family census is total")
            .clone()
    };
    let verifier_build_digest = root(
        "verifier-build",
        [
            option_env!("SOLID_CHECKER_BUILD_ID").unwrap_or("dev"),
            typefacts::v3::TYPE_FACTS_BUILD_ID,
            typefacts::v3::TYPE_FACTS_SCHEMA_SHA256,
        ],
    );
    if let Some(dependencies) = dependencies
        && dependencies
            .verifier_build_digest()
            .is_some_and(|actual| actual != verifier_build_digest)
    {
        return Err(Policy2FinalizationError::DependencyVerifierBuildMismatch);
    }
    let producer_sessions_root = type_facts.map_or_else(
        || {
            root(
                "empty-producer-sessions",
                [
                    pin.executable_sha256(),
                    pin.source_manifest_sha256(),
                    typefacts::v3::TYPE_FACTS_SCHEMA_SHA256,
                    typefacts::v3::TYPE_FACTS_BUILD_ID,
                    "item-count:0",
                ],
            )
        },
        |type_facts| {
            root(
                "producer-sessions",
                [
                    type_facts.session_evidence_root(),
                    pin.executable_sha256(),
                    pin.source_manifest_sha256(),
                    typefacts::v3::TYPE_FACTS_SCHEMA_SHA256,
                    typefacts::v3::TYPE_FACTS_BUILD_ID,
                ],
            )
        },
    );
    let empty = |domain: &str| {
        root(
            domain,
            [
                proof_policy_2().digest().as_str(),
                plan.demand_graph.root().as_str(),
                "schedule-version:1",
                "item-count:0",
            ],
        )
    };
    let bindings = Policy2ReceiptBindings {
        importer: plan.import_request.importer.clone(),
        specifier: plan.import_request.specifier.clone(),
        resolved_import_root: super::policy2_resolved_import_root(&plan.resolved_import)?,
        semantic_digest,
        artifact_provenance_root: plan.snapshot.provenance_root().to_owned(),
        snapshot_root: plan.snapshot.root().to_owned(),
        package_root: family("package-identity"),
        manifest_root: family("manifest-entrypoint"),
        artifacts_root: family("export-resolution"),
        declarations_root: family("artifact-declarations"),
        transform_root: empty("empty-transform-schedule"),
        exports_root: family("export-identity"),
        closure_root: family("module-closure"),
        demand_graph_root: plan.demand_graph.root().as_str().to_owned(),
        verified_positive_root: coverage.evidence_root().as_str().to_owned(),
        witness_roots,
        producer_sessions_root,
        dependency_receipts_root: dependencies.map_or_else(
            || empty("empty-dependency-receipt-schedule"),
            |dependencies| dependencies.receipts_root().into(),
        ),
        dependency_trust_root: dependencies.map_or_else(
            || empty("empty-dependency-trust-schedule"),
            |dependencies| dependencies.trust_root().into(),
        ),
        probe_gate_root: empty("empty-probe-gate-schedule"),
        closed_claims_root,
        verifier_source_digest: pin.source_manifest_sha256().to_owned(),
        verifier_build_digest: verifier_build_digest.clone(),
    };
    let receipt = issue_policy2_receipt(&canonical_main, &bindings, issuer)?;
    let trust_configuration =
        policy2_trust_configuration_for_issuer(issuer, &verifier_build_digest, revocation_epoch)?;
    let provenance = match issuer.kind() {
        super::ReceiptIssuerKind::PersistentLocal => Policy2ReceiptProvenance::PersistentLocal {
            trust_store: trust_configuration.trust_store(),
            scope: issuer.scope(),
        },
        super::ReceiptIssuerKind::Portable => Policy2ReceiptProvenance::Portable {
            trust_store: trust_configuration.trust_store(),
        },
        super::ReceiptIssuerKind::BuiltIn => {
            return Err(Policy2FinalizationError::ConfiguredBuiltInIssuer);
        }
    };
    let authenticated =
        authenticate_policy2_receipt(&canonical_main, &receipt, &bindings, provenance)?;
    Ok(FinalizedPolicy2Contract {
        canonical_main,
        receipt,
        bindings,
        authenticated,
        trust_configuration,
    })
}

fn root<'a>(domain: &str, values: impl IntoIterator<Item = &'a str>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:policy2-finalization-root:v1");
    hash_field(&mut hash, domain);
    for value in values {
        hash_field(&mut hash, value);
    }
    format!("sha256:{:x}", hash.finalize())
}

fn hash_field(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hash.update(value.as_bytes());
}

#[derive(Debug, Error)]
pub enum Policy2FinalizationError {
    #[error("policy-2 value-only finalization does not support demand family {family}")]
    UnsupportedDemand { family: String },
    #[error("policy-2 value-only finalization requires authenticated dependency receipts")]
    DependenciesRequired,
    #[error("policy-2 value-only finalization requires live Type Facts evidence")]
    TypeFactsRequired,
    #[error("policy-2 value-only finalization received dependency authority for a leaf")]
    UnexpectedDependencies,
    #[error("dependency receipts were produced by a different verifier build")]
    DependencyVerifierBuildMismatch,
    #[error("policy-2 value-only finalization requires a bound nonempty probe schedule")]
    ProbeAuthorityRequired,
    #[error(
        "policy-2 finalization changed semantic identity from {planned} to {finalized}; discard evidence and replan"
    )]
    ReplanningRequired { planned: String, finalized: String },
    #[error("policy-2 finalization requires exactly one selected artifact case")]
    ArtifactCaseCount,
    #[error("a configured issuer cannot claim built-in provenance")]
    ConfiguredBuiltInIssuer,
    #[error(transparent)]
    TypeFacts(#[from] TypeFactsCertificationError),
    #[error(transparent)]
    Probe(#[from] super::ProbeGateError),
    #[error(transparent)]
    DependencyComposition(#[from] DependencyReceiptCompositionError),
    #[error(transparent)]
    Coverage(#[from] solid_reactive_ir::contract_semantics::certification::WitnessCoverageError),
    #[error(transparent)]
    Receipt(#[from] Policy2ReceiptError),
    #[error(transparent)]
    Contract(#[from] crate::contract_interface::ContractFailure),
    #[error(transparent)]
    Model(#[from] solid_reactive_ir::contract_semantics::ModelError),
    #[error(transparent)]
    ReceiptValidation(#[from] solid_reactive_ir::contract_semantics::proof::ReceiptValidationError),
}
