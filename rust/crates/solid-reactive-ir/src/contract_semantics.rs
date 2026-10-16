//! Wire-independent package behavior at the analyzer trust seam.
//!
//! Compact JSON concepts such as summary names, `closed` arrays, aliases,
//! omission rules, and schema versions deliberately do not appear here. The
//! backend expands those mechanics and submits only semantic concepts to
//! [`ContractProposal::normalize`]. Validation, guard selection, recursive
//! uncertainty, and canonical semantic identity stay inside this deep module.

mod guards;

pub use crate::summary_semantics::InvokeProtocol;

use std::collections::BTreeSet;

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
}

impl<T: Ord> KnowledgeSet<T> {
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
    /// Authored result-use censuses, local to this call graph. Omission is
    /// historical behavior, never a negative claim about callback results.
    callback_results: Vec<CallbackResult>,
    /// Values retained at the enclosing factory call, not invocation arguments.
    /// Valid only on a whole returned callable's graph. Authored only.
    captures: Vec<CapturedValue>,
    /// Authored conditional slice: an own string key of a captured dictionary,
    /// with the listed factory defaults, returns that string after one ambient
    /// dictionary call. Outside the premise, callbacks/returns stay unknown.
    captured_lookup: Option<CapturedLookup>,
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
            callback_results: Vec::new(),
            captures: Vec::new(),
            captured_lookup: None,
            operations,
            edges,
            resources,
            guards,
        }
    }

    /// State the package's finite uses of each invocation result. This never
    /// asserts that a caller's implementation has the declared shape.
    #[must_use]
    pub fn with_callback_results(mut self, results: Vec<CallbackResult>) -> Self {
        self.callback_results = results;
        self
    }

    #[must_use]
    pub fn callback_results(&self) -> &[CallbackResult] {
        &self.callback_results
    }

    #[must_use]
    pub fn with_captures(mut self, captures: Vec<CapturedValue>) -> Self {
        self.captures = captures;
        self
    }

    #[must_use]
    pub fn with_captured_lookup(mut self, lookup: Option<CapturedLookup>) -> Self {
        self.captured_lookup = lookup;
        self
    }

    #[must_use]
    pub fn captured_lookup(&self) -> Option<&CapturedLookup> {
        self.captured_lookup.as_ref()
    }

    #[must_use]
    pub fn captures(&self) -> &[CapturedValue] {
        &self.captures
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
}

