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
    Policy2ReceiptProvenance, Policy2TrustConfiguration, ProbeHarnessConfiguration,
    ProbeHarnessError, TypeFactsCertificationError, TypeFactsProducerPin,
    VerifiedDependencyComposition, VerifiedProbeGateBatch, VerifiedTypeFactsEvidence,
    authenticate_policy2_receipt, issue_policy2_receipt, policy2_main_semantic_digest,
    policy2_trust_configuration_for_issuer, probe_harness,
};

pub struct FinalizedPolicy2Contract {
    canonical_main: Vec<u8>,
    receipt: Vec<u8>,
    bindings: Policy2ReceiptBindings,
    authenticated: AuthenticatedPolicy2Receipt,
    trust_configuration: Policy2TrustConfiguration,
    /// The closure candidates recipe-gated planning withheld before this
    /// contract was planned (`CertificationPlan::recipe_gated`). Audit
    /// material: the canonical main already says those domains are open, and
    /// the receipt binds nothing about them.
    withheld_closures: Vec<super::WithheldClosure>,
}

impl FinalizedPolicy2Contract {
    #[must_use]
    pub fn canonical_main(&self) -> &[u8] {
        &self.canonical_main
    }

    #[must_use]
    pub fn withheld_closures(&self) -> &[super::WithheldClosure] {
        &self.withheld_closures
    }

    pub(super) fn with_withheld_closures(mut self, withheld: Vec<super::WithheldClosure>) -> Self {
        self.withheld_closures = withheld;
        self
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

/// Runs and authenticates every mandatory probe veto this plan derived.
///
/// An empty schedule authenticates on its own: the certifier walked the
/// normalized artifact case and found no proposed closure to veto, and it is
/// not going to launch a fake harness to say so. A nonempty schedule is
/// executed here, inside the certification transaction and nowhere else, and
/// authenticates only against the harness identity that actually ran it.
pub(super) fn authenticate_probe_gates(
    plan: &CertificationPlan,
    probes: Option<&ProbeHarnessConfiguration>,
    pin: &TypeFactsProducerPin,
) -> Result<VerifiedProbeGateBatch, Policy2FinalizationError> {
    authenticate_probe_gates_with_dependencies(plan, probes, pin, &[])
}

pub(super) fn authenticate_probe_gates_with_dependencies(
    plan: &CertificationPlan,
    probes: Option<&ProbeHarnessConfiguration>,
    pin: &TypeFactsProducerPin,
    dependencies: &[&CertificationPlan],
) -> Result<VerifiedProbeGateBatch, Policy2FinalizationError> {
    let schedule = plan.probe_gate_schedule()?;
    if schedule.gates().is_empty() {
        let inspected = schedule.inspect_outcomes([])?;
        return Ok(schedule.authenticate(inspected)?);
    }
    let configuration = probes.ok_or(Policy2FinalizationError::ProbeAuthorityRequired)?;
    let (evaluation, identity) =
        probe_harness::run_probe_gates(plan, &schedule, configuration, pin, dependencies)?;
    let outcomes = schedule.outcomes_from_evaluation(&evaluation)?;
    let inspected = match schedule.inspect_outcomes(outcomes) {
        Ok(inspected) => inspected,
        // The gate says only that it did not complete; the evaluation knows
        // why. Carry that with the gate so the withheld record can say it
        // (ADR 0036) instead of naming a gate digest and nothing else.
        Err(super::ProbeGateError::IncompleteGate(gate_id)) => {
            let detail = schedule
                .gates()
                .iter()
                .find(|gate| gate.id() == gate_id)
                .and_then(|gate| evaluation.incompletion(gate.semantic_claim_id()))
                .map_or_else(
                    || "the evaluation recorded no completion for the gate".to_owned(),
                    str::to_owned,
                );
            return Err(Policy2FinalizationError::IncompleteGate { gate_id, detail });
        }
        Err(error) => return Err(error.into()),
    };
    Ok(schedule.authenticate_with_harness(inspected, &identity)?)
}

/// Whether any demand of the plan is one a Type Facts session answers. A plan
/// without one — every demand satisfied by the artifact snapshot itself, no
/// value claim, no closure candidate — has nothing to ask a producer, and
/// policy 2 finalizes it with the `item-count:0` producer-sessions root rather
/// than refusing it for lacking evidence nothing demanded.
pub(super) fn requires_type_facts(plan: &CertificationPlan) -> bool {
    plan.demand_graph.demands().iter().any(|demand| {
        matches!(
            demand.family(),
            ProofFamily::SelectedSignature
                | ProofFamily::ArgumentBinding
                | ProofFamily::RestSpreadCoverage
                | ProofFamily::CallablePath
                | ProofFamily::OperationReachability
                | ProofFamily::OperationCardinality
                | ProofFamily::RecursiveValueShape
                | ProofFamily::DomainExhaustiveness
        )
    })
}

/// Finalizes a plan no Type Facts demand applies to (see
/// [`requires_type_facts`]): no producer session is opened and none is bound.
/// A plan that does require one refuses inside with `TypeFactsRequired`.
pub(super) fn finalize_value_only_without_type_facts(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    probe_gates: &VerifiedProbeGateBatch,
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
    finalize_value_only_with_dependencies(
        plan,
        proposal_document,
        None,
        None,
        probe_gates,
        pin,
        issuer,
        revocation_epoch,
    )
}

pub(super) fn finalize_value_only(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    type_facts: &VerifiedTypeFactsEvidence,
    probe_gates: &VerifiedProbeGateBatch,
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
    finalize_value_only_with_dependencies(
        plan,
        proposal_document,
        Some(type_facts),
        None,
        probe_gates,
        pin,
        issuer,
        revocation_epoch,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one authority token per fact domain, each with no public constructor"
)]
pub(super) fn finalize_value_only_with_dependencies(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    type_facts: Option<&VerifiedTypeFactsEvidence>,
    dependencies: Option<&VerifiedDependencyComposition>,
    probe_gates: &VerifiedProbeGateBatch,
    pin: &TypeFactsProducerPin,
    issuer: &ConfiguredReceiptIssuer,
    revocation_epoch: u64,
) -> Result<FinalizedPolicy2Contract, Policy2FinalizationError> {
    if probe_gates.requires_controlled_execution() {
        return Err(Policy2FinalizationError::ControlledExecutionRequired);
    }
    let (canonical_main, bindings) = prepare_value_only(
        plan,
        proposal_document,
        type_facts,
        dependencies,
        probe_gates,
        pin,
    )?;
    let verifier_build_digest = &bindings.verifier_build_digest;
    let receipt = issue_policy2_receipt(&canonical_main, &bindings, issuer)?;
    let trust_configuration =
        policy2_trust_configuration_for_issuer(issuer, verifier_build_digest, revocation_epoch)?;
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
        withheld_closures: Vec::new(),
    })
}

