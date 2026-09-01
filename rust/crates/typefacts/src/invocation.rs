//! Demand-shaped invocation proof transcripts.
//!
//! These facts are read directly from one live TypeScript-Go generation. They
//! are intentionally not retained entity-table rows: package proof callers pay
//! for callable trees and censuses, ordinary editor analysis does not.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CallKind, CallTargetSet, Callability, Declaration, Location, PrimitiveLiteralCandidate,
    ResolvedCallValidity, ResolvedDeclaration, SourceHash, TypeDescriptor, TypeFactsError,
};

pub const MAX_INVOCATION_CALLABLE_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationDomain {
    Signature,
    Bindings,
    Omissions,
    Parameters,
    Result,
    Uses,
    ControlFlow,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvocationCompleteness(pub Vec<InvocationDomain>);

impl InvocationCompleteness {
    #[must_use]
    pub fn contains(&self, domain: InvocationDomain) -> bool {
        self.0.contains(&domain)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationDemand {
    pub location: Location,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub callable_depth: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub census: bool,
}

/// One exact expression whose compiler-resolved value is needed for package
/// certification. The caller normally points this at a deterministic import
/// binding in a verifier-owned harness; the producer still resolves the exact
/// expression, alias target, declaration, and recursive value tree itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportValueDemand {
    pub location: Location,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_location: Option<Location>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub callable_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentBindingDisposition {
    Direct,
    ExactTupleSpread,
    UnknownLengthSpread,
    Unmapped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpandedArgumentSlot {
    pub expanded_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuple_index: Option<usize>,
    pub parameter_index: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormalRange {
    pub start: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_exclusive: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unbounded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgumentBinding {
    pub argument_index: usize,
    pub location: Location,
    pub disposition: ArgumentBindingDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<ExpandedArgumentSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub possible: Option<FormalRange>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub reason: Arc<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueProtocol {
    Plain,
    Promise,
    AsyncIterable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationConstructability {
    Constructable,
    NonConstructable,
    Mixed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathSegmentKind {
    Property,
    Tuple,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathSegment {
    pub kind: PathSegmentKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub property: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathPresence {
    Required,
    Optional,
    Absent,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Discriminant {
    pub property: Arc<str>,
    pub value: PrimitiveLiteralCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueAlternative {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discriminants: Vec<Discriminant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallablePathFact {
    pub alternative: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSegment>,
    pub presence: PathPresence,
    pub callability: Callability,
    pub constructability: InvocationConstructability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    pub subtree_enumerated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinitePartitionAxis {
    Literal,
    Callability,
    Protocol,
    Tuple,
    Discriminant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FiniteCase {
    pub kind: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<PrimitiveLiteralCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ValueProtocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuple_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discriminants: Vec<Discriminant>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FinitePartition {
    pub axis: FinitePartitionAxis,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<FiniteCase>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValuePrimitiveDomain {
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_string: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_number: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_boolean: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_big_int: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_symbol: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_null: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_undefined: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_object: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub numbers_finite: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unknown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationValueFact {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<TypeDescriptor>,
    pub callability: Callability,
    pub constructability: InvocationConstructability,
    pub primitive: ValuePrimitiveDomain,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<ValueAlternative>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<FinitePartition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

/// Exact compiler answer for one demanded exported-value expression.
///
/// `complete` closes expression selection, alias resolution, and declaration
/// identity only. Recursive value and callable-path closure remain local to
/// `value` and `callable_paths`; an Unknown/open leaf can never be promoted by
/// this outer bit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportValueTranscript {
    pub location: Location,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub query_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ResolvedDeclaration>,
    pub value: InvocationValueFact,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callable_paths: Vec<CallablePathFact>,
    /// The exported value's one call signature, present only when its type has
    /// exactly one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_signature: Option<SelectedSignature>,
    /// Every call signature of an overloaded exported value, in declaration
    /// order. Populated only when the type has more than one and every one of
    /// them could be described, so it is never both non-empty and paired with
    /// `call_signature`. A consumer must require its premise of *all* of them:
    /// a claim that holds for every overload holds for the export, and no
    /// single member of the set is "the" signature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub call_signatures: Vec<SelectedSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<ExportImplementationTranscript>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

/// Exact runtime implementation selected independently of the declaration
/// expression used by [`ExportValueTranscript`]. This is not an invented
/// invocation: the producer inspects the snapshot-replayed binding itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportImplementationTranscript {
    pub location: Location,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub query_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ResolvedDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<SelectedSignature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_uses: Vec<ParameterUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_flow: Option<ControlFlowCensus>,
    /// Return-carry edges owned by nested callables in this implementation.
    ///
    /// The implementation's own returns live in [`Self::control_flow`]. These
    /// rows make a second-order chain explicit: a callable already proven to
    /// execute may return another callable. No consumer may read an absent row
    /// as proof that a callable returns nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callable_returns: Vec<CallableReturnCensus>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<ImplementationCall>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterValueSource {
    pub parameter_index: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImplementationCall {
    pub location: Location,
    pub reach: Reachability,
    /// Whether the site is `f(x)` or `new F(x)`.
    ///
    /// Both are recorded, because both run the callables they are handed —
    /// `new Promise(executor)` runs its executor synchronously — so an
    /// execution premise that ignored constructions could not reach the body
    /// of one. They are not interchangeable, though: a claim that the
    /// implementation *calls* a value is not answered by a construction of it,
    /// so every consumer whose witness says "call" checks this first.
    ///
    /// An *absent* kind deserializes to [`CallKind::Unknown`], which those
    /// consumers refuse: absence is never read as "call". An *unrecognized*
    /// kind is not mapped to `Unknown` at all — [`CallKind`] carries no
    /// `#[serde(other)]` arm, so deserialization fails and the whole transcript
    /// is rejected. That is deliberately the harder of the two failures: a
    /// producer that invented a third kind is a producer this side does not
    /// understand, and reading its census as a set of unknown-kind sites would
    /// keep every *other* field of those sites in play.
    #[serde(default = "unknown_call_kind")]
    pub kind: CallKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target_module: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ResolvedDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callee_parameter: Option<ParameterValueSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argument_parameters: Vec<Option<ParameterValueSource>>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub captured: bool,
    /// The exact source range of the *innermost* callable containing this call,
    /// absent when the call sits directly in the implementation's own body.
    /// `captured` is true exactly when this is present.
    ///
    /// It is what lets an execution premise compose rather than assume. Knowing
    /// only that a call's bytes sit somewhere inside a carried closure says
    /// nothing about the callables in between, which may have been stored in a
    /// registry and never run; knowing which callable immediately contains it
    /// makes every link of the chain a claim that has to be proven on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_callable: Option<Location>,
    /// Per argument slot, the exact source ranges of the callables that slot
    /// provably carries, and carries by identity — the callable expression
    /// itself, the wrappers that erase at runtime, and a single-declaration
    /// binding naming exactly one callable. A value that bundles several
    /// callables is deliberately absent: an invoking slot whose runtime picks
    /// one named member does not run the rest. An empty list is never proof
    /// that a slot carries nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argument_callables: Vec<ImplementationArgumentCallable>,
    /// The reviewed standard-library member this call's callee resolves to, by
    /// default-library symbol identity rather than by spelling. An unrecognized
    /// string is not a member of the reviewed table and must be refused, which
    /// is why the wire form stays a string and
    /// [`DefaultLibraryInvoker::from_wire`] is the only way in.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub default_library_invoker: Arc<str>,
    /// The argument slots the named member's runtime invokes zero or more
    /// times.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invoked_arguments: Vec<usize>,
    /// Parameter indices the callee calls directly in its own body — the only
    /// one of the three that by itself says the position is used as a function.
    ///
    /// "In its own body" excludes every callable nested inside it and every
    /// statement its own control flow cannot reach, so a parameter called from
    /// a closure the callee merely stores is credited to nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callee_directly_called_parameters: Vec<usize>,
    /// Parameter indices the callee's body sends to *some* proven invoking
    /// position, a reviewed default-library invoker included. This says the
    /// value runs; it does not say the callee calls it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callee_invoked_parameters: Vec<usize>,
    /// Parameter indices whose forwarding chain is a plain identifier forward
    /// at every hop and terminates in a direct call. A chain that ends at
    /// `addEventListener` is invoked but not strongly invoked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callee_strongly_invoked_parameters: Vec<usize>,
    /// The same two claims, each still missing one premise the producer may not
    /// decide: whether a named argument slot of a named imported function is a
    /// callback position. The callee's body calls its parameter from inside a
    /// callable it hands to that slot, so the claim holds exactly when the slot
    /// invokes what it is given.
    ///
    /// The producer states the syntax and refuses to state the semantics: it
    /// knows no framework vocabulary, and reading one out of a module and a
    /// name is the shortcut the precision contract forbids. This side owns that
    /// table and answers each requirement itself, so an entry whose premises it
    /// does not recognize proves nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callee_pending_invocations: Vec<CalleePendingInvocation>,
}

/// One conditional callee-parameter claim: parameter `parameter` of this call's
/// callee is invoked — and, when `strong`, invoked by a chain of plain forwards
/// terminating in a direct call — provided every slot in `requires` really does
/// invoke the callable handed to it.
///
/// An entry with no requirements is not an unconditional claim, it is a
/// malformed one: the unconditional claims travel in the index lists above.
/// Consumers refuse it rather than reading it as a fact that needs nothing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalleePendingInvocation {
    pub parameter: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strong: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<InvokingSlotPremise>,
}

/// One argument slot of one resolved imported callee, exactly as the source
/// spells it — the module it was imported from, the name it was exported under,
/// the slot, and the call's argument count. It is everything a dialect owner
/// needs to answer "does this position run what it is given", and nothing that
/// presumes the answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvokingSlotPremise {
    pub module: Arc<str>,
    pub name: Arc<str>,
    pub slot: usize,
    pub argument_count: usize,
}

fn unknown_call_kind() -> CallKind {
    CallKind::Unknown
}

/// One argument slot of a call bound to the exact source ranges of the
/// callables it provably carries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImplementationArgumentCallable {
    pub argument: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
}

/// The closed set of standard-library members the producer will vouch for as
/// invoking one of their arguments, and the slots each one invokes.
///
/// The verifier owns this table as well as the producer, and both must agree
/// before a slot counts. That is not redundancy: the wire value is a string
/// from another process, and a member nobody here reviewed — or a slot list
/// wider than the reviewed one — is not evidence. `from_wire` refuses an
/// unrecognized name outright, and `invokes` answers from this table rather
/// than from the transmitted list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultLibraryInvoker {
    SetTimeout,
    SetInterval,
    QueueMicrotask,
    RequestAnimationFrame,
    RequestIdleCallback,
    AddEventListener,
    PromiseThen,
    PromiseCatch,
    PromiseFinally,
    ArrayIteration,
    /// The one construct-expression row. `new Promise(executor)` runs its
    /// executor synchronously, before the constructor returns.
    PromiseConstructor,
}

impl DefaultLibraryInvoker {
    /// The reviewed member this wire value names, or `None` for anything else.
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "setTimeout" => Some(Self::SetTimeout),
            "setInterval" => Some(Self::SetInterval),
            "queueMicrotask" => Some(Self::QueueMicrotask),
            "requestAnimationFrame" => Some(Self::RequestAnimationFrame),
            "requestIdleCallback" => Some(Self::RequestIdleCallback),
            "addEventListener" => Some(Self::AddEventListener),
            "promiseThen" => Some(Self::PromiseThen),
            "promiseCatch" => Some(Self::PromiseCatch),
            "promiseFinally" => Some(Self::PromiseFinally),
            "arrayIteration" => Some(Self::ArrayIteration),
            "promiseConstructor" => Some(Self::PromiseConstructor),
            _ => None,
        }
    }

    /// Whether this member's runtime invokes the value at `argument`.
    #[must_use]
    pub fn invokes(self, argument: usize) -> bool {
        match self {
            Self::SetTimeout
            | Self::SetInterval
            | Self::QueueMicrotask
            | Self::RequestAnimationFrame
            | Self::RequestIdleCallback
            | Self::PromiseCatch
            | Self::PromiseFinally
            | Self::ArrayIteration
            | Self::PromiseConstructor => argument == 0,
            Self::AddEventListener => argument == 1,
            Self::PromiseThen => argument == 0 || argument == 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImplementationValueSourceKind {
    DirectCallable,
    CallResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImplementationValueSource {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSegment>,
    pub kind: ImplementationValueSourceKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target_name: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target_module: Arc<str>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_path: Vec<PathSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredTypeReference {
    pub name: Arc<str>,
    pub module: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedParameter {
    pub index: usize,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub defaulted: bool,
    pub value: InvocationValueFact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_type: Option<DeclaredTypeReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callable_paths: Vec<CallablePathFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedSignature {
    pub identity: Arc<str>,
    pub declaration: ResolvedDeclaration,
    pub overload_ordinal: usize,
    pub overload_count: usize,
    pub minimum_argument_count: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_rest: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<SelectedParameter>,
    pub result: InvocationValueFact,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_callable_paths: Vec<CallablePathFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterUseKind {
    DirectCall,
    AliasCall,
    ArgumentKnown,
    ArgumentUnknown,
    PropertyAccess,
    Return,
    Storage,
    Capture,
    UnknownEscape,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterUse {
    pub parameter_index: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_path: Vec<PathSegment>,
    pub location: Location,
    /// Whether invoking the implementation reaches this use, answered by the
    /// same body walk that answers it for a call in the same position. A use
    /// after a `return` or a `throw`, or in a branch a literal condition
    /// excludes, is [`Reachability::Unreachable`]; a use inside a loop body,
    /// a `switch`, or a `try` is [`Reachability::Unknown`].
    pub reach: Reachability,
    pub kind: ParameterUseKind,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alias: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub captured: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reachability {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReturnSite {
    pub location: Location,
    pub reach: Reachability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<InvocationValueFact>,
    /// Exact source ranges of the callables this returned value provably
    /// carries. A call inside a nested callable is reachable through the
    /// returned value exactly when its location lies within one of these
    /// ranges; an empty list is never proof that nothing is carried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carried_callables: Vec<Location>,
    /// Lower-bound strength of this value-return edge. Absence carries no
    /// authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carry_reach: Option<Reachability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<ImplementationValueSource>,
}

/// Exact return-carry rows for one nested callable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableReturnCensus {
    pub callable: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returns: Vec<CallableReturnCarrySite>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableReturnCarrySite {
    pub location: Location,
    pub reach: Reachability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carry_reach: Option<Reachability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carried_callables: Vec<CallableCarryBinding>,
}

/// One exact callable carried by a nested callable's returned value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallableCarryBinding {
    pub location: Location,
    pub reach: Reachability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThrowSite {
    pub location: Location,
    pub reach: Reachability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchSite {
    pub location: Location,
    pub reach: Reachability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<FinitePartition>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlFlowCensus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returns: Vec<ReturnSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throws: Vec<ThrowSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<BranchSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationTranscript {
    pub location: Location,
    pub validity: ResolvedCallValidity,
    pub kind: CallKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<CallTargetSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_signature: Option<SelectedSignature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<ArgumentBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_parameters: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_uses: Vec<ParameterUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_flow: Option<ControlFlowCensus>,
    #[serde(rename = "complete", default)]
    pub completeness: InvocationCompleteness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptSourceDigest {
    pub path: Arc<str>,
    pub sha256: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationEnvelope {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub project_id: Arc<str>,
    pub generation: u64,
    pub demand_sha256: Arc<str>,
    pub module_graph_sha256: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub schema_sha256: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub producer_build: Arc<str>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<TranscriptSourceDigest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationAnswer {
    pub transcripts: Vec<InvocationTranscript>,
    pub envelope: InvocationEnvelope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportValueAnswer {
    pub transcripts: Vec<ExportValueTranscript>,
    pub envelope: InvocationEnvelope,
}

/// Verifier-owned identity that a certification invocation must be bound to.
///
/// This value is deliberately absent from the Type Facts wire model. The live
/// Rust session adds it only after it has received and validated a response
/// from the process it launched. A serialized [`InvocationAnswer`] therefore
/// cannot be promoted back into certification authority by copying these
/// strings into JSON.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertificationInvocationContext {
    snapshot_root: SourceHash,
    demand_graph_root: SourceHash,
    proof_demand_ids: Vec<SourceHash>,
}

impl CertificationInvocationContext {
    pub fn new(
        snapshot_root: impl Into<String>,
        demand_graph_root: impl Into<String>,
        proof_demand_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, TypeFactsError> {
        let snapshot_root = SourceHash::parse(snapshot_root)?;
        let demand_graph_root = SourceHash::parse(demand_graph_root)?;
        let mut proof_demand_ids = proof_demand_ids
            .into_iter()
            .map(SourceHash::parse)
            .collect::<Result<Vec<_>, _>>()?;
        proof_demand_ids.sort();
        if proof_demand_ids.is_empty() || proof_demand_ids.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(TypeFactsError::InvalidCertificationContext);
        }
        Ok(Self {
            snapshot_root,
            demand_graph_root,
            proof_demand_ids,
        })
    }

    #[must_use]
    pub fn snapshot_root(&self) -> &str {
        self.snapshot_root.as_str()
    }

    #[must_use]
    pub fn demand_graph_root(&self) -> &str {
        self.demand_graph_root.as_str()
    }

    #[must_use]
    pub fn proof_demand_ids(&self) -> impl ExactSizeIterator<Item = &str> {
        self.proof_demand_ids.iter().map(SourceHash::as_str)
    }
}

/// Identity of the exact live producer response used for certification.
///
/// There is intentionally no serde implementation and no public constructor.
/// Only [`crate::Session::certification_invocations`] can create this token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProducerSessionIdentity {
    pub(crate) session_id: SourceHash,
    pub(crate) restart_epoch: u64,
    pub(crate) process_id: u32,
    pub(crate) executable_sha256: SourceHash,
    pub(crate) source_manifest_sha256: SourceHash,
    pub(crate) handshake_protocol: u64,
    pub(crate) handshake_schema_sha256: SourceHash,
    pub(crate) handshake_build: Arc<str>,
    pub(crate) project_id: Arc<str>,
    pub(crate) generation: u64,
    pub(crate) demand_sha256: SourceHash,
    pub(crate) context: CertificationInvocationContext,
    pub(crate) evidence_root: SourceHash,
}

impl LiveProducerSessionIdentity {
    #[must_use]
    pub fn session_id(&self) -> &str {
        self.session_id.as_str()
    }

    #[must_use]
    pub const fn restart_epoch(&self) -> u64 {
        self.restart_epoch
    }

    #[must_use]
    pub const fn process_id(&self) -> u32 {
        self.process_id
    }

    #[must_use]
    pub fn executable_sha256(&self) -> &str {
        self.executable_sha256.as_str()
    }

    #[must_use]
    pub fn source_manifest_sha256(&self) -> &str {
        self.source_manifest_sha256.as_str()
    }

    #[must_use]
    pub const fn handshake_protocol(&self) -> u64 {
        self.handshake_protocol
    }

    #[must_use]
    pub fn handshake_schema_sha256(&self) -> &str {
        self.handshake_schema_sha256.as_str()
    }

    #[must_use]
    pub fn handshake_build(&self) -> &str {
        &self.handshake_build
    }

    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn demand_sha256(&self) -> &str {
        self.demand_sha256.as_str()
    }

    #[must_use]
    pub const fn context(&self) -> &CertificationInvocationContext {
        &self.context
    }

    #[must_use]
    pub fn evidence_root(&self) -> &str {
        self.evidence_root.as_str()
    }
}

/// A response obtained directly from one pinned live producer process.
///
/// The ordinary answer remains available for audit, but the non-serializable
/// identity token is what lets the backend reject copied responses and
/// cross-session or cross-restart splicing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveInvocationAnswer {
    pub(crate) answer: InvocationAnswer,
    pub(crate) identity: LiveProducerSessionIdentity,
}

impl LiveInvocationAnswer {
    #[must_use]
    pub const fn answer(&self) -> &InvocationAnswer {
        &self.answer
    }

    #[must_use]
    pub const fn identity(&self) -> &LiveProducerSessionIdentity {
        &self.identity
    }
}

/// Authority-bearing answer for the distinct exported-value operation. It
/// deliberately cannot be converted from or to [`LiveInvocationAnswer`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveExportValueAnswer {
    pub(crate) answer: ExportValueAnswer,
    pub(crate) identity: LiveProducerSessionIdentity,
}

impl LiveExportValueAnswer {
    #[must_use]
    pub const fn answer(&self) -> &ExportValueAnswer {
        &self.answer
    }

    #[must_use]
    pub const fn identity(&self) -> &LiveProducerSessionIdentity {
        &self.identity
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
}

const fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}
