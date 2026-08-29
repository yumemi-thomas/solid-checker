//! Wire-independent package behavior at the analyzer trust seam.
//!
//! Compact JSON concepts such as summary names, `closed` arrays, aliases,
//! omission rules, and schema versions deliberately do not appear here. The
//! backend expands those mechanics and submits only semantic concepts to
//! [`ContractProposal::normalize`]. Validation, guard selection, recursive
//! uncertainty, and canonical semantic identity stay inside this deep module.

mod canonical;
mod consumer;
mod guards;
pub mod proof;
pub mod solid2_rc3;
mod validate;

pub use consumer::{
    AcceptedContractIndex, AcceptedContractInput, AcceptedContractUse, AcceptedImportIdentity,
    AcceptedSemanticIdentity, CallSiteFacts, FiniteFact, InstantiatedClaim, InstantiatedExport,
    OpenDomainDiagnostic, OpenDomainReason, PropertyFact, SemanticQueryError,
    native_claim_precedence,
};

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

pub const SEMANTIC_MODEL_VERSION: u16 = 1;
/// Hash family frozen for semantic-model version 1.
pub const SEMANTIC_DIGEST_ALGORITHM: &str = "sha256";
/// Domain separator frozen for semantic-model version 1 contract identities.
pub const SEMANTIC_DIGEST_DOMAIN: &str = "solid-checker:normalized-package-contract";
pub const SEMANTIC_CLAIM_ID_VERSION: u16 = 1;

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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCase {
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
    pub fn open_call_domains(&mut self, domains: impl IntoIterator<Item = ClaimDomain>) {
        for domain in domains {
            self.call.claims.open(domain);
        }
    }

    #[must_use]
    pub fn unresolved_call_claims(&self) -> Vec<ClaimPath> {
        ClaimDomain::ALL
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
}

impl ClaimDomain {
    pub const ALL: [Self; 9] = [
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
            operations,
            edges,
            resources,
            guards,
        }
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
}

#[cfg(test)]
mod tests;
