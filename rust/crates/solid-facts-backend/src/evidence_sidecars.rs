//! Claim-addressed evidence documents for replacement package contracts.
//!
//! This deep module owns both document families and their bidirectional
//! binding. The main contract names content hashes; each sidecar names exact
//! normalized contract, package, artifact, closure, and semantic claim
//! identity. Validated results retain claim IDs only, so ordinary analysis has
//! no interface through which raw transcripts can leak into its hot path.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    ArtifactIdentity, ClaimDomain, ClaimIdentityError, ClaimPath, Digest, NormalizedContract,
    OperationClaimDomain, OperationId, PackageIdentity, ResourceClaimDomain, ResourceId,
    SemanticClaimId, SemanticClaimPath, SemanticClaimSubject, ValueClaimDomain, ValuePath,
    ValuePathSegment, ValueRoot,
};
use thiserror::Error;

use crate::{contract_document_v2, proposal_generation::PlannedProposal};

pub const PROOF_EVIDENCE_FORMAT: &str = "solid-checker-proof-evidence";
pub const PROBE_EVIDENCE_FORMAT: &str = "solid-checker-runtime-probe-evidence";
pub const EVIDENCE_SIDECAR_VERSION: u16 = 1;

const MAX_SIDECAR_BYTES: usize = 16 * 1024 * 1024;
const MAX_CLAIMS: usize = 65_536;
const MAX_ITEMS_PER_CLAIM: usize = 16_384;
const MAX_STRING_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ToolIdentity {
    pub name: String,
    pub version: String,
    pub build: Digest,
    pub protocol: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FactDomainIdentity {
    OxcSyntax,
    TypeFacts,
    CompilerExecutionFacts,
    AcceptedPackageContract,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FactTranscriptIdentity {
    pub domain: FactDomainIdentity,
    pub transcript: Digest,
    pub generation: Option<u64>,
    pub producer: ToolIdentity,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofInputIdentity {
    pub rule: String,
    pub input: Digest,
    pub tool: ToolIdentity,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SandboxKind {
    None,
    Process,
    Container,
    VirtualMachine,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SandboxIdentity {
    pub kind: SandboxKind,
    pub policy: Option<Digest>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentIdentity {
    pub runtime: ToolIdentity,
    pub os: String,
    pub architecture: String,
    pub conditions: Vec<String>,
    pub sandbox: SandboxIdentity,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeOutcome {
    Planned,
    Witness { transcript: Digest },
    Falsification { transcript: Digest },
    Error { details: Digest },
    Timeout { limit_millis: u64 },
    Refused { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofClaimMaterial {
    pub subject: SemanticClaimSubject,
    pub producer: ToolIdentity,
    pub fact_transcripts: Vec<FactTranscriptIdentity>,
    pub proof_inputs: Vec<ProofInputIdentity>,
    pub coverage_limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeClaimMaterial {
    pub subject: SemanticClaimSubject,
    pub producer: ToolIdentity,
    pub recipe: Digest,
    pub environment: EnvironmentIdentity,
    pub outcome: ProbeOutcome,
    pub coverage_limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceSidecarDocuments {
    proof: Option<Vec<u8>>,
    probes: Option<Vec<u8>>,
    references: EvidenceSidecarReferences,
}

impl EvidenceSidecarDocuments {
    #[must_use]
    pub fn proof(&self) -> Option<&[u8]> {
        self.proof.as_deref()
    }

    #[must_use]
    pub fn probes(&self) -> Option<&[u8]> {
        self.probes.as_deref()
    }

    #[must_use]
    pub const fn references(&self) -> &EvidenceSidecarReferences {
        &self.references
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EvidenceSidecarReferences {
    pub proof: Option<Digest>,
    pub probes: Option<Digest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedEvidenceSidecars {
    proof_claims: Vec<SemanticClaimId>,
    probe_claims: Vec<SemanticClaimId>,
}

impl ValidatedEvidenceSidecars {
    #[must_use]
    pub fn proof_claims(&self) -> &[SemanticClaimId] {
        &self.proof_claims
    }

    #[must_use]
    pub fn probe_claims(&self) -> &[SemanticClaimId] {
        &self.probe_claims
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CatalogClaim {
    subject: SemanticClaimSubject,
    artifact: ArtifactEvidenceIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceCatalog {
    contract: NormalizedContract,
    proof_claims: BTreeMap<SemanticClaimId, CatalogClaim>,
    probe_claims: BTreeMap<SemanticClaimId, CatalogClaim>,
}

impl EvidenceCatalog {
    pub fn new(
        contract: NormalizedContract,
        proof_subjects: impl IntoIterator<Item = SemanticClaimSubject>,
        probe_subjects: impl IntoIterator<Item = SemanticClaimSubject>,
    ) -> Result<Self, EvidenceSidecarError> {
        let proof_claims = catalog_claims(&contract, proof_subjects)?;
        let probe_claims = catalog_claims(&contract, probe_subjects)?;
        Ok(Self {
            contract,
            proof_claims,
            probe_claims,
        })
    }

    pub fn for_proposal(proposal: &PlannedProposal) -> Result<Self, EvidenceSidecarError> {
        Self::new(
            proposal.contract().clone(),
            proposal
                .plan()
                .proof_obligations()
                .iter()
                .map(|obligation| obligation.subject.semantic_subject()),
            proposal
                .plan()
                .probe_candidates()
                .iter()
                .map(|probe| probe.operation.semantic_subject()),
        )
    }

    #[must_use]
    pub const fn contract(&self) -> &NormalizedContract {
        &self.contract
    }
}

fn catalog_claims(
    contract: &NormalizedContract,
    subjects: impl IntoIterator<Item = SemanticClaimSubject>,
) -> Result<BTreeMap<SemanticClaimId, CatalogClaim>, EvidenceSidecarError> {
    let mut claims = BTreeMap::new();
    for subject in subjects {
        let claim_id = contract.claim_id(&subject)?;
        let artifact = artifact_identity(contract, &subject)?;
        match claims.entry(claim_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(CatalogClaim { subject, artifact });
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get().subject == subject => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(EvidenceSidecarError::ClaimIdentityCollision);
            }
        }
    }
    Ok(claims)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArtifactEvidenceIdentity {
    artifact_case: String,
    entrypoint: String,
    runtime: ArtifactIdentity,
    declarations: ArtifactIdentity,
    closure: Digest,
    transform: Option<ArtifactIdentity>,
}

fn artifact_identity(
    contract: &NormalizedContract,
    subject: &SemanticClaimSubject,
) -> Result<ArtifactEvidenceIdentity, EvidenceSidecarError> {
    let case = contract
        .artifact_case(&subject.artifact_case)
        .ok_or_else(|| ClaimIdentityError::MissingArtifactCase {
            artifact_case: subject.artifact_case.clone(),
        })?;
    Ok(ArtifactEvidenceIdentity {
        artifact_case: case.id.clone(),
        entrypoint: case.entrypoint.clone(),
        runtime: case.runtime.clone(),
        declarations: case.declarations.clone(),
        closure: case.dependency_closure.clone(),
        transform: case.transform.clone(),
    })
}

#[derive(Debug, Error)]
pub enum EvidenceSidecarError {
    #[error("main contract is invalid: {0}")]
    Contract(#[from] crate::ContractFailure),
    #[error("semantic claim identity is invalid: {0}")]
    Claim(#[from] ClaimIdentityError),
    #[error("evidence sidecar exceeds the {limit}-byte limit")]
    DocumentTooLarge { limit: usize },
    #[error("evidence sidecar cannot be decoded: {message}")]
    Decode { message: String },
    #[error("unsupported evidence document {actual:?}; expected {expected:?}")]
    DocumentKind {
        expected: &'static str,
        actual: String,
    },
    #[error("unsupported evidence sidecar version {actual}; expected {expected}")]
    Version { expected: u16, actual: u16 },
    #[error("main contract does not match the evidence catalog")]
    MainContractMismatch,
    #[error("main contract references a missing {kind} evidence sidecar")]
    MissingDocument { kind: &'static str },
    #[error("{kind} evidence sidecar is not referenced by the main contract")]
    OrphanDocument { kind: &'static str },
    #[error("{kind} evidence bytes do not match the main contract's sidecar hash")]
    ContentMismatch { kind: &'static str },
    #[error("{kind} evidence sidecar names a stale or cross-package contract")]
    ContractBindingMismatch { kind: &'static str },
    #[error("evidence claim is not planned for the {kind} sidecar")]
    OrphanClaim { kind: &'static str },
    #[error("evidence claim ID does not match its semantic subject")]
    ClaimIdMismatch,
    #[error("evidence claim carries stale or cross-artifact identity")]
    ArtifactMismatch,
    #[error("evidence sidecar repeats claim {claim_id}")]
    DuplicateClaim { claim_id: String },
    #[error("semantic claim ID collision")]
    ClaimIdentityCollision,
    #[error("invalid evidence material: {reason}")]
    InvalidMaterial { reason: String },
    #[error("evidence sidecar emission failed: {message}")]
    Emission { message: String },
}

/// Emits deterministic proof/fact and runtime-probe documents. Empty families
/// are omitted rather than represented as evidence-free sidecars.
pub fn emit_evidence_sidecars(
    catalog: &EvidenceCatalog,
    producer: ToolIdentity,
    proof_material: Vec<ProofClaimMaterial>,
    probe_material: Vec<ProbeClaimMaterial>,
) -> Result<EvidenceSidecarDocuments, EvidenceSidecarError> {
    validate_tool(&producer)?;
    let proof = if proof_material.is_empty() {
        None
    } else {
        Some(emit_proof_document(catalog, &producer, proof_material)?)
    };
    let probes = if probe_material.is_empty() {
        None
    } else {
        Some(emit_probe_document(catalog, &producer, probe_material)?)
    };
    let references = EvidenceSidecarReferences {
        proof: proof.as_deref().map(content_digest),
        probes: probes.as_deref().map(content_digest),
    };
    Ok(EvidenceSidecarDocuments {
        proof,
        probes,
        references,
    })
}

/// Validates both hash directions and every claim/artifact binding. The
/// returned value deliberately contains no raw transcript bytes.
pub fn validate_evidence_sidecars(
    main_document: &[u8],
    catalog: &EvidenceCatalog,
    proof_bytes: Option<&[u8]>,
    probe_bytes: Option<&[u8]>,
) -> Result<ValidatedEvidenceSidecars, EvidenceSidecarError> {
    let decoded = contract_document_v2::decode(main_document)?;
    let references = decoded.sidecar_digests()?;
    let normalized = decoded.normalize()?;
    if normalized.semantic_model_version() != catalog.contract.semantic_model_version()
        || normalized.semantic_digest() != catalog.contract.semantic_digest()
        || normalized.package() != catalog.contract.package()
    {
        return Err(EvidenceSidecarError::MainContractMismatch);
    }
    let proof = validate_reference("proof", references.proof.as_ref(), proof_bytes)?;
    let probes = validate_reference("runtime-probe", references.probes.as_ref(), probe_bytes)?;
    let proof_claims = proof
        .map(|bytes| validate_proof_document(bytes, catalog))
        .transpose()?
        .unwrap_or_default();
    let probe_claims = probes
        .map(|bytes| validate_probe_document(bytes, catalog))
        .transpose()?
        .unwrap_or_default();
    Ok(ValidatedEvidenceSidecars {
        proof_claims,
        probe_claims,
    })
}

fn validate_reference<'a>(
    kind: &'static str,
    reference: Option<&Digest>,
    bytes: Option<&'a [u8]>,
) -> Result<Option<&'a [u8]>, EvidenceSidecarError> {
    match (reference, bytes) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(EvidenceSidecarError::MissingDocument { kind }),
        (None, Some(_)) => Err(EvidenceSidecarError::OrphanDocument { kind }),
        (Some(reference), Some(bytes)) if &content_digest(bytes) == reference => Ok(Some(bytes)),
        (Some(_), Some(_)) => Err(EvidenceSidecarError::ContentMismatch { kind }),
    }
}

fn emit_proof_document(
    catalog: &EvidenceCatalog,
    producer: &ToolIdentity,
    mut material: Vec<ProofClaimMaterial>,
) -> Result<Vec<u8>, EvidenceSidecarError> {
    if material.len() > MAX_CLAIMS {
        return invalid_material("proof claim count exceeds the limit");
    }
    material.sort_by(|left, right| left.subject.cmp(&right.subject));
    let mut seen = BTreeSet::new();
    let mut claims = Vec::with_capacity(material.len());
    for mut item in material {
        validate_tool(&item.producer)?;
        canonicalize_proof_material(&mut item)?;
        let claim_id = catalog.contract.claim_id(&item.subject)?;
        let expected = catalog
            .proof_claims
            .get(&claim_id)
            .ok_or(EvidenceSidecarError::OrphanClaim { kind: "proof" })?;
        if !seen.insert(claim_id.clone()) {
            return Err(EvidenceSidecarError::DuplicateClaim {
                claim_id: claim_id.as_str().into(),
            });
        }
        claims.push(WireProofClaim::new(claim_id, expected, item));
    }
    emit_json(&WireProofDocument {
        format: PROOF_EVIDENCE_FORMAT.into(),
        sidecar_version: EVIDENCE_SIDECAR_VERSION,
        contract: WireContractIdentity::from(catalog.contract()),
        producer: WireToolIdentity::from(producer),
        claims,
    })
}

fn emit_probe_document(
    catalog: &EvidenceCatalog,
    producer: &ToolIdentity,
    mut material: Vec<ProbeClaimMaterial>,
) -> Result<Vec<u8>, EvidenceSidecarError> {
    if material.len() > MAX_CLAIMS {
        return invalid_material("probe claim count exceeds the limit");
    }
    material.sort_by(|left, right| left.subject.cmp(&right.subject));
    let mut seen = BTreeSet::new();
    let mut claims = Vec::with_capacity(material.len());
    for mut item in material {
        validate_tool(&item.producer)?;
        canonicalize_probe_material(&mut item)?;
        let claim_id = catalog.contract.claim_id(&item.subject)?;
        let expected =
            catalog
                .probe_claims
                .get(&claim_id)
                .ok_or(EvidenceSidecarError::OrphanClaim {
                    kind: "runtime-probe",
                })?;
        if !seen.insert(claim_id.clone()) {
            return Err(EvidenceSidecarError::DuplicateClaim {
                claim_id: claim_id.as_str().into(),
            });
        }
        claims.push(WireProbeClaim::new(claim_id, expected, item));
    }
    emit_json(&WireProbeDocument {
        format: PROBE_EVIDENCE_FORMAT.into(),
        sidecar_version: EVIDENCE_SIDECAR_VERSION,
        contract: WireContractIdentity::from(catalog.contract()),
        producer: WireToolIdentity::from(producer),
        claims,
    })
}

fn canonicalize_proof_material(
    material: &mut ProofClaimMaterial,
) -> Result<(), EvidenceSidecarError> {
    if material.fact_transcripts.len() > MAX_ITEMS_PER_CLAIM
        || material.proof_inputs.len() > MAX_ITEMS_PER_CLAIM
        || material.coverage_limitations.len() > MAX_ITEMS_PER_CLAIM
    {
        return invalid_material("proof material count exceeds the per-claim limit");
    }
    for transcript in &material.fact_transcripts {
        validate_tool(&transcript.producer)?;
    }
    for proof in &material.proof_inputs {
        validate_nonempty(&proof.rule, "proof rule")?;
        validate_tool(&proof.tool)?;
    }
    canonicalize_strings(&mut material.coverage_limitations, "coverage limitation")?;
    material.fact_transcripts.sort();
    material.fact_transcripts.dedup();
    material.proof_inputs.sort();
    material.proof_inputs.dedup();
    if material.fact_transcripts.is_empty()
        && material.proof_inputs.is_empty()
        && material.coverage_limitations.is_empty()
    {
        return invalid_material("proof claim has no evidence or coverage limitation");
    }
    Ok(())
}

fn canonicalize_probe_material(
    material: &mut ProbeClaimMaterial,
) -> Result<(), EvidenceSidecarError> {
    validate_environment(&mut material.environment)?;
    if let ProbeOutcome::Refused { reason } = &material.outcome {
        validate_nonempty(reason, "probe refusal reason")?;
    }
    if let ProbeOutcome::Timeout { limit_millis: 0 } = material.outcome {
        return invalid_material("probe timeout must be non-zero");
    }
    canonicalize_strings(&mut material.coverage_limitations, "coverage limitation")?;
    Ok(())
}

fn validate_environment(environment: &mut EnvironmentIdentity) -> Result<(), EvidenceSidecarError> {
    validate_tool(&environment.runtime)?;
    validate_nonempty(&environment.os, "environment operating system")?;
    validate_nonempty(&environment.architecture, "environment architecture")?;
    canonicalize_strings(&mut environment.conditions, "environment condition")?;
    match (environment.sandbox.kind, &environment.sandbox.policy) {
        (SandboxKind::None, None) => {}
        (SandboxKind::None, Some(_)) => {
            return invalid_material("an unsandboxed environment cannot name a sandbox policy");
        }
        (_, Some(_)) => {}
        (_, None) => return invalid_material("a sandboxed environment requires a policy digest"),
    }
    Ok(())
}

fn validate_tool(tool: &ToolIdentity) -> Result<(), EvidenceSidecarError> {
    validate_nonempty(&tool.name, "tool name")?;
    validate_nonempty(&tool.version, "tool version")?;
    if let Some(protocol) = &tool.protocol {
        validate_nonempty(protocol, "tool protocol")?;
    }
    Ok(())
}

fn validate_nonempty(value: &str, field: &str) -> Result<(), EvidenceSidecarError> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES {
        invalid_material(format!(
            "{field} must contain between 1 and {MAX_STRING_BYTES} bytes"
        ))
    } else {
        Ok(())
    }
}

fn canonicalize_strings(values: &mut Vec<String>, field: &str) -> Result<(), EvidenceSidecarError> {
    if values.len() > MAX_ITEMS_PER_CLAIM {
        return invalid_material(format!("{field} count exceeds the limit"));
    }
    for value in values.iter() {
        validate_nonempty(value, field)?;
    }
    values.sort();
    values.dedup();
    Ok(())
}

fn invalid_material<T>(reason: impl Into<String>) -> Result<T, EvidenceSidecarError> {
    Err(EvidenceSidecarError::InvalidMaterial {
        reason: reason.into(),
    })
}

fn emit_json(value: &impl Serialize) -> Result<Vec<u8>, EvidenceSidecarError> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| EvidenceSidecarError::Emission {
            message: error.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn content_digest(bytes: &[u8]) -> Digest {
    Digest::parse(format!("sha256:{:x}", Sha256::digest(bytes)))
        .expect("SHA-256 formatting is canonical")
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProofDocument {
    format: String,
    sidecar_version: u16,
    contract: WireContractIdentity,
    producer: WireToolIdentity,
    claims: Vec<WireProofClaim>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProbeDocument {
    format: String,
    sidecar_version: u16,
    contract: WireContractIdentity,
    producer: WireToolIdentity,
    claims: Vec<WireProbeClaim>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireContractIdentity {
    semantic_model_version: u16,
    semantic_digest: String,
    package: WirePackageIdentity,
}

impl From<&NormalizedContract> for WireContractIdentity {
    fn from(contract: &NormalizedContract) -> Self {
        Self {
            semantic_model_version: contract.semantic_model_version(),
            semantic_digest: contract.semantic_digest().as_str().into(),
            package: WirePackageIdentity::from(contract.package()),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePackageIdentity {
    name: String,
    version: String,
    integrity: String,
    manifest: WireArtifactIdentity,
}

impl From<&PackageIdentity> for WirePackageIdentity {
    fn from(package: &PackageIdentity) -> Self {
        Self {
            name: package.name.clone(),
            version: package.version.clone(),
            integrity: package.integrity.clone(),
            manifest: WireArtifactIdentity::from(&package.manifest),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireArtifactIdentity {
    path: String,
    digest: String,
}

impl From<&ArtifactIdentity> for WireArtifactIdentity {
    fn from(artifact: &ArtifactIdentity) -> Self {
        Self {
            path: artifact.path.clone(),
            digest: artifact.digest.as_str().into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireClaimArtifactIdentity {
    artifact_case: String,
    entrypoint: String,
    runtime: WireArtifactIdentity,
    declarations: WireArtifactIdentity,
    closure: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<WireArtifactIdentity>,
}

impl From<&ArtifactEvidenceIdentity> for WireClaimArtifactIdentity {
    fn from(artifact: &ArtifactEvidenceIdentity) -> Self {
        Self {
            artifact_case: artifact.artifact_case.clone(),
            entrypoint: artifact.entrypoint.clone(),
            runtime: WireArtifactIdentity::from(&artifact.runtime),
            declarations: WireArtifactIdentity::from(&artifact.declarations),
            closure: artifact.closure.as_str().into(),
            transform: artifact.transform.as_ref().map(WireArtifactIdentity::from),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProofClaim {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    artifact: WireClaimArtifactIdentity,
    producer: WireToolIdentity,
    fact_transcripts: Vec<WireFactTranscriptIdentity>,
    proof_inputs: Vec<WireProofInputIdentity>,
    coverage_limitations: Vec<String>,
}

impl WireProofClaim {
    fn new(
        claim_id: SemanticClaimId,
        expected: &CatalogClaim,
        material: ProofClaimMaterial,
    ) -> Self {
        Self {
            claim_id: claim_id.as_str().into(),
            subject: WireSemanticClaimSubject::from(&expected.subject),
            artifact: WireClaimArtifactIdentity::from(&expected.artifact),
            producer: WireToolIdentity::from(&material.producer),
            fact_transcripts: material
                .fact_transcripts
                .iter()
                .map(WireFactTranscriptIdentity::from)
                .collect(),
            proof_inputs: material
                .proof_inputs
                .iter()
                .map(WireProofInputIdentity::from)
                .collect(),
            coverage_limitations: material.coverage_limitations,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProbeClaim {
    claim_id: String,
    subject: WireSemanticClaimSubject,
    artifact: WireClaimArtifactIdentity,
    producer: WireToolIdentity,
    recipe: String,
    environment: WireEnvironmentIdentity,
    outcome: WireProbeOutcome,
    coverage_limitations: Vec<String>,
}

impl WireProbeClaim {
    fn new(
        claim_id: SemanticClaimId,
        expected: &CatalogClaim,
        material: ProbeClaimMaterial,
    ) -> Self {
        Self {
            claim_id: claim_id.as_str().into(),
            subject: WireSemanticClaimSubject::from(&expected.subject),
            artifact: WireClaimArtifactIdentity::from(&expected.artifact),
            producer: WireToolIdentity::from(&material.producer),
            recipe: material.recipe.as_str().into(),
            environment: WireEnvironmentIdentity::from(&material.environment),
            outcome: WireProbeOutcome::from(&material.outcome),
            coverage_limitations: material.coverage_limitations,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireToolIdentity {
    name: String,
    version: String,
    build: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
}

impl From<&ToolIdentity> for WireToolIdentity {
    fn from(tool: &ToolIdentity) -> Self {
        Self {
            name: tool.name.clone(),
            version: tool.version.clone(),
            build: tool.build.as_str().into(),
            protocol: tool.protocol.clone(),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireFactDomainIdentity {
    OxcSyntax,
    TypeFacts,
    CompilerExecutionFacts,
    AcceptedPackageContract,
}

impl From<FactDomainIdentity> for WireFactDomainIdentity {
    fn from(domain: FactDomainIdentity) -> Self {
        match domain {
            FactDomainIdentity::OxcSyntax => Self::OxcSyntax,
            FactDomainIdentity::TypeFacts => Self::TypeFacts,
            FactDomainIdentity::CompilerExecutionFacts => Self::CompilerExecutionFacts,
            FactDomainIdentity::AcceptedPackageContract => Self::AcceptedPackageContract,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireFactTranscriptIdentity {
    domain: WireFactDomainIdentity,
    transcript: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<u64>,
    producer: WireToolIdentity,
}

impl From<&FactTranscriptIdentity> for WireFactTranscriptIdentity {
    fn from(transcript: &FactTranscriptIdentity) -> Self {
        Self {
            domain: transcript.domain.into(),
            transcript: transcript.transcript.as_str().into(),
            generation: transcript.generation,
            producer: WireToolIdentity::from(&transcript.producer),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProofInputIdentity {
    rule: String,
    input: String,
    tool: WireToolIdentity,
}

impl From<&ProofInputIdentity> for WireProofInputIdentity {
    fn from(proof: &ProofInputIdentity) -> Self {
        Self {
            rule: proof.rule.clone(),
            input: proof.input.as_str().into(),
            tool: WireToolIdentity::from(&proof.tool),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEnvironmentIdentity {
    runtime: WireToolIdentity,
    os: String,
    architecture: String,
    conditions: Vec<String>,
    sandbox: WireSandboxIdentity,
}

impl From<&EnvironmentIdentity> for WireEnvironmentIdentity {
    fn from(environment: &EnvironmentIdentity) -> Self {
        Self {
            runtime: WireToolIdentity::from(&environment.runtime),
            os: environment.os.clone(),
            architecture: environment.architecture.clone(),
            conditions: environment.conditions.clone(),
            sandbox: WireSandboxIdentity::from(&environment.sandbox),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSandboxIdentity {
    kind: WireSandboxKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy: Option<String>,
}

impl From<&SandboxIdentity> for WireSandboxIdentity {
    fn from(sandbox: &SandboxIdentity) -> Self {
        Self {
            kind: sandbox.kind.into(),
            policy: sandbox.policy.as_ref().map(|digest| digest.as_str().into()),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSandboxKind {
    None,
    Process,
    Container,
    VirtualMachine,
}

impl From<SandboxKind> for WireSandboxKind {
    fn from(kind: SandboxKind) -> Self {
        match kind {
            SandboxKind::None => Self::None,
            SandboxKind::Process => Self::Process,
            SandboxKind::Container => Self::Container,
            SandboxKind::VirtualMachine => Self::VirtualMachine,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum WireProbeOutcome {
    Planned,
    Witness { transcript: String },
    Falsification { transcript: String },
    Error { details: String },
    Timeout { limit_millis: u64 },
    Refused { reason: String },
}

impl From<&ProbeOutcome> for WireProbeOutcome {
    fn from(outcome: &ProbeOutcome) -> Self {
        match outcome {
            ProbeOutcome::Planned => Self::Planned,
            ProbeOutcome::Witness { transcript } => Self::Witness {
                transcript: transcript.as_str().into(),
            },
            ProbeOutcome::Falsification { transcript } => Self::Falsification {
                transcript: transcript.as_str().into(),
            },
            ProbeOutcome::Error { details } => Self::Error {
                details: details.as_str().into(),
            },
            ProbeOutcome::Timeout { limit_millis } => Self::Timeout {
                limit_millis: *limit_millis,
            },
            ProbeOutcome::Refused { reason } => Self::Refused {
                reason: reason.clone(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireSemanticClaimSubject {
    artifact_case: String,
    export: String,
    path: WireSemanticClaimPath,
}

impl From<&SemanticClaimSubject> for WireSemanticClaimSubject {
    fn from(subject: &SemanticClaimSubject) -> Self {
        Self {
            artifact_case: subject.artifact_case.clone(),
            export: subject.export.clone(),
            path: WireSemanticClaimPath::from(&subject.path),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireSemanticClaimPath {
    Call {
        domain: WireClaimDomain,
    },
    Value {
        root: WireValueRoot,
        path: Vec<WireValuePathSegment>,
        domain: WireValueClaimDomain,
    },
    OperationAxis {
        operation: String,
        domain: WireOperationClaimDomain,
    },
    Resource {
        resource: String,
        domain: WireResourceClaimDomain,
    },
    GuardPartition,
    Operation {
        operation: String,
    },
}

impl From<&SemanticClaimPath> for WireSemanticClaimPath {
    fn from(path: &SemanticClaimPath) -> Self {
        match path {
            SemanticClaimPath::Domain(ClaimPath::Call(domain)) => Self::Call {
                domain: (*domain).into(),
            },
            SemanticClaimPath::Domain(ClaimPath::Value { root, path, domain }) => Self::Value {
                root: WireValueRoot::from(root),
                path: path.0.iter().map(WireValuePathSegment::from).collect(),
                domain: (*domain).into(),
            },
            SemanticClaimPath::Domain(ClaimPath::Operation { operation, domain }) => {
                Self::OperationAxis {
                    operation: operation.0.clone(),
                    domain: (*domain).into(),
                }
            }
            SemanticClaimPath::Domain(ClaimPath::Resource { resource, domain }) => Self::Resource {
                resource: resource.0.clone(),
                domain: (*domain).into(),
            },
            SemanticClaimPath::Domain(ClaimPath::GuardPartition) => Self::GuardPartition,
            SemanticClaimPath::Operation(operation) => Self::Operation {
                operation: operation.0.clone(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireValueRoot {
    Export,
    OperationInput { operation: String, index: u16 },
    OperationOutput { operation: String },
}

impl From<&ValueRoot> for WireValueRoot {
    fn from(root: &ValueRoot) -> Self {
        match root {
            ValueRoot::Export => Self::Export,
            ValueRoot::OperationInput { operation, index } => Self::OperationInput {
                operation: operation.0.clone(),
                index: *index,
            },
            ValueRoot::OperationOutput { operation } => Self::OperationOutput {
                operation: operation.0.clone(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireValuePathSegment {
    TupleItem { index: u32 },
    ArrayElement,
    ObjectProperty { name: String },
    ChoiceAlternative { index: u32 },
    PromiseValue,
    AsyncIterableElement,
}

impl From<&ValuePathSegment> for WireValuePathSegment {
    fn from(segment: &ValuePathSegment) -> Self {
        match segment {
            ValuePathSegment::TupleItem(index) => Self::TupleItem { index: *index },
            ValuePathSegment::ArrayElement => Self::ArrayElement,
            ValuePathSegment::ObjectProperty(name) => Self::ObjectProperty { name: name.clone() },
            ValuePathSegment::ChoiceAlternative(index) => Self::ChoiceAlternative { index: *index },
            ValuePathSegment::PromiseValue => Self::PromiseValue,
            ValuePathSegment::AsyncIterableElement => Self::AsyncIterableElement,
        }
    }
}

macro_rules! wire_enum {
    ($wire:ident, $semantic:ident, { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum $wire { $($variant),+ }

        impl From<$semantic> for $wire {
            fn from(value: $semantic) -> Self {
                match value { $($semantic::$variant => Self::$variant),+ }
            }
        }

        impl From<$wire> for $semantic {
            fn from(value: $wire) -> Self {
                match value { $($wire::$variant => Self::$variant),+ }
            }
        }
    };
}

wire_enum!(WireClaimDomain, ClaimDomain, {
    Callbacks,
    Reads,
    Writes,
    Creates,
    Invalidates,
    Throws,
    Returns,
    Cleanups,
    Disposals,
});
wire_enum!(WireValueClaimDomain, ValueClaimDomain, {
    Shape,
    TupleItems,
    ObjectProperties,
    ChoiceAlternatives,
    ArrayMinimumLength,
    ArrayMaximumLength,
    Capabilities,
});
wire_enum!(WireOperationClaimDomain, OperationClaimDomain, {
    Trigger,
    ExecutionPoint,
    Schedule,
    Tracking,
    OwnerSource,
    OwnerChildCapability,
    OwnerCleanupCapability,
    OwnerLifetime,
    OwnerProductions,
    CardinalityScope,
    CardinalityMinimum,
    CardinalityMaximum,
});
wire_enum!(WireResourceClaimDomain, ResourceClaimDomain, {
    States,
    Capabilities,
    Lifetime,
});

impl TryFrom<WireSemanticClaimSubject> for SemanticClaimSubject {
    type Error = EvidenceSidecarError;

    fn try_from(subject: WireSemanticClaimSubject) -> Result<Self, Self::Error> {
        validate_nonempty(&subject.artifact_case, "claim artifact case")?;
        validate_nonempty(&subject.export, "claim export")?;
        Ok(Self {
            artifact_case: subject.artifact_case,
            export: subject.export,
            path: subject.path.try_into()?,
        })
    }
}

impl TryFrom<WireSemanticClaimPath> for SemanticClaimPath {
    type Error = EvidenceSidecarError;

    fn try_from(path: WireSemanticClaimPath) -> Result<Self, Self::Error> {
        Ok(match path {
            WireSemanticClaimPath::Call { domain } => Self::Domain(ClaimPath::Call(domain.into())),
            WireSemanticClaimPath::Value { root, path, domain } => {
                if path.len() > MAX_ITEMS_PER_CLAIM {
                    return invalid_material("semantic value path exceeds the limit");
                }
                Self::Domain(ClaimPath::Value {
                    root: root.try_into()?,
                    path: ValuePath(
                        path.into_iter()
                            .map(ValuePathSegment::try_from)
                            .collect::<Result<_, _>>()?,
                    ),
                    domain: domain.into(),
                })
            }
            WireSemanticClaimPath::OperationAxis { operation, domain } => {
                validate_nonempty(&operation, "claim operation")?;
                Self::Domain(ClaimPath::Operation {
                    operation: OperationId(operation),
                    domain: domain.into(),
                })
            }
            WireSemanticClaimPath::Resource { resource, domain } => {
                validate_nonempty(&resource, "claim resource")?;
                Self::Domain(ClaimPath::Resource {
                    resource: ResourceId(resource),
                    domain: domain.into(),
                })
            }
            WireSemanticClaimPath::GuardPartition => Self::Domain(ClaimPath::GuardPartition),
            WireSemanticClaimPath::Operation { operation } => {
                validate_nonempty(&operation, "claim operation")?;
                Self::Operation(OperationId(operation))
            }
        })
    }
}

impl TryFrom<WireValueRoot> for ValueRoot {
    type Error = EvidenceSidecarError;

    fn try_from(root: WireValueRoot) -> Result<Self, Self::Error> {
        Ok(match root {
            WireValueRoot::Export => Self::Export,
            WireValueRoot::OperationInput { operation, index } => {
                validate_nonempty(&operation, "value-root operation")?;
                Self::OperationInput {
                    operation: OperationId(operation),
                    index,
                }
            }
            WireValueRoot::OperationOutput { operation } => {
                validate_nonempty(&operation, "value-root operation")?;
                Self::OperationOutput {
                    operation: OperationId(operation),
                }
            }
        })
    }
}

impl TryFrom<WireValuePathSegment> for ValuePathSegment {
    type Error = EvidenceSidecarError;

    fn try_from(segment: WireValuePathSegment) -> Result<Self, Self::Error> {
        Ok(match segment {
            WireValuePathSegment::TupleItem { index } => Self::TupleItem(index),
            WireValuePathSegment::ArrayElement => Self::ArrayElement,
            WireValuePathSegment::ObjectProperty { name } => {
                validate_nonempty(&name, "value-path property")?;
                Self::ObjectProperty(name)
            }
            WireValuePathSegment::ChoiceAlternative { index } => Self::ChoiceAlternative(index),
            WireValuePathSegment::PromiseValue => Self::PromiseValue,
            WireValuePathSegment::AsyncIterableElement => Self::AsyncIterableElement,
        })
    }
}

fn validate_proof_document(
    bytes: &[u8],
    catalog: &EvidenceCatalog,
) -> Result<Vec<SemanticClaimId>, EvidenceSidecarError> {
    validate_size(bytes)?;
    let document: WireProofDocument = serde_json::from_slice(bytes).map_err(decode_error)?;
    validate_document_header(
        "proof",
        PROOF_EVIDENCE_FORMAT,
        &document.format,
        document.sidecar_version,
        &document.contract,
        &document.producer,
        catalog,
    )?;
    if document.claims.len() > MAX_CLAIMS {
        return invalid_material("proof claim count exceeds the limit");
    }
    let mut claims = BTreeSet::new();
    for claim in document.claims {
        let claim_id = validate_claim_common(
            "proof",
            claim.claim_id,
            claim.subject,
            claim.artifact,
            &catalog.proof_claims,
            catalog,
        )?;
        validate_wire_tool(&claim.producer)?;
        if claim.fact_transcripts.len() > MAX_ITEMS_PER_CLAIM
            || claim.proof_inputs.len() > MAX_ITEMS_PER_CLAIM
            || claim.coverage_limitations.len() > MAX_ITEMS_PER_CLAIM
        {
            return invalid_material("proof material count exceeds the per-claim limit");
        }
        let has_material = !claim.fact_transcripts.is_empty()
            || !claim.proof_inputs.is_empty()
            || !claim.coverage_limitations.is_empty();
        for transcript in claim.fact_transcripts {
            parse_digest(&transcript.transcript)?;
            validate_wire_tool(&transcript.producer)?;
        }
        for proof in claim.proof_inputs {
            validate_nonempty(&proof.rule, "proof rule")?;
            parse_digest(&proof.input)?;
            validate_wire_tool(&proof.tool)?;
        }
        validate_wire_strings(&claim.coverage_limitations, "coverage limitation")?;
        if !has_material {
            return invalid_material("proof claim has no evidence or coverage limitation");
        }
        if !claims.insert(claim_id.clone()) {
            return Err(EvidenceSidecarError::DuplicateClaim {
                claim_id: claim_id.as_str().into(),
            });
        }
    }
    Ok(claims.into_iter().collect())
}

fn validate_probe_document(
    bytes: &[u8],
    catalog: &EvidenceCatalog,
) -> Result<Vec<SemanticClaimId>, EvidenceSidecarError> {
    validate_size(bytes)?;
    let document: WireProbeDocument = serde_json::from_slice(bytes).map_err(decode_error)?;
    validate_document_header(
        "runtime-probe",
        PROBE_EVIDENCE_FORMAT,
        &document.format,
        document.sidecar_version,
        &document.contract,
        &document.producer,
        catalog,
    )?;
    if document.claims.len() > MAX_CLAIMS {
        return invalid_material("probe claim count exceeds the limit");
    }
    let mut claims = BTreeSet::new();
    for claim in document.claims {
        let claim_id = validate_claim_common(
            "runtime-probe",
            claim.claim_id,
            claim.subject,
            claim.artifact,
            &catalog.probe_claims,
            catalog,
        )?;
        validate_wire_tool(&claim.producer)?;
        parse_digest(&claim.recipe)?;
        validate_wire_environment(&claim.environment)?;
        validate_wire_outcome(&claim.outcome)?;
        validate_wire_strings(&claim.coverage_limitations, "coverage limitation")?;
        if !claims.insert(claim_id.clone()) {
            return Err(EvidenceSidecarError::DuplicateClaim {
                claim_id: claim_id.as_str().into(),
            });
        }
    }
    Ok(claims.into_iter().collect())
}

fn validate_document_header(
    kind: &'static str,
    expected_format: &'static str,
    actual_format: &str,
    version: u16,
    identity: &WireContractIdentity,
    producer: &WireToolIdentity,
    catalog: &EvidenceCatalog,
) -> Result<(), EvidenceSidecarError> {
    if actual_format != expected_format {
        return Err(EvidenceSidecarError::DocumentKind {
            expected: expected_format,
            actual: actual_format.into(),
        });
    }
    if version != EVIDENCE_SIDECAR_VERSION {
        return Err(EvidenceSidecarError::Version {
            expected: EVIDENCE_SIDECAR_VERSION,
            actual: version,
        });
    }
    validate_wire_tool(producer)?;
    if !wire_contract_matches(identity, catalog.contract()) {
        return Err(EvidenceSidecarError::ContractBindingMismatch { kind });
    }
    Ok(())
}

fn wire_contract_matches(identity: &WireContractIdentity, contract: &NormalizedContract) -> bool {
    identity.semantic_model_version == contract.semantic_model_version()
        && identity.semantic_digest == contract.semantic_digest().as_str()
        && identity.package.name == contract.package().name
        && identity.package.version == contract.package().version
        && identity.package.integrity == contract.package().integrity
        && wire_artifact_matches(&identity.package.manifest, &contract.package().manifest)
}

fn validate_claim_common(
    kind: &'static str,
    claim_id: String,
    subject: WireSemanticClaimSubject,
    artifact: WireClaimArtifactIdentity,
    expected_claims: &BTreeMap<SemanticClaimId, CatalogClaim>,
    catalog: &EvidenceCatalog,
) -> Result<SemanticClaimId, EvidenceSidecarError> {
    let encoded_id = SemanticClaimId::parse(claim_id)?;
    let subject = SemanticClaimSubject::try_from(subject)?;
    let derived_id = catalog.contract.claim_id(&subject)?;
    if encoded_id != derived_id {
        return Err(EvidenceSidecarError::ClaimIdMismatch);
    }
    let expected = expected_claims
        .get(&derived_id)
        .ok_or(EvidenceSidecarError::OrphanClaim { kind })?;
    if expected.subject != subject || !wire_claim_artifact_matches(&artifact, &expected.artifact) {
        return Err(EvidenceSidecarError::ArtifactMismatch);
    }
    Ok(derived_id)
}

fn wire_claim_artifact_matches(
    actual: &WireClaimArtifactIdentity,
    expected: &ArtifactEvidenceIdentity,
) -> bool {
    actual.artifact_case == expected.artifact_case
        && actual.entrypoint == expected.entrypoint
        && wire_artifact_matches(&actual.runtime, &expected.runtime)
        && wire_artifact_matches(&actual.declarations, &expected.declarations)
        && actual.closure == expected.closure.as_str()
        && match (&actual.transform, &expected.transform) {
            (None, None) => true,
            (Some(actual), Some(expected)) => wire_artifact_matches(actual, expected),
            _ => false,
        }
}

fn wire_artifact_matches(actual: &WireArtifactIdentity, expected: &ArtifactIdentity) -> bool {
    actual.path == expected.path && actual.digest == expected.digest.as_str()
}

fn validate_wire_tool(tool: &WireToolIdentity) -> Result<(), EvidenceSidecarError> {
    validate_nonempty(&tool.name, "tool name")?;
    validate_nonempty(&tool.version, "tool version")?;
    parse_digest(&tool.build)?;
    if let Some(protocol) = &tool.protocol {
        validate_nonempty(protocol, "tool protocol")?;
    }
    Ok(())
}

fn validate_wire_environment(
    environment: &WireEnvironmentIdentity,
) -> Result<(), EvidenceSidecarError> {
    validate_wire_tool(&environment.runtime)?;
    validate_nonempty(&environment.os, "environment operating system")?;
    validate_nonempty(&environment.architecture, "environment architecture")?;
    validate_wire_strings(&environment.conditions, "environment condition")?;
    match (&environment.sandbox.kind, &environment.sandbox.policy) {
        (WireSandboxKind::None, None) => {}
        (WireSandboxKind::None, Some(_)) => {
            return invalid_material("an unsandboxed environment cannot name a sandbox policy");
        }
        (_, Some(policy)) => {
            parse_digest(policy)?;
        }
        (_, None) => return invalid_material("a sandboxed environment requires a policy digest"),
    }
    Ok(())
}

fn validate_wire_outcome(outcome: &WireProbeOutcome) -> Result<(), EvidenceSidecarError> {
    match outcome {
        WireProbeOutcome::Planned => Ok(()),
        WireProbeOutcome::Witness { transcript }
        | WireProbeOutcome::Falsification { transcript } => parse_digest(transcript).map(drop),
        WireProbeOutcome::Error { details } => parse_digest(details).map(drop),
        WireProbeOutcome::Timeout { limit_millis } if *limit_millis > 0 => Ok(()),
        WireProbeOutcome::Timeout { .. } => invalid_material("probe timeout must be non-zero"),
        WireProbeOutcome::Refused { reason } => validate_nonempty(reason, "probe refusal reason"),
    }
}

fn validate_wire_strings(values: &[String], field: &str) -> Result<(), EvidenceSidecarError> {
    if values.len() > MAX_ITEMS_PER_CLAIM {
        return invalid_material(format!("{field} count exceeds the limit"));
    }
    for value in values {
        validate_nonempty(value, field)?;
    }
    if values.windows(2).any(|window| window[0] >= window[1]) {
        return invalid_material(format!("{field} values must be sorted and unique"));
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<Digest, EvidenceSidecarError> {
    let digest = Digest::parse(value).map_err(|_| EvidenceSidecarError::InvalidMaterial {
        reason: "digest is not canonical SHA-256".into(),
    })?;
    if digest.as_str() != value {
        return invalid_material("digest is not canonical lowercase SHA-256");
    }
    Ok(digest)
}

fn validate_size(bytes: &[u8]) -> Result<(), EvidenceSidecarError> {
    if bytes.len() > MAX_SIDECAR_BYTES {
        Err(EvidenceSidecarError::DocumentTooLarge {
            limit: MAX_SIDECAR_BYTES,
        })
    } else {
        Ok(())
    }
}

fn decode_error(error: serde_json::Error) -> EvidenceSidecarError {
    EvidenceSidecarError::Decode {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
