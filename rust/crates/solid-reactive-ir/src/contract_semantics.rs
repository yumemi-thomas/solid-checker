//! Wire-independent package behavior at the analyzer trust seam.
//!
//! Compact JSON concepts such as summary names, `closed` arrays, aliases,
//! omission rules, and schema versions deliberately do not appear here. The
//! backend expands those mechanics and submits only semantic concepts to
//! [`ContractProposal::normalize`]. Validation, guard selection, recursive
//! uncertainty, and canonical semantic identity stay inside this deep module.

mod canonical;
pub mod certification;
mod consumer;
mod guards;
pub mod proof;
pub mod solid2_rc3;
mod validate;

pub use consumer::{
    AcceptedContractIndex, AcceptedContractInput, AcceptedContractUse, AcceptedImportIdentity,
    AcceptedSemanticIdentity, CallSiteFacts, FiniteFact, InstantiatedClaim, InstantiatedExport,
    OpenDomainDiagnostic, OpenDomainReason, PropertyFact, SemanticQueryError,
    UncertifiableImportReason, native_claim_precedence,
};

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

pub const SEMANTIC_MODEL_VERSION: u16 = 1;
/// Hash family frozen for semantic-model version 1.
pub const SEMANTIC_DIGEST_ALGORITHM: &str = "sha256";
/// Domain separator frozen for semantic-model version 1 contract identities.
pub const SEMANTIC_DIGEST_DOMAIN: &str = "solid-checker:normalized-package-contract";
/// The digest domain for a contract in which at least one operation states
/// composed provenance.
///
/// A second domain rather than a second model version, because the model is
/// unchanged: `composedFrom` is an additive optional field, every document
/// that omits it still validates, and `semanticModelVersion` stays 1. What
/// needs separating is the *byte stream*, so that a contract with no composed
/// operation keeps hashing exactly what it hashed before the field existed —
/// and with it every policy-2 receipt already issued for it. A contract that
/// does carry provenance is a new document making a new claim, and it gets a
/// digest in its own family.
pub const SEMANTIC_DIGEST_DOMAIN_COMPOSED: &str =
    "solid-checker:normalized-package-contract:composed-provenance";
/// The digest domain for a contract in which at least one export proposes a
/// call domain for closure proof (`CallSemantics::proposed_closures`).
///
/// The same reasoning as `SEMANTIC_DIGEST_DOMAIN_COMPOSED`, one field later,
/// and the two features are independent: a contract may carry either, both, or
/// neither, so there are four domains and not three. Each is a distinct
/// length-prefixed first write, so the families cannot collide, and every
/// contract that proposes nothing keeps hashing exactly what it hashed before
/// the marker existed.
pub const SEMANTIC_DIGEST_DOMAIN_PROPOSED_CLOSURE: &str =
    "solid-checker:normalized-package-contract:proposed-closure";
/// The digest domain for a contract carrying composed provenance *and* a
/// proposed closure.
pub const SEMANTIC_DIGEST_DOMAIN_COMPOSED_PROPOSED_CLOSURE: &str =
    "solid-checker:normalized-package-contract:composed-provenance:proposed-closure";
/// The length-prefixed marker a semantic digest or a recipe address writes
/// first when some operation it encodes states a non-call
/// [`InvokeProtocol`]. A stream with no such operation never writes it, so it
/// hashes exactly as it did before the protocol existed.
pub const SEMANTIC_INVOKE_PROTOCOL_MARKER: &str = "solid-checker:semantic-invoke-protocol:v1";
/// The length-prefixed marker a semantic digest or a recipe address writes
/// first when some operation it encodes happens at [`Event::ResultAccess`]
/// (ADR 0139). A stream with no such operation never writes it, so it hashes
/// exactly as it did before the event existed.
pub const SEMANTIC_RESULT_ACCESS_MARKER: &str = "solid-checker:semantic-result-access:v1";
/// The length-prefixed marker a semantic digest writes first when some export
/// it encodes states a [`ContextPremise`] (ADR 0153 part 3). A contract with
/// none never writes it, so it hashes exactly as it did before premises
/// existed.
pub const SEMANTIC_CONTEXT_PREMISES_MARKER: &str = "solid-checker:semantic-context-premises:v1";
/// The length-prefixed marker a semantic digest writes first when some export
/// it encodes states an accessor-installation bound (ADR 0153 item C). A
/// contract with none never writes it, so it hashes exactly as it did before
/// bounds existed.
pub const SEMANTIC_ACCESSOR_BOUNDS_MARKER: &str = "solid-checker:semantic-accessor-bounds:v1";
pub const SEMANTIC_CLAIM_ID_VERSION: u16 = 1;
/// Version of the byte-only artifact-case identity a [`RecipeAddress`] binds.
pub const ARTIFACT_CASE_BYTES_VERSION: u16 = 1;
/// Version of the [`RecipeAddress`] stream.
pub const RECIPE_ADDRESS_VERSION: u16 = 1;

/// Local knowledge for one immediate collection-valued claim domain.
///
/// The four semantic states are represented without a redundant enum case:
/// `Unknown`, non-empty `Partial`, non-empty `Complete`, and empty `Complete`.
/// An open empty collection is invalid and is rejected by normalization.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum KnowledgeSet<T> {
    #[default]
    Unknown,
    Partial(Vec<T>),
    Complete(Vec<T>),
}

impl<T> KnowledgeSet<T> {
    #[must_use]
    pub const fn unknown() -> Self {
        Self::Unknown
    }

    #[must_use]
    pub fn partial(items: Vec<T>) -> Option<Self> {
        (!items.is_empty()).then_some(Self::Partial(items))
    }

    #[must_use]
    pub const fn complete(items: Vec<T>) -> Self {
        Self::Complete(items)
    }

    #[must_use]
    pub fn items(&self) -> &[T] {
        match self {
            Self::Unknown => &[],
            Self::Partial(items) | Self::Complete(items) => items,
        }
    }

    #[must_use]
    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    #[must_use]
    pub fn proves_absence(&self) -> bool {
        matches!(self, Self::Complete(items) if items.is_empty())
    }

    #[must_use]
    pub fn state(&self) -> KnowledgeState {
        match self {
            Self::Unknown => KnowledgeState::Unknown,
            Self::Partial(_) => KnowledgeState::PartialPositive,
            Self::Complete(items) if items.is_empty() => KnowledgeState::CompleteNegative,
            Self::Complete(_) => KnowledgeState::CompletePositive,
        }
    }

    fn into_items(self) -> Vec<T> {
        match self {
            Self::Unknown => Vec::new(),
            Self::Partial(items) | Self::Complete(items) => items,
        }
    }

    fn items_mut(&mut self) -> &mut [T] {
        match self {
            Self::Unknown => &mut [],
            Self::Partial(items) | Self::Complete(items) => items,
        }
    }
}

impl<T: Ord> KnowledgeSet<T> {
    fn close_verified(&mut self) -> bool {
        if self.is_closed() {
            return false;
        }
        let items = std::mem::take(self).into_items();
        *self = Self::Complete(items);
        true
    }

    fn open_proposed_closure(&mut self) -> bool {
        if !self.is_closed() {
            return false;
        }
        *self = std::mem::take(self).weaken();
        true
    }

    /// Monotonically joins all possible alternatives.
    ///
    /// Positive items are unioned. Closure survives only when every possible
    /// alternative is complete; therefore one unresolved alternative can
    /// retract a negative proof but cannot erase a known positive sibling.
    #[must_use]
    pub fn join(alternatives: impl IntoIterator<Item = Self>) -> Self {
        let mut saw_alternative = false;
        let mut all_complete = true;
        let mut items = BTreeSet::new();
        for alternative in alternatives {
            saw_alternative = true;
            all_complete &= alternative.is_closed();
            items.extend(alternative.into_items());
        }
        if !saw_alternative {
            return Self::Unknown;
        }
        let items = items.into_iter().collect::<Vec<_>>();
        match (all_complete, items.is_empty()) {
            (true, _) => Self::Complete(items),
            (false, true) => Self::Unknown,
            (false, false) => Self::Partial(items),
        }
    }