/// A finite, non-recursive use census for the value a caller invocation
/// produces. `shape` is an upper-bound description; consumers must substitute
/// the actual caller result before proving a negative or executing a target.
/// A partial/unknown `uses` remains an obligation even for primitive data.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallbackResult {
    pub producer: OperationId,
    pub shape: ValueShape,
    pub uses: KnowledgeSet<OperationId>,
    /// Exact uses guarded by a runtime callable test of this result value.
    /// A zero lower bound alone never supplies that negative premise.
    pub callable_only: BTreeSet<OperationId>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallbackInvocation {
    pub from: ValueSource,
    pub operation: OperationId,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ValueSource {
    /// A value from this returned graph's capture catalogue. `path` is
    /// relative to the retained value, never to an invocation argument.
    Capture {
        capture: String,
        path: Vec<String>,
    },
    Parameter {
        index: u16,
        path: Vec<String>,
    },
    /// ADR 0207: every member of the value at `index`/`path` in `class`. The
    /// item is exhaustive for that class: every invocation of such a member
    /// is its operation, whatever the domain's closure says.
    ParameterMembers {
        index: u16,
        path: Vec<String>,
        class: MemberClass,
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
pub struct CapturedValue {
    pub id: String,
    /// Exact source in the enclosing factory's graph.
    pub from: ValueSource,
}

/// ADR 0207: a class of an object's own properties a callback item names.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemberClass {
    /// Every key that starts with `on` and contains no `:`: the properties
    /// the Solid runtime's spread attaches as event listeners.
    EventHandlerProps,
}

impl MemberClass {
    /// Whether `key` is a member of the class.
    #[must_use]
    pub fn contains(self, key: &str) -> bool {
        match self {
            Self::EventHandlerProps => key.starts_with("on") && !key.contains(':'),
        }
    }

    /// The class's wire spelling.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::EventHandlerProps => "event-handler-props",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationId(pub String);

/// Finite conditional lookup recipe, not an unconditional callback census.
/// `dictionary` names one bare factory-parameter capture; `key` is an invocation
/// argument. Only omitted `default_arguments` are initially admitted. A literal
/// string key is optionally stripped of one leading dot, then an exact own data
/// string property is selected. No dictionary/result identity escapes this slice.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapturedLookup {
    pub dictionary: String,
    pub key: u16,
    pub default_arguments: Vec<u16>,
    pub strip_leading_dot: bool,
}

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
    /// (`validate::validate_result_access_operation`). A catalogued callback
    /// result use instead states its exact context and can run synchronously
    /// and untracked on a later caller's stack; it is never guessed queued.
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

/// The contract wire spelling and schema cardinality of the shared
/// [`InvokeProtocol`], which [`crate::summary_semantics`] defines.
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
            Self::GetEnumerableStringValues => "get-enumerable-string-values",
            Self::GetOwnEnumerableValues => "get-own-enumerable-values",
        }
    }

    /// Fixed occurrence bounds of the two deliberately narrow initial forms.
    /// Bounds count enumeration entries, never every selected property Get.
    #[must_use]
    pub fn enumeration_cardinality(self) -> Option<Cardinality> {
        self.is_value_enumeration().then_some(Cardinality {
            scope: Some(CardinalityScope::Call),
            min: Some(u32::from(self == Self::GetEnumerableStringValues)),
            max: Some(UpperBound::Finite(1)),
        })
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

/// An explicitly established strict-read execution context. Absence states
/// no fact about strict-read warnings, including on an untracked operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StrictRead {
    /// The read executes with the strict-read window cleared. Valid only on
    /// a read whose tracking is untracked; never inferred from tracking alone.
    Cleared,
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
    /// Only an explicit contract assertion can clear strict-read attribution.
    pub strict_read: Option<StrictRead>,
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
    /// An exact, fresh own-data object with precisely this unordered key set.
    /// Type declarations and partial property observations cannot prove it.
    OwnDataKeys {
        argument: u16,
        path: Vec<String>,
        names: Vec<String>,
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
    /// ADR 0235: a callable the export returns as a member of a tuple or
    /// object, with what one call of it does: its own call graph. Its
    /// operations share the export's id namespace (distinct from the
    /// export's own), and it may name the export's resources, which is how
    /// it states that `start` reads the signal its constructor created. Valid
    /// only as a direct member of a `return` operation's tuple or object
    /// output, never nested.
    EffectfulCallable(Box<CallSemantics>),
    /// A whole function returned by a factory. The graph's parameters are
    /// invocation arguments, never retained factory arguments. Its explicit
    /// capture catalogue supplies those retained values in a separate scope.
    /// Named function
    /// object members use the existing returned-member vocabulary. Missing
    /// graphs or members assert nothing about later dispatch. Authored only;
    /// valid only as the whole output of a factory return, never nested.
    ReturnedCallable {
        call: Option<Box<CallSemantics>>,
        members: Vec<ObjectProperty>,
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
    /// Authored-only fresh instance returned by an exact exported constructor.
    /// Members state the audited getObserver-gated TriggerCache recipe, not an
    /// unconditional reactive brand. Iterator tracks run on first resumption.
    /// Each cache is local to this one instance; argument and shared keys are
    /// distinct. A missing member never states an empty call graph.
    PrototypeInstance {
        members: Vec<PrototypeMember>,
        population: PrototypePopulation,
    },
    /// Authored Get recipe for a fresh static object. Exact named keys, or
    /// keys of the factory argument at `from`; these alternatives are exclusive.
    /// An unobserved first Get returns plain data; an observed Get creates an
    /// owned-write signal for this key and reads it. Subsequent Gets retain
    /// that signal. This does not brand the object as an unconditional store.
    /// The initial consumer proves only direct tracked Gets. It deliberately
    /// retains every untracked Get as an obligation, without guessing cache
    /// state, dominance, or the observer from the lifecycle owner.
    /// `from` additionally requires a getter-free primitive-valued literal
    /// at the consumer: arbitrary functions can select createSignal's memo
    /// overload, and arbitrary inherited/cache-prototype keys are unsafe.
    LazyGetterObject {
        keys: Vec<String>,
        from: Option<u16>,
    },
    /// ADR 0272: a direct member of a returned plain object. Each Get
    /// yields plain data and, when reactive, reads a reactive source.
    Getter {
        reactive: bool,
    },
}

/// Constructor input protocol. Opaque population admits only the zero-argument
/// slice. Values iterate one input; entries additionally spread every entry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrototypePopulation {
    Opaque,
    Values,
    Entries,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrototypeInstanceRecipe {
    pub members: Vec<PrototypeMember>,
    pub population: PrototypePopulation,
}

/// One exact prototype dispatch. `@@iterator` names the well-known iteration
/// protocol, never a string-named property or an arbitrary computed expression.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrototypeMember {
    pub name: String,
    pub kind: PrototypeMemberKind,
    pub tracks: Vec<PrototypeTrack>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PrototypeMemberKind {
    Method,
    Getter,
    Iterator,
}

/// TriggerCache.track's complete conditional behavior: absent observer does
/// nothing; present observer creates a signal if missing, always registers
/// cleanup, and always reads the signal. Signal construction alone is optional.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrototypeTrack {
    pub cache: String,
    pub argument: Option<u16>,
    pub shared: Option<String>,
}

/// The audited static-store cache is an ordinary object, not a Map or a
/// null-prototype object. A prototype collision is not a lazy signal recipe.
pub(crate) fn lazy_getter_cache_key_is_reserved(key: &str) -> bool {
    matches!(
        key,
        "__proto__"
            | "constructor"
            | "toString"
            | "toLocaleString"
            | "valueOf"
            | "hasOwnProperty"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "__defineGetter__"
            | "__defineSetter__"
            | "__lookupGetter__"
            | "__lookupSetter__"
    )
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
