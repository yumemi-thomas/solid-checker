//! Wire-independent package behavior accepted at the analyzer trust boundary.
//!
//! Compact JSON concepts such as summary names, `closed` arrays, aliases, and
//! schema versions deliberately do not appear here. The backend expands and
//! validates those details before constructing this model.

use std::collections::{BTreeMap, BTreeSet};

pub const SEMANTIC_MODEL_VERSION: u16 = 1;

/// Local knowledge for one set-valued claim. An open empty set is
/// unrepresentable: it normalizes to [`Self::Unknown`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeState {
    Unknown,
    PartialPositive,
    CompletePositive,
    CompleteNegative,
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
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    Digest,
    MissingArtifactCase,
    SemanticModelVersion { expected: u16, actual: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    pub integrity: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactIdentity {
    pub path: String,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationIdentity {
    pub path: String,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionStep {
    pub condition: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCase {
    pub id: String,
    pub entrypoint: String,
    pub resolution_trace: Vec<ResolutionStep>,
    pub runtime: ArtifactIdentity,
    pub declarations: DeclarationIdentity,
    pub dependency_closure: Digest,
    pub transform: Option<ArtifactIdentity>,
    pub stability: StabilityKnowledge,
    pub exports: BTreeMap<String, ExportSemantics>,
}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceReference {
    pub claim: ClaimPath,
    pub digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceBundle {
    pub artifact: ArtifactIdentity,
    pub dependency_closure: Digest,
    pub static_proofs: Vec<EvidenceReference>,
    pub probe_observations: Vec<EvidenceReference>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedContract {
    package: PackageIdentity,
    selected_case: ArtifactCase,
    receipt: AcceptanceReceipt,
}

impl AcceptedContract {
    /// Constructs the typestate after the backend has independently validated
    /// selection, identity, normalization, and every receipt binding.
    pub fn from_verified_selection(
        proposal: ContractProposal,
        selected_case: usize,
        receipt: AcceptanceReceipt,
    ) -> Result<Self, ModelError> {
        if receipt.semantic_model_version != proposal.semantic_model_version {
            return Err(ModelError::SemanticModelVersion {
                expected: proposal.semantic_model_version,
                actual: receipt.semantic_model_version,
            });
        }
        let selected_case = proposal
            .artifact_cases
            .get(selected_case)
            .cloned()
            .ok_or(ModelError::MissingArtifactCase)?;
        Ok(Self {
            package: proposal.package,
            selected_case,
            receipt,
        })
    }

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

    #[must_use]
    pub fn export(&self, name: &str) -> Option<&ExportSemantics> {
        self.selected_case.exports.get(name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportSemantics {
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

    #[must_use]
    pub fn unresolved_call_claims(&self) -> Vec<ClaimPath> {
        ClaimDomain::ALL
            .into_iter()
            .filter(|domain| {
                matches!(
                    self.call.claim_state(*domain),
                    KnowledgeState::Unknown | KnowledgeState::PartialPositive
                )
            })
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
    Value(String),
    Resource(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSemantics {
    claims: CallClaims,
    pub operations: Vec<Operation>,
    pub edges: Vec<OperationEdge>,
    pub resources: Vec<Resource>,
    pub cases: Vec<GuardedCase>,
}

impl CallSemantics {
    #[must_use]
    pub fn new(
        claims: CallClaims,
        operations: Vec<Operation>,
        edges: Vec<OperationEdge>,
        resources: Vec<Resource>,
        cases: Vec<GuardedCase>,
    ) -> Self {
        Self {
            claims,
            operations,
            edges,
            resources,
            cases,
        }
    }

    #[must_use]
    pub fn claim_state(&self, domain: ClaimDomain) -> KnowledgeState {
        self.claims.state(domain)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallbackInvocation {
    pub from: ValueSource,
    pub operation: OperationId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    Event(Event),
    Operation(OperationId),
    Resource(ResourceId, Event),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schedule {
    SameStack,
    Queued,
    External,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tracking {
    Tracked,
    Untracked,
    AmbientAtExecution,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSource {
    None,
    AmbientAtCall,
    AmbientAtExecution,
    Captured,
    Created,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Requirement {
    Required,
    Forbidden,
    Unconstrained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityKnowledge {
    Allowed,
    Forbidden,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifetime {
    Call,
    Resource,
    Owner,
    Request,
    Transition,
    AsyncSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerRelation {
    pub source: OwnerSource,
    pub resource: Option<ResourceId>,
    pub requirement: Requirement,
    pub child_owners: CapabilityKnowledge,
    pub cleanup: CapabilityKnowledge,
    pub lifetime: Option<Lifetime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardinalityScope {
    Trigger,
    Call,
    Resource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpperBound {
    Finite(u32),
    Many,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cardinality {
    pub scope: Option<CardinalityScope>,
    pub min: Option<u32>,
    pub max: Option<UpperBound>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub id: OperationId,
    pub kind: OperationKind,
    pub guard: Guard,
    pub trigger: Trigger,
    pub at: Event,
    pub schedule: Schedule,
    pub tracking: Tracking,
    pub owner: OwnerRelation,
    pub cardinality: Cardinality,
    pub inputs: Vec<ValueShape>,
    pub output: Option<ValueShape>,
    pub resources: BTreeSet<ResourceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeKind {
    Orders,
    Data,
    Invalidates,
    Error,
    Cleanup,
    Lifetime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationEdge {
    pub kind: EdgeKind,
    pub from: OperationId,
    pub to: OperationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub states: KnowledgeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guard(pub Vec<GuardAtom>);

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Number(String),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardedCase {
    pub guard: Option<Guard>,
    pub otherwise: bool,
    pub operations: Vec<OperationId>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservableCapability {
    Readable,
    Writable,
    Refreshable,
    PendingAware,
    Optimistic,
}

/// Version 1 has only positive experimental evidence. Unknown is not stable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StabilityKnowledge {
    Unknown,
    Experimental,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReactiveRole {
    Accessor,
    Setter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    Plain,
    Callable,
    Promise,
    AsyncIterable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectProperty {
    pub name: String,
    pub value: ValueShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        min_length: Option<u32>,
        max_length: Option<u32>,
    },
    Object(KnowledgeSet<ObjectProperty>),
    Choice(KnowledgeSet<ValueShape>),
    Callable,
    Promise(Box<ValueShape>),
    AsyncIterable(Box<ValueShape>),
    Reactive {
        role: ReactiveRole,
        resource: Option<ResourceId>,
        capabilities: KnowledgeSet<ObservableCapability>,
    },
    Store {
        resource: Option<ResourceId>,
        capabilities: KnowledgeSet<ObservableCapability>,
    },
    Action {
        resource: Option<ResourceId>,
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
mod tests {
    use super::{
        CallClaims, CallSemantics, CallbackInvocation, ClaimDomain, ClaimPath, ExportSemantics,
        KnowledgeSet, KnowledgeState, OperationId, StabilityKnowledge, ValueShape, ValueSource,
    };

    #[test]
    fn knowledge_states_keep_negative_proof_distinct_from_unknown() {
        assert!(!KnowledgeSet::<u8>::unknown().proves_absence());
        assert!(KnowledgeSet::<u8>::complete(vec![]).proves_absence());
        assert!(KnowledgeSet::partial(vec![1]).is_some());
        assert!(KnowledgeSet::<u8>::partial(vec![]).is_none());
    }

    #[test]
    fn semantic_queries_do_not_expose_wire_closure_or_summary_names() {
        let callback = OperationId("invoke-callback".into());
        let call = CallSemantics::new(
            CallClaims {
                callbacks: KnowledgeSet::complete(vec![CallbackInvocation {
                    from: ValueSource::Parameter {
                        index: 0,
                        path: vec!["effect".into()],
                    },
                    operation: callback.clone(),
                }]),
                writes: KnowledgeSet::complete(vec![]),
                ..CallClaims::default()
            },
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let export = ExportSemantics {
            shape: ValueShape::Callable,
            stability: StabilityKnowledge::Unknown,
            call,
        };

        assert_eq!(
            export.claim_state(ClaimDomain::Callbacks),
            KnowledgeState::CompletePositive
        );
        assert_eq!(
            export.claim_state(ClaimDomain::Writes),
            KnowledgeState::CompleteNegative
        );
        assert_eq!(export.callbacks().items()[0].operation, callback);
        assert_eq!(
            export.unresolved_call_claims(),
            vec![
                ClaimPath::Call(ClaimDomain::Reads),
                ClaimPath::Call(ClaimDomain::Creates),
                ClaimPath::Call(ClaimDomain::Invalidates),
                ClaimPath::Call(ClaimDomain::Throws),
                ClaimPath::Call(ClaimDomain::Returns),
                ClaimPath::Call(ClaimDomain::Cleanups),
                ClaimPath::Call(ClaimDomain::Disposals),
            ]
        );
    }
}