    fn weaken(self) -> Self {
        match self {
            Self::Complete(items) if items.is_empty() => Self::Unknown,
            Self::Complete(items) => Self::Partial(items),
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KnowledgeState {
    Unknown,
    PartialPositive,
    CompletePositive,
    CompleteNegative,
}

impl KnowledgeState {
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Unknown | Self::PartialPositive)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Digest(String);

impl Digest {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        let payload = value.strip_prefix("sha256:").ok_or(ModelError::Digest)?;
        if payload.len() != 64 || !payload.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ModelError::Digest);
        }
        Ok(Self(format!("sha256:{}", payload.to_ascii_lowercase())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_sha256(bytes: [u8; 32]) -> Self {
        let mut value = String::with_capacity(71);
        value.push_str("sha256:");
        for byte in bytes {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to a String cannot fail");
        }
        Self(value)
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error("digest must be sha256 followed by exactly 64 hexadecimal digits")]
    Digest,
    #[error("semantic model version {actual} is unsupported; expected {expected}")]
    SemanticModelVersion { expected: u16, actual: u16 },
    #[error("{field} must not be empty")]
    EmptyIdentity { field: String },
    #[error("duplicate {kind} identity {id}")]
    DuplicateIdentity { kind: &'static str, id: String },
    #[error("invalid local knowledge at {path}: {reason}")]
    InvalidKnowledge { path: String, reason: String },
    #[error("contradictory semantic claims at {path}: {reason}")]
    Contradiction { path: String, reason: String },
    #[error("{path} references missing operation {operation}")]
    MissingOperation { path: String, operation: String },
    #[error("{path} references missing resource {resource}")]
    MissingResource { path: String, resource: String },
    #[error("operation graph contains a causal cycle involving {operation}")]
    OperationCycle { operation: String },
    #[error("resource lifetime graph contains a cycle involving {resource}")]
    ResourceCycle { resource: String },
    #[error("invalid guard at {path}: {reason}")]
    InvalidGuard { path: String, reason: String },
    #[error("guards {left} and {right} overlap in partition {path}")]
    OverlappingGuards {
        path: String,
        left: usize,
        right: usize,
    },
    #[error("export {export} does not have exact identity for artifact case {case}")]
    ExportIdentity { case: String, export: String },
    #[error("artifact case selection identity is duplicated by {first} and {second}")]
    DuplicateArtifactSelection { first: String, second: String },
    #[error("selected artifact case index {selected} does not exist")]
    MissingArtifactCase { selected: usize },
    #[error("no recipe address: {reason}")]
    Unaddressable { reason: String },
    #[error(
        "recipe address must be canonical recipe-address:v1:sha256 followed by 64 lowercase hexadecimal digits"
    )]
    RecipeAddressFormat,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactIdentity {
    pub path: String,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    pub integrity: String,
    pub manifest: ArtifactIdentity,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResolutionStep {
    pub condition: String,
    pub target: String,
}

/// Exact runtime or declaration binding selected for one public export.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExportTargetIdentity {
    pub module: ArtifactIdentity,
    pub export_name: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExportIdentity {
    pub entrypoint: String,
    pub public_name: String,
    pub runtime: ExportTargetIdentity,
    pub declarations: ExportTargetIdentity,
}

/// An explicit proposal about evaluation of the selected runtime module.
/// Absence is not a claim; only authenticated proof can make this knowledge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModuleInitializationClaim {
    Inert,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCase {
    pub initialization: Option<ModuleInitializationClaim>,
    pub id: String,
    pub entrypoint: String,
    pub resolution_trace: Vec<ResolutionStep>,
    pub runtime: ArtifactIdentity,
    pub declarations: ArtifactIdentity,
    pub dependency_closure: Digest,
    pub transform: Option<ArtifactIdentity>,
    pub stability: StabilityKnowledge,
    pub exports: BTreeMap<String, ExportSemantics>,
}

/// Unaccepted semantic candidates. This typestate may contain proposed local
/// closure, but it cannot construct [`AcceptedContract`]. Later proof replay
/// must independently authorize every closed claim and receipt binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractProposal {
    semantic_model_version: u16,
    package: PackageIdentity,
    artifact_cases: Vec<ArtifactCase>,
}

impl ContractProposal {
    #[must_use]
    pub fn new(package: PackageIdentity, artifact_cases: Vec<ArtifactCase>) -> Self {
        Self {
            semantic_model_version: SEMANTIC_MODEL_VERSION,
            package,
            artifact_cases,
        }
    }

    #[must_use]
    pub const fn semantic_model_version(&self) -> u16 {
        self.semantic_model_version
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub fn artifact_cases(&self) -> &[ArtifactCase] {
        &self.artifact_cases
    }

    /// Validates every cross-reference and contradiction, canonicalizes every
    /// semantically unordered collection, and computes semantic identity.
    pub fn normalize(self) -> Result<NormalizedContract, ModelError> {
        validate::normalize(self)
    }
}

/// Canonical wire-independent meaning. All fields are private so callers must
/// use semantic queries rather than reconstructing schema mechanics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedContract {
    semantic_model_version: u16,
    package: PackageIdentity,
    artifact_cases: Vec<ArtifactCase>,
    semantic_digest: Digest,
}

impl NormalizedContract {
    #[must_use]
    pub const fn semantic_model_version(&self) -> u16 {
        self.semantic_model_version
    }

    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub fn artifact_cases(&self) -> &[ArtifactCase] {
        &self.artifact_cases
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> &Digest {
        &self.semantic_digest
    }

    #[must_use]
    pub fn artifact_case(&self, id: &str) -> Option<&ArtifactCase> {
        self.artifact_cases.iter().find(|case| case.id == id)
    }

    /// Computes the stable identity of one addressable semantic claim.
    ///
    /// The digest includes exact package, artifact-case, and export identity
    /// plus the normalized semantic path. It excludes wire positions, summary
    /// names, formatting, sidecar layout, and unrelated claim values.
    pub fn claim_id(
        &self,
        subject: &SemanticClaimSubject,
    ) -> Result<SemanticClaimId, ClaimIdentityError> {
        let artifact_case = self.artifact_case(&subject.artifact_case).ok_or_else(|| {
            ClaimIdentityError::MissingArtifactCase {
                artifact_case: subject.artifact_case.clone(),
            }
        })?;
        let export = artifact_case.exports.get(&subject.export).ok_or_else(|| {
            ClaimIdentityError::MissingExport {
                artifact_case: subject.artifact_case.clone(),
                export: subject.export.clone(),
            }
        })?;
        if !validate::claim_subject_exists(export, &subject.path) {
            return Err(ClaimIdentityError::InvalidSubject {
                artifact_case: subject.artifact_case.clone(),
                export: subject.export.clone(),
            });
        }
        Ok(canonical::semantic_claim_id(
            &self.package,
            artifact_case,
            export,
            &subject.path,
        ))
    }

    /// The byte-only identity of one artifact case (ways-to-improve § 3.2):
    /// package bytes, entrypoint, resolution trace, runtime, declarations and
    /// transform artifacts, and `closure_bytes`, the caller's byte-only
    /// identity of the case's dependency closure. It deliberately omits the
    /// case id and `dependency_closure`, both of which hash every accepted
    /// dependency edge's contract digest.
    ///
    /// It is an input to [`Self::recipe_address`] and nothing else: it is not
    /// a claim identity and authenticates nothing.
    pub fn artifact_case_byte_identity(
        &self,
        artifact_case: &str,
        closure_bytes: &str,
    ) -> Result<Digest, ModelError> {
        let case = self
            .artifact_case(artifact_case)
            .ok_or_else(|| ModelError::Unaddressable {
                reason: format!("the contract has no artifact case {artifact_case}"),
            })?;
        Ok(canonical::artifact_case_byte_identity(
            &self.package,
            case,
            closure_bytes,
        ))
    }

    /// The second address of a probe recipe: `case_bytes` (from
    /// [`Self::artifact_case_byte_identity`]), the export identity, the claim
    /// path, and the claim's normalized value, with every artifact-case prefix
    /// removed from the ids it writes.
    ///
    /// A recipe corpus may bind an entry by this address when its claim id no
    /// longer names a plan claim. The address decides only which claim a
    /// recipe module is launched for; it is never authority. Only call-domain
    /// and operation subjects have one.
    pub fn recipe_address(
        &self,
        subject: &SemanticClaimSubject,
        case_bytes: &Digest,
    ) -> Result<RecipeAddress, ModelError> {
        let artifact_case = self.artifact_case(&subject.artifact_case).ok_or_else(|| {
            ModelError::Unaddressable {
                reason: format!(
                    "the contract has no artifact case {}",
                    subject.artifact_case
                ),
            }
        })?;
        let export = artifact_case.exports.get(&subject.export).ok_or_else(|| {
            ModelError::Unaddressable {
                reason: format!(
                    "artifact case {} has no export {}",
                    subject.artifact_case, subject.export
                ),
            }
        })?;
        if !validate::claim_subject_exists(export, &subject.path) {
            return Err(ModelError::Unaddressable {
                reason: format!(
                    "the subject does not exist for export {} in artifact case {}",
                    subject.export, subject.artifact_case
                ),
            });
        }
        canonical::recipe_address(artifact_case, export, &subject.path, case_bytes)
    }

    /// Answers whether one exact semantic claim is closed in this contract.
    ///
    /// Claim identity binds the package, artifact case, export, and semantic
    /// path. Callers must not substitute a matching domain name or receipt
    /// digest for this lookup.
    #[must_use]
    pub fn contains_closed_claim_id(&self, value: &str) -> bool {
        let Ok(expected) = SemanticClaimId::parse(value.to_owned()) else {
            return false;
        };
        self.artifact_cases.iter().any(|artifact_case| {
            artifact_case.exports.iter().any(|(export_name, export)| {
                validate::closed_claims(export).into_iter().any(|path| {
                    let subject = SemanticClaimSubject {
                        artifact_case: artifact_case.id.clone(),
                        export: export_name.clone(),
                        path: SemanticClaimPath::Domain(path),
                    };
                    self.claim_id(&subject).is_ok_and(|claim| claim == expected)
                })
            })
        })
    }
}

/// A semantic proposition address, independent of compact-wire layout.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticClaimSubject {
    pub artifact_case: String,
    pub export: String,
    pub path: SemanticClaimPath,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticClaimPath {
    Domain(ClaimPath),
    /// Positive existence of one normalized operation. Its axes have their
    /// own [`ClaimPath::Operation`] subjects.
    Operation(OperationId),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticClaimId(String);

impl SemanticClaimId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ClaimIdentityError> {
        let value = value.into();
        let digest = value
            .strip_prefix("claim:v1:")
            .ok_or(ClaimIdentityError::InvalidId)?;
        let parsed = Digest::parse(digest).map_err(|_| ClaimIdentityError::InvalidId)?;
        if parsed.as_str() != digest {
            return Err(ClaimIdentityError::InvalidId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(format!(
            "claim:v{SEMANTIC_CLAIM_ID_VERSION}:{}",
            Digest::from_sha256(bytes).as_str()
        ))
    }
}

/// A recipe's byte-only second address; see
/// [`NormalizedContract::recipe_address`]. Formatted
/// `recipe-address:v1:sha256:<64 lowercase hex>`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RecipeAddress(String);

impl RecipeAddress {
    pub fn parse(value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        let digest = value
            .strip_prefix("recipe-address:v1:")
            .ok_or(ModelError::RecipeAddressFormat)?;
        let parsed = Digest::parse(digest).map_err(|_| ModelError::RecipeAddressFormat)?;
        if parsed.as_str() != digest {
            return Err(ModelError::RecipeAddressFormat);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_sha256(bytes: [u8; 32]) -> Self {
        Self(format!(
            "recipe-address:v{RECIPE_ADDRESS_VERSION}:{}",
            Digest::from_sha256(bytes).as_str()
        ))
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ClaimIdentityError {
    #[error(
        "semantic claim ID must be canonical claim:v1:sha256 followed by 64 lowercase hexadecimal digits"
    )]
    InvalidId,
    #[error("semantic claim names missing artifact case {artifact_case}")]
    MissingArtifactCase { artifact_case: String },
    #[error("semantic claim names missing export {export} in artifact case {artifact_case}")]
    MissingExport {
        artifact_case: String,
        export: String,
    },
    #[error(
        "semantic claim subject does not exist for export {export} in artifact case {artifact_case}"
    )]
    InvalidSubject {
        artifact_case: String,
        export: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceReference {
    pub claim: SemanticClaimId,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceBundle {
    pub semantic_digest: Digest,
    pub static_proofs: Vec<EvidenceReference>,
    pub probe_observations: Vec<EvidenceReference>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VerifierIdentity {
    pub build: String,
    pub policy: u32,
}

/// Authentication identity retained by policy-2 accepted typestate. The
/// receipt digest binds every signed certification root; trust-store identity
/// and revocation epoch prevent cache reuse across a trust-policy change.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReceiptAuthenticationIdentity {
    pub receipt_digest: Digest,
    pub policy_digest: Digest,
    pub trust_store_digest: Digest,
    pub revocation_epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceReceipt {
    pub receipt_version: u16,
    pub wire_digest: Digest,
    pub semantic_model_version: u16,
    pub semantic_digest: Digest,
    pub artifacts_digest: Digest,
    pub closure_digest: Digest,
    pub proof_root: Digest,
    pub closed_claims_root: Digest,
    pub verifier: VerifierIdentity,
    pub authentication: Option<ReceiptAuthenticationIdentity>,
}

/// Accepted typestate. It intentionally exposes no constructor: only
/// [`proof::verify_and_accept`] can manufacture verified local closure and its
/// receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContract {
    package: PackageIdentity,
    selected_case: ArtifactCase,
    receipt: AcceptanceReceipt,
}

impl AcceptedContract {
    #[must_use]
    pub const fn package(&self) -> &PackageIdentity {
        &self.package
    }

    #[must_use]
    pub const fn artifact_case(&self) -> &ArtifactCase {
        &self.selected_case
    }

    #[must_use]
    pub const fn receipt(&self) -> &AcceptanceReceipt {
        &self.receipt
    }

    /// Complete cache identity for analyzer-visible meaning. Receipt policy
    /// and verifier build are identity, not ambient configuration, so a policy
    /// change cannot reuse a program built from an older acceptance decision.
    #[must_use]
    pub fn semantic_identity(&self) -> AcceptedSemanticIdentity {
        AcceptedSemanticIdentity {
            package: self.package.clone(),
            artifact_case: self.selected_case.id.clone(),
            receipt_version: self.receipt.receipt_version,
            semantic_model_version: self.receipt.semantic_model_version,
            semantic_digest: self.receipt.semantic_digest.clone(),
            artifacts_digest: self.receipt.artifacts_digest.clone(),
            closure_digest: self.receipt.closure_digest.clone(),
            proof_root: self.receipt.proof_root.clone(),
            closed_claims_root: self.receipt.closed_claims_root.clone(),
            verifier: self.receipt.verifier.clone(),
            authentication: self.receipt.authentication.clone(),
        }
    }

    #[must_use]
    pub fn export(&self, name: &str) -> Option<&ExportSemantics> {
        self.selected_case.exports.get(name)
    }

    /// Resolves an effective export only through its exact runtime and
    /// declaration identity. A public spelling alone is insufficient at this
    /// trust seam because reexports and conditional artifacts may bind it to a
    /// different implementation.
    pub fn resolve_export(
        &self,
        identity: &ExportIdentity,
    ) -> Result<&ExportSemantics, SemanticQueryError> {
        consumer::resolve_export(self, identity)
    }

    /// Resolves and instantiates guarded behavior for one exact call site.
    pub fn instantiate_export<'contract, 'facts>(
        &'contract self,
        identity: &ExportIdentity,
        facts: &'facts CallSiteFacts,
    ) -> Result<InstantiatedExport<'contract, 'facts>, SemanticQueryError> {
        let export = self.resolve_export(identity)?;
        consumer::instantiate_export(&self.selected_case.id, export, facts)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExportSemantics {
    pub identity: ExportIdentity,
    pub shape: ValueShape,
    pub stability: StabilityKnowledge,
    pub call: CallSemantics,
}

impl ExportSemantics {
    #[must_use]
    pub fn claim_state(&self, domain: ClaimDomain) -> KnowledgeState {
        self.call.claim_state(domain)
    }

    #[must_use]
    pub const fn callbacks(&self) -> &KnowledgeSet<CallbackInvocation> {
        &self.call.claims.callbacks
    }

    #[must_use]
    pub fn operation_claim(&self, domain: ClaimDomain) -> Option<&KnowledgeSet<OperationId>> {
        self.call.claims.operation_claim(domain)
    }

    #[must_use]
    pub fn operation(&self, id: &str) -> Option<&Operation> {
        self.call
            .operations
            .iter()
            .find(|operation| operation.id.0 == id)
    }

    /// Traverses every local call, operation-axis, resource, and recursive
    /// value claim without widening an open leaf to a known sibling.
    #[must_use]
    pub fn unresolved_claims(&self) -> Vec<ClaimPath> {
        validate::unresolved_claims(self)
    }

    /// Withdraws every locally proposed completeness claim and returns the
    /// exact semantic leaves whose closure must be proved later.
    ///
    /// Positive items survive as partial knowledge. Complete-negative leaves
    /// become unknown. This is the proposal-generator boundary: constructing
    /// a proposal can retain candidates for later proof planning, but cannot
    /// publish any of them as accepted closure.
    pub fn open_proposed_closure(&mut self) -> Vec<ClaimPath> {
        validate::open_proposed_closure(self)
    }

    /// Republishes the named call domains as *proposed* closures: the domain
    /// closed over the positive items it already carries, and labelled as the
    /// generator's proposal rather than a reviewed claim.
    ///
    /// The inverse of the weakening [`Self::open_proposed_closure`] performs,
    /// used by the proposal generator on exactly the domains a certifier has a
    /// census for. See [`CallSemantics::proposed_closures`].
    pub fn propose_closures(&mut self, domains: impl IntoIterator<Item = ClaimDomain>) {
        for domain in domains {
            match self.call.claims.operation_claim_mut(domain) {
                Some(claim) => claim.close_verified(),
                None => self.call.claims.callbacks.close_verified(),
            };
            self.call.proposed_closures.insert(domain);
        }
    }

    /// Restates an unaccepted fixed-return enumeration as a proposal. The
    /// certifier withdraws it again and independently proves every member.
    /// This cannot propose capability or unrelated call-domain closure.
    pub fn propose_return_value_closure(&mut self, claim: &ClaimPath) -> Result<(), ModelError> {
        let supported = matches!(claim, ClaimPath::Value {
            root: ValueRoot::OperationOutput { operation },
            domain: ValueClaimDomain::TupleItems | ValueClaimDomain::ObjectProperties,
            ..
        } if self.operation(&operation.0).is_some_and(Operation::is_bare_return));
        if !supported {
            return Err(ModelError::InvalidKnowledge {
                path: format!("{claim:?}"),
                reason: "only a fixed return's member enumeration can be proposed here".into(),
            });
        }
        self.close_verified_claim(claim)
    }

    fn close_verified_claim(&mut self, claim: &ClaimPath) -> Result<(), ModelError> {
        validate::close_verified_claim(self, claim)
    }

    /// Opens only the named immediate call domains while preserving every
    /// known positive operation or callback.
    ///
    /// Artifact-closure validation uses this when an opaque edge can affect a
    /// finite set of domains. A complete negative becomes unknown and a
    /// complete positive becomes partial; unrelated call and recursive value
    /// knowledge is unchanged.
    /// Opening a domain also withdraws its *proposal*. A proposal is an offer
    /// to prove closure over exactly the knowledge the document states; an
    /// opaque closure frontier or a recipe-gated withholding that reopens the
    /// domain has invalidated that offer, so leaving the marker in place would
    /// let the candidate outlive the fact it was derived from — and would make
    /// `withheld_weakening` a no-op, since the certifier would rediscover the
    /// candidate it had just withheld.
    pub fn open_call_domains(&mut self, domains: impl IntoIterator<Item = ClaimDomain>) {
        for domain in domains {
            self.call.claims.open(domain);
            self.call.proposed_closures.remove(&domain);
            // A bound conditions a closed `reads` and nothing else (ADR 0153
            // item C); an open one has nothing left for it to bound.
            if domain == ClaimDomain::Reads {
                self.call.accessor_bounds.clear();
            }
        }
    }

    /// States that this export's closed `reads` holds against the named
    /// accessor-installation hazard sites (ADR 0153 item C).
    pub fn add_accessor_bounds(&mut self, sources: impl IntoIterator<Item = String>) {
        self.call.accessor_bounds.extend(sources);
    }

    /// Adds context premises to this export's claims (ADR 0153 part 3). A
    /// premise only ever weakens: the claims it conditions are the same
    /// claims, now stated for fewer programs.
    pub fn add_context_premises(&mut self, premises: impl IntoIterator<Item = ContextPremise>) {
        self.call.context_premises.extend(premises);
    }

    /// Clears `composed_from` naming any withdrawn `(export, operation)` of
    /// this artifact case.
    ///
    /// The sibling half of [`Self::withhold_operations`], which can only see
    /// its own export. Provenance is an *additional* discharge route, never the
    /// only one, so clearing it weakens the document and never strengthens it.
    pub fn clear_composed_provenance(&mut self, withdrawn: &BTreeSet<(String, OperationId)>) {
        for operation in &mut self.call.operations {
            if operation.composed_from.as_ref().is_some_and(|composed| {
                withdrawn.contains(&(composed.export.clone(), composed.operation.clone()))
            }) {
                operation.composed_from = None;
            }
        }
    }

    /// Withdraws operations whose positive facts no census could certify, and
    /// opens every domain that listed one.
    ///
    /// This is the weakening below [`Self::open_call_domains`]. Opening a
    /// domain keeps every operation and only stops claiming the enumeration is
    /// exhaustive; this *removes* an operation the document should never have
    /// stated, and then opens its domain for the same reason — a shorter list
    /// still marked closed would assert an absence the census never
    /// established, which is a stronger claim than the one being withdrawn.
    ///
    /// The withdrawal is transitive within the export, because a reference to
    /// a withdrawn operation describes nothing:
    ///
    /// - an operation triggered by a withdrawn one goes with it;
    /// - an edge touching a withdrawn operation is removed;
    /// - a callback invocation naming a withdrawn operation, or sourced from
    ///   its output, is removed and opens `callbacks`;
    /// - `composed_from` naming a withdrawn operation of *this* export is
    ///   cleared, which only removes a discharge route and never adds one.
    ///
    /// Returns every id actually withdrawn, the seeds included, so a caller
    /// can record the cascade rather than infer it. An id this export does not
    /// carry contributes nothing.
    pub fn withhold_operations(&mut self, seeds: &BTreeSet<OperationId>) -> BTreeSet<OperationId> {
        self.withhold_operations_narrowing(seeds, &BTreeSet::new())
    }

    /// [`Self::withhold_operations`], except that a seed in `narrowed` which is
    /// a non-call `invoke` (a property read, iteration, coercion or
    /// `hasInstance` of the caller's value) *narrows* `callbacks` instead of
    /// opening it: its item is removed and the domain keeps its knowledge
    /// state, closed or partial, and with it any proposed closure.
    ///
    /// Narrowing is not a weaker claim that the census is spared from checking.
    /// A closed enumeration that lost an item is still a closure candidate, and
    /// the implementation census re-confirms the narrowed enumeration site for
    /// site -- a use it does see still refuses as undescribed -- so nothing is
    /// certified the census did not confirm. The caller decides which items may
    /// narrow (the certifier: an item whose positive facts found no use of a
    /// parameter its declared signature types primitive-only); any other
    /// removal from `callbacks`, or a narrowed id that is not a non-call
    /// invoke, opens the domain as before.
    pub fn withhold_operations_narrowing(
        &mut self,
        seeds: &BTreeSet<OperationId>,
        narrowed: &BTreeSet<OperationId>,
    ) -> BTreeSet<OperationId> {
        let narrows: BTreeSet<OperationId> = self
            .call
            .operations
            .iter()
            .filter(|operation| narrowed.contains(&operation.id))
            .filter(|operation| {
                operation.kind == OperationKind::Invoke && operation.is_protocol_invocation()
            })
            .map(|operation| operation.id.clone())
            .collect();
        let mut gone: BTreeSet<OperationId> = self
            .call
            .operations
            .iter()
            .filter(|operation| seeds.contains(&operation.id))
            .map(|operation| operation.id.clone())
            .collect();
        loop {
            let cascade: BTreeSet<OperationId> = self
                .call
                .operations
                .iter()
                .filter(|operation| !gone.contains(&operation.id))
                .filter(|operation| match &operation.trigger {
                    Some(Trigger::Operation(trigger)) => gone.contains(trigger),
                    _ => false,
                })
                .map(|operation| operation.id.clone())
                .collect();
            if cascade.is_empty() {
                break;
            }
            gone.extend(cascade);
        }
        if gone.is_empty() {
            return gone;
        }
        let mut opened: BTreeSet<ClaimDomain> = BTreeSet::new();
        for domain in ClaimDomain::ALL {
            let Some(claim) = self.call.claims.operation_claim_mut(domain) else {
                continue;
            };
            let removed = match claim {
                KnowledgeSet::Unknown => continue,
                KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => {
                    let before = items.len();
                    items.retain(|id| !gone.contains(id));
                    before != items.len()
                }
            };
            if !removed {
                continue;
            }
            // A domain emptied by the withdrawal is *unknown*, not an empty
            // list. `Complete([])` proves the domain has no operations and
            // `Partial([])` is refused outright ("partial knowledge must
            // contain positive evidence"); the truth after withdrawing the
            // only thing it listed is that this document no longer says.
            if claim.items().is_empty() {
                *claim = KnowledgeSet::Unknown;
            }
            opened.insert(domain);
        }
        // A consumer reads a closed `creates` as "no owner requirement beyond
        // the published items" (`contracts.rs`' `project_owner_requirements`),
        // so withdrawing an operation that imposes one on the caller withdraws
        // that reading with it: `creates` opens too, and the import stays
        // uncertifiable rather than reading as needing no owner (ADR 0114).
        if self
            .call
            .operations
            .iter()
            .any(|operation| gone.contains(&operation.id) && operation.imposes_owner_requirement())
        {
            opened.insert(ClaimDomain::Creates);
        }
        let sourced_from_gone = |source: &ValueSource| match source {
            ValueSource::OperationOutput { operation, .. } => gone.contains(operation),
            _ => false,
        };
        let mut callbacks_narrowed = false;
        let callbacks_changed = match &mut self.call.claims.callbacks {
            KnowledgeSet::Unknown => false,
            KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => {
                // Removals that open the domain; a narrowing one does not.
                let mut opening = 0usize;
                items.retain(|invocation| {
                    let removed =
                        gone.contains(&invocation.operation) || sourced_from_gone(&invocation.from);
                    if !removed {
                        return true;
                    }
                    if narrows.contains(&invocation.operation) {
                        callbacks_narrowed = true;
                    } else {
                        opening += 1;
                    }
                    false
                });
                opening > 0
            }
        };
        // A partial enumeration narrowed to nothing is not "partial with no
        // item", which the model refuses; it says nothing, so it is unknown.
        if callbacks_narrowed
            && !callbacks_changed
            && !self.call.claims.callbacks.is_closed()
            && self.call.claims.callbacks.items().is_empty()
        {
            self.call.claims.callbacks = KnowledgeSet::Unknown;
        }
        if callbacks_changed {
            if self.call.claims.callbacks.items().is_empty() {
                self.call.claims.callbacks = KnowledgeSet::Unknown;
            }
            opened.insert(ClaimDomain::Callbacks);
        }
        self.call
            .operations
            .retain(|operation| !gone.contains(&operation.id));
        self.call
            .edges
            .retain(|edge| !gone.contains(&edge.from) && !gone.contains(&edge.to));
        for operation in &mut self.call.operations {
            if operation.composed_from.as_ref().is_some_and(|composed| {
                composed.export == self.identity.public_name && gone.contains(&composed.operation)
            }) {
                operation.composed_from = None;
            }
        }
        self.open_call_domains(opened);
        gone
    }

    #[must_use]
    pub fn unresolved_call_claims(&self) -> Vec<ClaimPath> {
        ClaimDomain::CLOSABLE
            .into_iter()
            .filter(|domain| self.call.claims.state(*domain).is_open())
            .map(ClaimPath::Call)
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClaimDomain {
    Callbacks,
    Reads,
    Writes,
    Creates,
    Invalidates,
    Throws,
    Returns,
    Cleanups,
    Disposals,
    /// The computations one invocation registers on an owner it does not
    /// create (ADR 0114). Version 1 states it by item only: no document may
    /// close it, so it is never one of [`Self::CLOSABLE`].
    Computations,
}

impl ClaimDomain {
    pub const ALL: [Self; 10] = [
        Self::Callbacks,
        Self::Reads,
        Self::Writes,
        Self::Creates,
        Self::Invalidates,
        Self::Throws,
        Self::Returns,
        Self::Cleanups,
        Self::Disposals,
        Self::Computations,
    ];

    /// The domains a document may name in `closed`, which are therefore the
    /// ones an open domain is an *unresolved claim* in: every domain but
    /// `computations`. That one has no closure in version 1 -- no census
    /// decides it and `closed` may not name it -- so a document that does not
    /// mention it says nothing, and nothing can resolve the silence (ADR 0114).
    pub const CLOSABLE: [Self; 9] = [
        Self::Callbacks,
        Self::Reads,
        Self::Writes,
        Self::Creates,
        Self::Invalidates,
        Self::Throws,
        Self::Returns,
        Self::Cleanups,
        Self::Disposals,
    ];

    /// Whether a document may name this domain in `closed`.
    #[must_use]
    pub fn is_closable(self) -> bool {
        Self::CLOSABLE.contains(&self)
    }

    /// The call domains a document may *propose* closed
    /// (`CallSemantics::proposed_closures`).
    ///
    /// The behavioral call domains the certifier has a proof mode for — the
    /// implementation census: `creates` (ADR 0008) and `returns` (ADR 0035,
    /// the empty closure only). A candidate the certifier cannot decide is not
    /// a weaker proposal, it is a refused row — every other domain refuses by
    /// name at witness acquisition, and only these are recipe-gated, so a
    /// proposal of one of the others could never close and could only turn a
    /// row whose every other claim was proven into a refusal. The generator's
    /// candidates for the other domains therefore stay in the proposal plan
    /// sidecar as measurement, and this list grows one domain at a time as
    /// each census lands.
    /// `Reads` is admitted (2026-09-10) on a premise it does not obtain
    /// itself: a `runtime-accessor-installation` closure hazard withdraws the
    /// domain for a case whose reads the census structurally cannot refuse.
    /// That hazard is computed twice, over two ASTs, and both must agree —
    /// see § 10 of
    /// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`.
    /// `Callbacks` is admitted (2026-09-12) for the empty enumeration only:
    /// "this export invokes none of the callable arguments it is handed". The
    /// implementation census proves it by there being no parameter-rooted
    /// disposition in the same call walk `creates` runs, and the synthesized
    /// veto contradicts it by observing invocation from inside the sampled
    /// callback. ADR 0023's line holds by construction: the walk dispositions
    /// calls, and retaining a callable is not one.
    pub const PROPOSABLE: [Self; 4] = [Self::Creates, Self::Returns, Self::Reads, Self::Callbacks];

    /// Whether a document may propose this domain for closure proof.
    #[must_use]
    pub fn is_proposable(self) -> bool {
        Self::PROPOSABLE.contains(&self)
    }

    /// The domain's stable wire name, as every document, audit and refusal
    /// sidecar spells it.
    ///
    /// One mapping, because there were two: the certifier had its own copy in
    /// `contract_certification::type_facts`, and a generator-side record
    /// needing the same names would have made a third. A name that drifts
    /// between producer and consumer is the dual-census failure in miniature.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Callbacks => "callbacks",
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::Creates => "creates",
            Self::Invalidates => "invalidates",
            Self::Throws => "throws",
            Self::Returns => "returns",
            Self::Cleanups => "cleanups",
            Self::Disposals => "disposals",
            Self::Computations => "computations",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClaimPath {
    Call(ClaimDomain),
    Value {
        root: ValueRoot,
        path: ValuePath,
        domain: ValueClaimDomain,
    },
    Operation {
        operation: OperationId,
        domain: OperationClaimDomain,
    },
    Resource {
        resource: ResourceId,
        domain: ResourceClaimDomain,
    },
    GuardPartition,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueRoot {
    Export,
    OperationInput { operation: OperationId, index: u16 },
    OperationOutput { operation: OperationId },
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValuePath(pub Vec<ValuePathSegment>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValuePathSegment {
    TupleItem(u32),
    ArrayElement,
    ObjectProperty(String),
    ChoiceAlternative(u32),
    PromiseValue,
    AsyncIterableElement,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueClaimDomain {
    Shape,
    TupleItems,
    ObjectProperties,
    ChoiceAlternatives,
    ArrayMinimumLength,
    ArrayMaximumLength,
    Capabilities,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OperationClaimDomain {
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
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceClaimDomain {
    States,
    Capabilities,
    Lifetime,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallSemantics {
    claims: CallClaims,
    /// The closed call domains this document *proposes* rather than asserts:
    /// closure a generator inferred and offers for proof, not closure an audit
    /// established.
    ///
    /// Each named domain is closed in `claims` — a proposal of a closure the
    /// document does not state is meaningless, and normalization refuses it.
    /// The closure has to be in the document because that is the only place a
    /// closure ever is: `ProofPolicy2::inspect_candidates` rebuilds the
    /// planner's candidate universe by weakening the candidate's own closed
    /// claims, and the canonical main a receipt binds is the candidate
    /// document itself. A candidate that stated its closure anywhere else —
    /// a sidecar, a request field — would make the planner's universe a
    /// caller's choice and would leave nothing for a receipt to bind.
    ///
    /// What the marker adds is the distinction the weakening used to carry:
    /// an emitted proposal is otherwise byte-indistinguishable from a reviewed
    /// document making the same claim. It is in the semantic digest for the
    /// same reason `composed_from` is — the two documents mean different
    /// things, and a receipt for one must not authenticate the other.
    proposed_closures: BTreeSet<ClaimDomain>,
    /// ADR 0153 part 3: the conditions every claim of this export holds under.
    /// Each names a context this package exports, and the claims hold only in
    /// a program where that context receives no value from outside the
    /// package. Empty for every export whose certification needed none.
    ///
    /// It is a condition, not a closure: weakening a candidate's closures
    /// keeps it, and a consumer that cannot show the condition holds reads
    /// every domain of the export as open.
    context_premises: BTreeSet<ContextPremise>,
    /// ADR 0153 item C: the closure's `runtime-accessor-installation` hazard
    /// sites, by the source each names, that this export's closed `reads`
    /// holds against. Each is a site whose target is an allocation its
    /// installing function makes fresh and on which this export can execute
    /// no operation, so what was installed there cannot run inside a call of
    /// it. A consumer opens `reads` for every accessor hazard of the closure
    /// not named here, exactly as it did before bounds existed. Stated only
    /// beside a closed `reads`.
    accessor_bounds: BTreeSet<String>,
    pub operations: Vec<Operation>,
    pub edges: Vec<OperationEdge>,
    pub resources: Vec<Resource>,
    pub guards: GuardPartition,
}

impl CallSemantics {
    #[must_use]
    pub fn new(
        claims: CallClaims,
        operations: Vec<Operation>,
        edges: Vec<OperationEdge>,
        resources: Vec<Resource>,
        guards: GuardPartition,
    ) -> Self {
        Self {
            claims,
            proposed_closures: BTreeSet::new(),
            context_premises: BTreeSet::new(),
            accessor_bounds: BTreeSet::new(),
            operations,
            edges,
            resources,
            guards,
        }
    }

    /// The same call semantics, additionally proposing the named domains for
    /// closure proof. The domains' knowledge is untouched.
    #[must_use]
    pub fn with_proposed_closures(
        mut self,
        domains: impl IntoIterator<Item = ClaimDomain>,
    ) -> Self {
        self.proposed_closures.extend(domains);
        self
    }

    #[must_use]
    pub const fn proposed_closures(&self) -> &BTreeSet<ClaimDomain> {
        &self.proposed_closures
    }

    /// The same call semantics, holding only under the named context
    /// premises as well (ADR 0153 part 3).
    #[must_use]
    pub fn with_context_premises(
        mut self,
        premises: impl IntoIterator<Item = ContextPremise>,
    ) -> Self {
        self.context_premises.extend(premises);
        self
    }

    #[must_use]
    pub const fn context_premises(&self) -> &BTreeSet<ContextPremise> {
        &self.context_premises
    }

    /// The same call semantics, with its closed `reads` bounded against the
    /// named accessor-installation hazard sites as well (ADR 0153 item C).
    #[must_use]
    pub fn with_accessor_bounds(mut self, sources: impl IntoIterator<Item = String>) -> Self {
        self.accessor_bounds.extend(sources);
        self
    }

    #[must_use]
    pub const fn accessor_bounds(&self) -> &BTreeSet<String> {
        &self.accessor_bounds
    }

    #[must_use]
    pub fn claim_state(&self, domain: ClaimDomain) -> KnowledgeState {
        self.claims.state(domain)
    }

    #[must_use]
    pub const fn claims(&self) -> &CallClaims {
        &self.claims
    }
}

/// ADR 0153 part 3: a context this package exports as `export` receives no
/// value from outside the package in the program the claims are used in.
///
/// A context that a package creates and provides itself can only be read as
/// what the package provided, and a certification may rest a claim on that.
/// Once the context escapes through an export, a consumer can provide its own
/// value (`<RouterContext value={mock}>`), so the claim is stated under this
/// premise, and the consumer discharges it or loses the claim.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct ContextPremise {
    /// The package export the context escapes under (`RouterContext`).
    pub export: String,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallClaims {
    pub callbacks: KnowledgeSet<CallbackInvocation>,
    pub reads: KnowledgeSet<OperationId>,
    pub writes: KnowledgeSet<OperationId>,
    pub creates: KnowledgeSet<OperationId>,
    pub invalidates: KnowledgeSet<OperationId>,
    pub throws: KnowledgeSet<OperationId>,
    pub returns: KnowledgeSet<OperationId>,
    pub cleanups: KnowledgeSet<OperationId>,
    pub disposals: KnowledgeSet<OperationId>,
    /// ADR 0114: never `Complete` in version 1.
    pub computations: KnowledgeSet<OperationId>,
}

impl CallClaims {
    #[must_use]
    pub fn state(&self, domain: ClaimDomain) -> KnowledgeState {
        match domain {
            ClaimDomain::Callbacks => self.callbacks.state(),
            domain => self
                .operation_claim(domain)
                .expect("non-callback claim has an operation domain")
                .state(),
        }
    }

    fn operation_claim_mut(
        &mut self,
        domain: ClaimDomain,
    ) -> Option<&mut KnowledgeSet<OperationId>> {
        match domain {
            ClaimDomain::Callbacks => None,
            ClaimDomain::Reads => Some(&mut self.reads),
            ClaimDomain::Writes => Some(&mut self.writes),
            ClaimDomain::Creates => Some(&mut self.creates),
            ClaimDomain::Invalidates => Some(&mut self.invalidates),
            ClaimDomain::Throws => Some(&mut self.throws),
            ClaimDomain::Returns => Some(&mut self.returns),
            ClaimDomain::Cleanups => Some(&mut self.cleanups),
            ClaimDomain::Disposals => Some(&mut self.disposals),
            ClaimDomain::Computations => Some(&mut self.computations),
        }
    }

    #[must_use]
    pub const fn operation_claim(&self, domain: ClaimDomain) -> Option<&KnowledgeSet<OperationId>> {
        match domain {
            ClaimDomain::Callbacks => None,
            ClaimDomain::Reads => Some(&self.reads),
            ClaimDomain::Writes => Some(&self.writes),
            ClaimDomain::Creates => Some(&self.creates),
            ClaimDomain::Invalidates => Some(&self.invalidates),
            ClaimDomain::Throws => Some(&self.throws),
            ClaimDomain::Returns => Some(&self.returns),
            ClaimDomain::Cleanups => Some(&self.cleanups),
            ClaimDomain::Disposals => Some(&self.disposals),
            ClaimDomain::Computations => Some(&self.computations),
        }
    }

    fn open(&mut self, domain: ClaimDomain) {
        match domain {
            ClaimDomain::Callbacks => {
                self.callbacks = std::mem::take(&mut self.callbacks).weaken();
            }
            ClaimDomain::Reads => self.reads = std::mem::take(&mut self.reads).weaken(),
            ClaimDomain::Writes => self.writes = std::mem::take(&mut self.writes).weaken(),
            ClaimDomain::Creates => self.creates = std::mem::take(&mut self.creates).weaken(),
            ClaimDomain::Invalidates => {
                self.invalidates = std::mem::take(&mut self.invalidates).weaken();
            }
            ClaimDomain::Throws => self.throws = std::mem::take(&mut self.throws).weaken(),
            ClaimDomain::Returns => self.returns = std::mem::take(&mut self.returns).weaken(),
            ClaimDomain::Cleanups => self.cleanups = std::mem::take(&mut self.cleanups).weaken(),
            ClaimDomain::Disposals => {
                self.disposals = std::mem::take(&mut self.disposals).weaken();
            }
            ClaimDomain::Computations => {
                self.computations = std::mem::take(&mut self.computations).weaken();
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallbackInvocation {
    pub from: ValueSource,
    pub operation: OperationId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueSource {
    Parameter {
        index: u16,
        path: Vec<String>,
    },
    OperationOutput {
        operation: OperationId,
        path: Vec<String>,
    },
    Resource {
        resource: ResourceId,
        path: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationId(pub String);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceId(pub String);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OperationKind {
    Invoke,
    Return,
    Read,
    Write,
    Invalidate,
    Create,
    Cleanup,
    Dispose,
    /// Registering a computation on an owner this operation does not create
    /// (ADR 0114); the `computations` domain's one kind.
    Compute,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Event {
    Call,
    Render,
    Flush,
    Settle,
    Transition,
    AsyncEmission,
    Cleanup,
    External,
    Request,
    ResponseCommitment,
    /// ADR 0139: the export stores the callable only in the value it returns
    /// (for a construction, the instance), and the callable runs later, on
    /// the stack of code that invokes it through that value. Valid only on an
    /// `invoke` a `callbacks` item names from a bare parameter, with schedule
    /// `external`, tracking and owner `ambient-at-execution`, counted per
    /// trigger from zero to many, unguarded
    /// (`validate::validate_result_access_operation`).
    ResultAccess,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Trigger {
    Event(Event),
    Operation(OperationId),
    Resource { resource: ResourceId, event: Event },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Schedule {
    SameStack,
    Queued,
    External,
}

/// Which protocol of the caller's value an `invoke` operation runs.
///
/// A call is the historical meaning of every `invoke`, and stays the one
/// spelled by absence: [`Operation::protocol`] is `None` for it, and a decoded
/// `call` normalizes to `None`, so the model has one meaning for it. The other
/// four are the non-call invocations of caller-supplied code `semantic-model.md`
/// § callbacks names: a property read that may run a getter or a proxy trap
/// (`Get`), the iteration protocol (`Iterate`), ToPrimitive (`Coerce`), and
/// `Symbol.hasInstance` (`HasInstance`). Each runs whatever the caller's value
/// carries, at the call, on the caller's stack, in the caller's tracking
/// context, so a non-call item is always `ambient-at-execution` and never a
/// claim that the export clears or establishes tracking.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvokeProtocol {
    Call,
    Get,
    Iterate,
    Coerce,
    HasInstance,
}

impl InvokeProtocol {
    /// The wire spelling (`has-instance`, kebab-case).
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Get => "get",
            Self::Iterate => "iterate",
            Self::Coerce => "coerce",
            Self::HasInstance => "has-instance",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tracking {
    Tracked,
    Untracked,
    AmbientAtExecution,
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerSource {
    None,
    AmbientAtCall,
    AmbientAtExecution,
    Captured(ResourceId),
    Created(ResourceId),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Requirement {
    Required,
    Forbidden,
    Unconstrained,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapabilityKnowledge {
    Allowed,
    Forbidden,
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Lifetime {
    Call,
    Resource(ResourceId),
    Owner(ResourceId),
    Request(ResourceId),
    Transition(ResourceId),
    AsyncSource(ResourceId),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerRequirements {
    pub owner: Requirement,
    pub child_owners: Requirement,
    pub cleanup: Requirement,
}

impl Default for OwnerRequirements {
    fn default() -> Self {
        Self {
            owner: Requirement::Unconstrained,
            child_owners: Requirement::Unconstrained,
            cleanup: Requirement::Unconstrained,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerCapabilities {
    pub child_owners: CapabilityKnowledge,
    pub cleanup: CapabilityKnowledge,
}

impl Default for OwnerCapabilities {
    fn default() -> Self {
        Self {
            child_owners: CapabilityKnowledge::Unknown,
            cleanup: CapabilityKnowledge::Unknown,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerProduction {
    pub resource: ResourceId,
    pub capabilities: OwnerCapabilities,
    pub lifetime: Option<Lifetime>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OwnerRelation {
    pub source: OwnerSource,
    pub requirements: OwnerRequirements,
    pub capabilities: OwnerCapabilities,
    pub lifetime: Option<Lifetime>,
    pub productions: KnowledgeSet<OwnerProduction>,
}

impl Default for OwnerRelation {
    fn default() -> Self {
        Self {
            source: OwnerSource::Unknown,
            requirements: OwnerRequirements::default(),
            capabilities: OwnerCapabilities::default(),
            lifetime: None,
            productions: KnowledgeSet::Unknown,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CardinalityScope {
    Trigger,
    Call,
    Resource(ResourceId),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UpperBound {
    Finite(u32),
    Many,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Cardinality {
    pub scope: Option<CardinalityScope>,
    pub min: Option<u32>,
    pub max: Option<UpperBound>,
}

impl Cardinality {
    #[must_use]
    pub const fn strength(&self) -> BehaviorStrength {
        match self.min {
            Some(1..) => BehaviorStrength::Guaranteed,
            Some(0) | None => BehaviorStrength::Possible,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BehaviorStrength {
    Possible,
    Guaranteed,
}

/// The `(export, operation)` an operation was composed from, inside the same
/// artifact case.
///
/// A *positive* claim, not a hint: "the behaviour this operation describes is
/// that export's own operation, performed through this export's call to it".
/// The export is named because a consumer has to resolve the composing call's
/// callee to it exactly, and the operation is named because a composed row
/// says which of the target's operations it is — "some read of that export"
/// would be a claim about a set.
///
/// Composition is intra-package and same-stack by construction. The operation
/// id is qualified with this artifact case, so a provenance can never name
/// another package's export; and a consumer must prove the composing call is
/// a reachable, uncaptured *call*, so the composed row's `at: call /
/// schedule: same-stack` stamp survives the hop rather than being inherited
/// through a closure.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ComposedFrom {
    pub export: String,
    pub operation: OperationId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Operation {
    pub id: OperationId,
    pub kind: OperationKind,
    pub guard: Option<Guard>,
    pub trigger: Option<Trigger>,
    pub at: Option<Event>,
    pub schedule: Option<Schedule>,
    pub tracking: Tracking,
    pub owner: OwnerRelation,
    pub cardinality: Cardinality,
    pub inputs: Vec<ValueShape>,
    pub output: Option<ValueShape>,
    pub resources: BTreeSet<ResourceId>,
    /// The `(export, operation)` of the same artifact case this operation was
    /// composed from, when the generator could name it exactly.
    ///
    /// `None` is every other case and keeps the operation's own evidence the
    /// only route to discharging it. Provenance may only *add* a discharge
    /// route, never remove one.
    pub composed_from: Option<ComposedFrom>,
    /// The protocol a non-call `invoke` runs (see [`InvokeProtocol`]). `None`
    /// is a call, and is the only value any other kind may carry.
    pub protocol: Option<InvokeProtocol>,
}

impl Operation {
    /// Whether this operation imposes an owner obligation on the export's
    /// *caller*: it requires an owner it does not itself create.
    ///
    /// `requires: required` alone is not the test. Audited `render`'s
    /// `register-delegation` requires an owner *and* made it (`source:
    /// created`), and reading `requires` alone would report an owner-less
    /// effect for a top-level `render(...)`. The consumer's owner-requirement
    /// projection and [`ExportSemantics::withhold_operations`] both ask this,
    /// so the two cannot disagree about which operations a withdrawal loses.
    #[must_use]
    pub fn imposes_owner_requirement(&self) -> bool {
        self.owner.requirements.owner == Requirement::Required
            && !matches!(self.owner.source, OwnerSource::Created(_))
    }

    /// Whether this is a `return` that states nothing but its output: unguarded,
    /// triggered by and at the call, on the same stack, untracked, under no
    /// owner relation, of the default per-call cardinality, with no input, no
    /// resource, no provenance and no protocol (ADR 0170).
    ///
    /// The shape every generated `return` has. A dependency's return that
    /// states anything more -- a guard, an owner, a resource it names -- is not
    /// one a re-exporting package can state again by output alone, because the
    /// restatement would drop what the extra field said.
    #[must_use]
    pub fn is_bare_return(&self) -> bool {
        self.kind == OperationKind::Return
            && self.guard.is_none()
            && self.trigger == Some(Trigger::Event(Event::Call))
            && self.at == Some(Event::Call)
            && self.schedule == Some(Schedule::SameStack)
            && self.tracking == Tracking::Untracked
            && self.owner == OwnerRelation::default()
            && self.cardinality
                == (Cardinality {
                    scope: Some(CardinalityScope::Call),
                    min: Some(0),
                    max: Some(UpperBound::Many),
                })
            && self.inputs.is_empty()
            && self.resources.is_empty()
            && self.composed_from.is_none()
            && self.protocol.is_none()
    }

    /// The protocol this operation invokes, `Call` for every operation that
    /// states none.
    #[must_use]
    pub fn invoke_protocol(&self) -> InvokeProtocol {
        self.protocol.unwrap_or(InvokeProtocol::Call)
    }

    /// Whether this is an `invoke` of a protocol other than a call — a
    /// property read, iteration, coercion or `hasInstance` of the caller's
    /// value, which is not an inline invocation of a callable.
    #[must_use]
    pub fn is_protocol_invocation(&self) -> bool {
        self.invoke_protocol() != InvokeProtocol::Call
    }

    /// Whether this operation happens at [`Event::ResultAccess`] (ADR 0139):
    /// its execution point or its trigger names the event.
    #[must_use]
    pub fn is_result_access(&self) -> bool {
        self.at == Some(Event::ResultAccess)
            || matches!(
                self.trigger,
                Some(Trigger::Event(Event::ResultAccess))
                    | Some(Trigger::Resource {
                        event: Event::ResultAccess,
                        ..
                    })
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EdgeKind {
    Orders,
    Data,
    Invalidates,
    Error,
    Cleanup,
    Lifetime,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationEdge {
    pub kind: EdgeKind,
    pub from: OperationId,
    pub to: OperationId,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceKind {
    Owner,
    ReactiveSource,
    AsyncComputation,
    Transition,
    Cleanup,
    Request,
    Response,
    Stream,
    ServerFunctionReference,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceState {
    OwnerActive,
    OwnerDisposed,
    CleanupInstalled,
    CleanupDisposed,
    AsyncPending,
    AsyncSettled,
    AsyncErrored,
    AsyncCancelled,
    TransitionActive,
    TransitionSettled,
    TransitionReverted,
    ResponseUncommitted,
    ResponseCommitted,
    StreamUnclaimed,
    StreamClaimed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceCapability {
    Refreshable,
    Writable,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Resource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub states: KnowledgeSet<ResourceState>,
    pub capabilities: KnowledgeSet<ResourceCapability>,
    pub lifetime: Option<Lifetime>,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Guard(pub Vec<GuardAtom>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GuardAtom {
    Signature(String),
    ArgumentCount {
        min: u16,
        max: Option<u16>,
    },
    Literal {
        argument: u16,
        path: Vec<String>,
        value: Literal,
    },
    ValueKind {
        argument: u16,
        path: Vec<String>,
        kind: ValueKind,
    },
    Property {
        argument: u16,
        path: Vec<String>,
        name: String,
        callable: Option<bool>,
    },
    TupleAlternative {
        argument: u16,
        alternative: u16,
    },
    ResultProtocol(ValueKind),
    ArtifactCase(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Literal {
    Null,
    Bool(bool),
    Number(String),
    String(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GuardedCase {
    When {
        guard: Guard,
        operations: KnowledgeSet<OperationId>,
    },
    Otherwise {
        operations: KnowledgeSet<OperationId>,
    },
}

impl GuardedCase {
    #[must_use]
    pub const fn operations(&self) -> &KnowledgeSet<OperationId> {
        match self {
            Self::When { operations, .. } | Self::Otherwise { operations } => operations,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct GuardPartition {
    pub cases: KnowledgeSet<GuardedCase>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GuardTruth {
    True,
    False,
    Unknown,
}

impl GuardPartition {
    /// Selects exact cases when possible and otherwise joins every possible
    /// alternative without retaining a complete negative from one branch.
    #[must_use]
    pub fn select_operations(
        &self,
        evaluate: impl FnMut(&GuardAtom) -> GuardTruth,
    ) -> KnowledgeSet<OperationId> {
        guards::select_operations(self, evaluate)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservableCapability {
    Readable,
    Writable,
    Refreshable,
    PendingAware,
    Optimistic,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapabilityClaim {
    pub capability: ObservableCapability,
    pub resource: Option<ResourceId>,
}

/// Version 1 has only positive experimental evidence. Unknown is not stable.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StabilityKnowledge {
    Unknown,
    Experimental,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReactiveRole {
    Accessor,
    Setter,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueKind {
    Plain,
    Callable,
    Promise,
    AsyncIterable,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObjectProperty {
    pub name: String,
    pub value: ValueShape,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArrayLength {
    pub min: Option<u32>,
    pub max: Option<UpperBound>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueShape {
    Unknown,
    Plain,
    Parameter {
        index: u16,
        path: Vec<String>,
    },
    Tuple(KnowledgeSet<ValueShape>),
    Array {
        element: Box<ValueShape>,
        length: ArrayLength,
    },
    Object(KnowledgeSet<ObjectProperty>),
    Choice(KnowledgeSet<ValueShape>),
    Callable,
    Promise(Box<ValueShape>),
    AsyncIterable(Box<ValueShape>),
    Reactive {
        role: ReactiveRole,
        resource: Option<ResourceId>,
        capabilities: KnowledgeSet<CapabilityClaim>,
    },
    Store {
        resource: Option<ResourceId>,
        capabilities: KnowledgeSet<CapabilityClaim>,
    },
    /// An object whose property reads reach through to **the caller's argument
    /// at `from`**: what a props merge yields (ADR 0109).
    ///
    /// Conditional, and that is the whole shape. A merge creates no reactive
    /// source; it carries its arguments'. `solid-js@1.9.14` returns a `$PROXY`
    /// only when some source is already a proxy or is a function, and otherwise
    /// rebuilds the object preserving each source's own descriptors — so
    /// `mergeProps({ a: 1 }, { b: 2 })` is plain and destructuring it loses
    /// nothing. A [`ValueShape::Store`] here would assert reactivity the
    /// runtime does not always produce; this shape asserts it exactly of a
    /// caller who passed a reactive argument at `from`, and asserts nothing
    /// otherwise.
    MergedProps {
        from: u16,
    },
    /// A **fresh** array whose elements, in order, are exactly the caller's own
    /// arguments at `items` (ADR 0115): `[value]` is `[0]`, and `[]` is empty.
    ///
    /// Exact by construction, so it carries no knowledge set and no closure of
    /// its own to decide: a `Tuple` of `Parameter`s would say the same thing
    /// with a `tuple-items` closure beside it, which no census decides for an
    /// operation's output. The array is the callee's, freshly built; what it
    /// holds is the caller's.
    ArgumentArray {
        items: Vec<u16>,
    },
    /// The value **an invocation of the caller's own argument at `parameter`**
    /// returned, handed back unchanged (ADR 0116): `valueOrFn(...args)` is
    /// `parameter: 0`.
    ///
    /// Exact for the same reason [`ValueShape::ArgumentArray`] is: no
    /// knowledge set and no closure of its own. It names no argument of the
    /// invocation -- which arguments produced the value is the `callbacks`
    /// domain's question -- and says nothing of the value but where it came
    /// from.
    InvocationResult {
        parameter: u16,
    },
    /// Exactly `undefined` (item B round 2 of ways-to-improve § 3.3): what an
    /// optional chain hands back when its receiver is nullish -- `p?.key`'s
    /// other value. Narrower than [`ValueShape::Plain`], which says only
    /// "no reactive capability": this names the one value, so a `returns`
    /// census can enumerate it beside the member read and a veto can compare
    /// a completion with it by `===`. Exact, with no knowledge set and no
    /// closure of its own.
    Undefined,
    Action {
        transition: Option<ResourceId>,
    },
    Component,
    Cleanup {
        resource: Option<ResourceId>,
        lifetime: Option<Lifetime>,
    },
    RefApplication,
    ServerFunctionReference {
        resource: Option<ResourceId>,
    },
    /// ADR 0145: a callable the export hands its caller, **together with what
    /// one invocation of it does** -- its own call claims, stated exactly.
    ///
    /// Exact by construction, like [`ValueShape::ArgumentArray`]: it carries
    /// no knowledge set and so no closure of its own for a census to decide.
    /// Stated, it says that one invocation of the value, by whoever holds it,
    /// invokes no callable it did not itself define -- neither its own
    /// arguments nor any value the export was handed (`callbacks: []`),
    /// creates nothing and registers nothing on an owner (`creates: []`),
    /// performs exactly [`DescribedCall::reads`], and hands back exactly one of
    /// [`DescribedCall::returns`] (none: it completes without a value). It says
    /// nothing about the other domains. Where the census cannot establish all
    /// of that, the `return` stating it is withdrawn and the domain opens: a
    /// nested claim is never partially stated.
    ///
    /// Valid only as the whole output of a `return` operation
    /// (`validate::normalize_described_callable`).
    DescribedCallable(Box<DescribedCall>),
    /// ADR 0146: exactly the value one of the enclosing described callable's
    /// own [`DescribedCall::reads`] observed, handed back unchanged --
    /// `() => count()`. Valid only as an item of a described callable's
    /// `returns` whose `reads` is not empty (`validate::normalize_described_callable`).
    ReadValue,
}

/// ADR 0145: what one invocation of a [`ValueShape::DescribedCallable`] does.
///
/// Every list is an exact enumeration, canonically sorted and without
/// duplicates. `returns` admits only exact outputs whose meaning does not
/// depend on who calls: `plain`, (ADR 0146) [`ValueShape::ReadValue`], and
/// (ADR 0152) [`ValueShape::InvocationResult`] of an argument one of
/// `callbacks` names. Every read is performed on the invoking caller's stack,
/// in that caller's tracking context.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DescribedCall {
    pub reads: Vec<DescribedRead>,
    pub returns: Vec<ValueShape>,
    /// ADR 0152: the callables one invocation of the described callable runs
    /// that it did not itself define -- each an argument the **export** was
    /// handed and the returned callable captured. Empty is ADR 0145's
    /// `callbacks: []`, and it is how every document before ADR 0152 reads.
    pub callbacks: Vec<DescribedCallback>,
}

/// ADR 0152: one invocation a described callable performs of a callable its
/// export was handed, in the top-level `callbacks` vocabulary: a `from` (the
/// export's own argument, a bare [`ValueSource::Parameter`]) and the
/// invocation's execution point, schedule, tracking, owner and count, spelled
/// as an `invoke` operation spells them. Stated inside a described callable,
/// every `call` is the invocation of the described callable, not the export's.
///
/// Validation admits exactly one invocation today
/// (`validate::normalize_described_callable`): triggered by and at that call,
/// on the same stack, in the invoking caller's tracking context and under its
/// owner, exactly once per invocation, unguarded, a call. That is the one the
/// census can prove -- a call of the captured argument in the returned
/// literal's own frame that runs on every normal completion of it; a deferred,
/// conditional or repeated invocation is not stated at all, and the `return`
/// carrying it is withdrawn instead.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DescribedCallback {
    pub from: ValueSource,
    pub trigger: Option<Trigger>,
    pub at: Option<Event>,
    pub schedule: Option<Schedule>,
    pub tracking: Tracking,
    pub owner: OwnerRelation,
    pub cardinality: Cardinality,
}

impl DescribedCallback {
    /// The one invocation ADR 0152 admits, of the export's argument `index`.
    #[must_use]
    pub fn same_stack_once(index: u16) -> Self {
        Self {
            from: ValueSource::Parameter {
                index,
                path: Vec::new(),
            },
            trigger: Some(Trigger::Event(Event::Call)),
            at: Some(Event::Call),
            schedule: Some(Schedule::SameStack),
            tracking: Tracking::AmbientAtExecution,
            owner: OwnerRelation {
                source: OwnerSource::AmbientAtExecution,
                requirements: OwnerRequirements::default(),
                capabilities: OwnerCapabilities::default(),
                lifetime: None,
                productions: KnowledgeSet::Unknown,
            },
            cardinality: Cardinality {
                scope: Some(CardinalityScope::Call),
                min: Some(1),
                max: Some(UpperBound::Finite(1)),
            },
        }
    }

    /// The export argument this item invokes, when it is the bare parameter
    /// the one admitted shape names.
    #[must_use]
    pub fn parameter(&self) -> Option<u16> {
        match &self.from {
            ValueSource::Parameter { index, path } if path.is_empty() => Some(*index),
            _ => None,
        }
    }
}

/// ADR 0145/0146: one reactive read a described callable performs when it is
/// invoked.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DescribedRead {
    /// ADR 0146: a read of a signal accessor the export's own invocation
    /// created through a dialect primitive and captured, whose read runs no
    /// code of anyone's.
    OwnedSignal,
    /// ADR 0162: a read of a memo accessor the export's own invocation
    /// created with the dialect's `createMemo` and handed back unaltered. It
    /// observes the memo's current value in the invoking caller's tracking
    /// context. Unlike [`DescribedRead::OwnedSignal`] it is **not inert**: when
    /// the memo is stale the read re-runs the computation the creating call
    /// registered, and that computation -- the code the export defined or was
    /// handed, and every callable it invokes -- is accounted for by the
    /// export's own `creates` and `callbacks` claims where it was registered,
    /// never by this read. Its creating call's options are certified to retain
    /// no callback; the read invokes no other callable, and may throw
    /// the memo's own error or a not-ready signal.
    OwnedMemo,
}

impl DescribedRead {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::OwnedSignal => "owned-signal",
            Self::OwnedMemo => "owned-memo",
        }
    }

    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "owned-signal" => Some(Self::OwnedSignal),
            "owned-memo" => Some(Self::OwnedMemo),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