/// Shared positive-proof verification, deliberately before any signature is
/// issued. Controlled execution must never create an extractable v2 receipt.
pub(super) fn prepare_value_only(
    plan: &CertificationPlan,
    proposal_document: &[u8],
    type_facts: Option<&VerifiedTypeFactsEvidence>,
    dependencies: Option<&VerifiedDependencyComposition>,
    probe_gates: &VerifiedProbeGateBatch,
    pin: &TypeFactsProducerPin,
) -> Result<(Vec<u8>, Policy2ReceiptBindings), Policy2FinalizationError> {
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
        // A closed claim domain. Its witness is the Type Facts
        // `DomainExhaustiveness` census (`type_facts::require_domain_closure`,
        // `require_closed_value`), which is what actually proves the domain
        // enumerates every behavior possible for this exact export. The probe
        // gate below is only a veto over the same claim and never a substitute
        // for that witness.
        ProofFamily::DomainExhaustiveness,
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
    if requires_type_facts(plan) && type_facts.is_none() {
        return Err(Policy2FinalizationError::TypeFactsRequired);
    }
    probe_gates.verify_plan(plan)?;

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
    let empty = |domain: &str| empty_authority_root(plan, domain);
    // Zero gates keeps the canonical empty authority root byte-identical, so
    // every receipt already issued against an empty schedule stays valid. A
    // nonempty batch binds the exact gate ids *and* the harness/runtime image
    // that ran them: the same claim vetoed by a different Node binary or a
    // different harness image is a different root.
    let probe_gate_root = if probe_gates.gate_ids().is_empty() {
        empty_probe_gate_root(plan)
    } else {
        let policy_digest = proof_policy_2().digest().as_str().to_owned();
        let demand_graph_root = plan.demand_graph.root().as_str().to_owned();
        let mut values = vec![
            policy_digest.as_str(),
            demand_graph_root.as_str(),
            "schedule-version:1",
        ];
        values.extend(probe_gates.gate_ids().iter().map(String::as_str));
        values.extend(probe_gates.harness_identity_fields());
        root("probe-gate-schedule", values)
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
        probe_gate_root,
        closed_claims_root,
        verifier_source_digest: pin.source_manifest_sha256().to_owned(),
        verifier_build_digest: verifier_build_digest.clone(),
    };
    Ok((canonical_main, bindings))
}

impl From<super::RecipeGatingError> for Policy2FinalizationError {
    fn from(error: super::RecipeGatingError) -> Self {
        Self::RecipeGating(Box::new(error))
    }
}

/// The canonical empty authority root for one adapter's schedule: domain-
/// separated by policy digest, demand-graph root, schedule version, and a zero
/// item count. Never a shared zero hash and never caller-supplied.
fn empty_authority_root(plan: &CertificationPlan, domain: &str) -> String {
    root(
        domain,
        [
            proof_policy_2().digest().as_str(),
            plan.demand_graph.root().as_str(),
            "schedule-version:1",
            "item-count:0",
        ],
    )
}

/// The root a receipt binds when the verifier itself derived an *empty* probe
/// schedule. Shared with the tracer tests so "a launched veto does not reuse
/// the empty root" is checkable rather than asserted by eye.
pub(super) fn empty_probe_gate_root(plan: &CertificationPlan) -> String {
    empty_authority_root(plan, "empty-probe-gate-schedule")
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
    #[error(
        "profiled evidence requires the controlled execution consumer; ordinary receipt issuance refused"
    )]
    ControlledExecutionRequired,
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
    #[error(
        "policy-2 value-only finalization requires a bound harness for its nonempty probe schedule"
    )]
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
    /// A mandatory veto that ended in an error, a timeout, or a refused run,
    /// with the evaluation's account of why. `Probe(IncompleteGate)` is the
    /// same fact without it, from a path that never saw the evaluation.
    #[error("mandatory probe gate {gate_id} did not complete: {detail}")]
    IncompleteGate { gate_id: String, detail: String },
    #[error(transparent)]
    ProbeHarness(#[from] ProbeHarnessError),
    /// Boxed: the gating error carries a whole planning error, and unboxed it
    /// would grow this enum — and every graph-lane `Result` that wraps it —
    /// past the size Clippy's `result_large_err` accepts.
    #[error(transparent)]
    RecipeGating(Box<super::RecipeGatingError>),
    #[error("synthesized veto corpus could not be prepared: {0}")]
    VetoSynthesis(String),
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
