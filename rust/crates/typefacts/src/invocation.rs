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
    /// Asks for an implementation transcript of the function-like declaration
    /// whose source range is *exactly* this location, inside the analyzed
    /// snapshot.
    ///
    /// It is the only way to reach a declaration no export names.
    /// [`Self::implementation_location`] starts from an identifier and
    /// resolves through the export's runtime binding, so a module-local helper
    /// — which is most of what an implementation census recurses into — is
    /// unreachable through it.
    ///
    /// The location names the **declaration node**, not an identifier, and the
    /// match is exact in both bytes. The producer refuses by open reason and
    /// never by answering about a different declaration: `sourceUnavailable`
    /// when the accepted program resolved no file at that path or the byte
    /// range is invalid, `declarationOutsideSnapshot` when the file is in the
    /// program but carries no runtime bytes (every `lib.*.d.ts` and every
    /// dependency `.d.ts` is a program file, and none of them is snapshot
    /// source), `declarationNotExact` when no node has exactly that span or
    /// the node that does is not function-like, `declarationAmbiguous` when
    /// more than one function-like node does, `implementationUnavailable` when
    /// the declaration has no body, `symbolUnresolved` for an anonymous
    /// callable whose name cannot be recovered from an enclosing variable
    /// declaration, and `declarationIdentityUnbound` when the declaration the
    /// checker resolves does not sit inside the demanded span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_declaration_location: Option<Location>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub callable_depth: usize,
    /// The premise the local declaration's uncensused-form census is asked to
    /// classify under: each named parameter bound to the given type, which is
    /// the type the **caller's** premised census found in that argument slot
    /// at the call that reached this declaration
    /// ([`ExportImplementationTranscript::call_argument_premises`]), copied
    /// back verbatim (handshake protocol 23, ADR 0038). Meaningful only beside
    /// [`Self::local_declaration_location`]; the producer refuses a demand
    /// stating it for anything else, because an export's root has a declared
    /// signature to bind. Indexes are strictly increasing. The answer's
    /// [`ExportImplementationTranscript::parameter_premises`] either equals
    /// this list — every entry re-established on the callee's twin — or is
    /// empty, the strictly more refusing census; `Session::export_values`
    /// refuses anything in between.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_premises: Vec<ParameterPremise>,
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
    /// True for a member the value carries only through the compiler's
    /// apparent-type augmentation: the global `Function` interface's members on
    /// a node with call or construct signatures. Such a member exists — the
    /// compiler answers it and `tsc` type-checks the access — but it is
    /// library-owned rather than declared by the package under analysis, it is
    /// never caller-supplied, and the producer emits it as a leaf. Declared
    /// members carry false.
    ///
    /// Declared-member census closure excludes these facts; exact path lookups
    /// still find them. There is deliberately no `serde(default)`: a producer
    /// that omits the field is rejected rather than defaulted, because
    /// defaulting it to false would silently turn every apparent leaf into a
    /// declared census member.
    pub apparent: bool,
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
    /// The answer to [`ExportValueDemand::local_declaration_location`],
    /// present exactly when that field was set.
    ///
    /// Its [`ExportImplementationTranscript::location`] is the demanded
    /// location **verbatim**, which is precisely why that field cannot be the
    /// binding on its own: a producer answering about a different helper would
    /// echo the demand just the same. What
    /// [`crate::Session::export_values`] actually requires, and what a caller
    /// may rely on, is:
    ///
    /// - presence agrees with the demand — a `local_declaration` for a demand
    ///   that asked for none, and its absence for a demand that asked for one,
    ///   are both refused;
    /// - `location` equals the demanded location (the echo, which catches a
    ///   producer that answered a *different demand of the same batch*, since
    ///   the batch's demands differ there);
    /// - when [`ExportImplementationTranscript::declaration`] is present, its
    ///   location names the demanded file and lies inside the demanded span.
    ///   This is the non-echoed half: the producer derives it from the located
    ///   declaration's own symbol, not from the demand. It is a containment
    ///   rather than an equality because a named function resolves to its
    ///   *identifier* while an anonymous `const helper = () => …` resolves to
    ///   the arrow itself;
    /// - when both are populated, `query_name` and the resolved declaration's
    ///   `name` agree.
    ///
    /// None of that makes a *well-formed* answer about another helper in
    /// another file impossible to construct — a producer is trusted for the
    /// contents of the body it censuses, exactly as it is for an export's. It
    /// makes an answer whose own two identity fields disagree, or which
    /// describes a declaration outside the bytes that were asked about,
    /// refusable without reading the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_declaration: Option<ExportImplementationTranscript>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

/// What a function-like implementation hands its caller when it completes,
/// classified from the `async` modifier and the asterisk token on the
/// declaration itself (ADR 0035). A plain callable completes with whatever its
/// return sites carry; an `async` function always hands back a promise, a
/// generator an iterator, an async generator an async iterator, whatever the
/// body does. `Unclassified` is the producer's default for a declaration kind
/// its classifier does not review, and a consumer refuses it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImplementationCompletionForm {
    Plain,
    Async,
    Generator,
    AsyncGenerator,
    Unclassified,
}

/// Exact runtime implementation selected independently of the declaration
/// expression used by [`ExportValueTranscript`]. This is not an invented
/// invocation: the producer inspects the snapshot-replayed binding itself.
/// One parameter's type binding under which an implementation's
/// uncensused-form census was classified (ADR 0038). See
/// [`ExportImplementationTranscript::parameter_premises`].
///
/// `identity` is the producer's own binding of the text to a declaration —
/// type flags and declaration positions — stated on a call-argument premise
/// and echoed on the demand and the callee's premise so that a spelling which
/// resolves to a *different* declaration of the same name on the callee's twin
/// is refused by the producer rather than bound. A consumer compares it byte
/// for byte and reads nothing into it; a root premise, bound to the declared
/// signature by its text alone, carries none.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterPremise {
    pub index: usize,
    pub r#type: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub identity: Arc<str>,
}

/// The type each informative written argument slot of one call carried on the
/// twin a premised census ran over (handshake protocol 23). See
/// [`ExportImplementationTranscript::call_argument_premises`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallArgumentPremise {
    pub call: Location,
    pub arguments: Vec<ParameterPremise>,
}

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
    /// How this implementation completes to its caller, from its own syntax
    /// (ADR 0035, handshake protocol 19). Stated beside [`Self::declaration`]
    /// on every transcript that reached its implementation; a consumer never
    /// reads its absence as [`ImplementationCompletionForm::Plain`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_form: Option<ImplementationCompletionForm>,
    /// The declaration whose body this transcript's census walked when
    /// [`Self::declaration`] is a binding that aliases it by identity --
    /// `const defaultScheduler = systemSetTimeoutZero` (handshake protocol
    /// 20). Absent when the body is the declaration's own. The producer states
    /// it only for an exact alias: an identifier initializer, the binding never
    /// assigned in its file, the aliased function never assigned in its own;
    /// an import that module resolution took to a sibling `.d.ts` is followed
    /// to the runtime module the specifier denotes and to that module's export
    /// of the imported name, which is what the import binds at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_of: Option<ResolvedDeclaration>,
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
    /// Every syntactic position in this implementation that can invoke user
    /// code and that [`Self::calls`] does *not* record.
    ///
    /// `calls` holds `CallExpression` and `NewExpression` only. Everything
    /// else that reaches a callable — a tagged template, an accessor behind a
    /// property access, the iteration protocol, a decorator, `Symbol.dispose`,
    /// `Symbol.hasInstance`, a thenable's `then`, a coercion, a JSX lowering —
    /// arrives here instead, so a consumer that must enumerate every invoking
    /// form refuses **by name** rather than concluding from silence.
    ///
    /// **An absent field and a present empty one are different facts, and no
    /// consumer may conflate them.** A present empty list is the producer's
    /// positive claim that every form it walked was a call, a construction, or
    /// a node kind on its reviewed list of kinds that provably cannot invoke
    /// user code. An absent list is a producer that never classified anything,
    /// and the census must refuse. Serde cannot tell them apart here — the
    /// field defaults to empty — so the discriminator is the **handshake
    /// protocol**: a producer at or above the protocol that introduced this
    /// field always *classified every form it walked*, whether or not the
    /// resulting list reaches the wire (the field is `omitempty` there, so an
    /// empty list is encoded as nothing at all), and
    /// [`crate::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL`] is compared
    /// field-for-field before any transcript is read. A census must therefore
    /// establish the protocol, not inspect the emptiness.
    ///
    /// [`Self::complete`] says nothing about this field and this field says
    /// nothing about `complete`; see `complete`'s own doc comment for the
    /// seven gates it actually asserts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncensused_invoking_forms: Vec<UncensusedInvokingForm>,
    /// The declared-signature premise [`Self::uncensused_invoking_forms`] was
    /// classified under, when the producer bound one (ADR 0038, handshake
    /// protocol 22): one entry per parameter of this implementation, in
    /// position order, each naming the type the export's *declared* call
    /// signature gives that position — printed byte-identically to the
    /// corresponding `SelectedParameter.value.type.text` of the export's own
    /// transcript, which is how a consumer binds the premise to the signature
    /// it already holds and the synthesized veto already samples from.
    ///
    /// Present, the forms were classified with the implementation's parameters
    /// carrying those types instead of the implicit `any` an unannotated
    /// JavaScript parameter has, and a consumer that closes a domain on the
    /// resulting census records every entry as a condition of the closure.
    /// Absent, the forms were classified over the parameters' own types, the
    /// strictly more refusing reading; an empty list is never a premise. On
    /// the export's root implementation the entries are the declared
    /// signature's types; on a local declaration's transcript (protocol 23)
    /// they equal the demand's [`ExportValueDemand::parameter_premises`],
    /// which the consumer copied from the caller's
    /// [`Self::call_argument_premises`] for the call it followed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_premises: Vec<ParameterPremise>,
    /// Why the producer, holding a declared signature and a form a type could
    /// have cleared, stated no premise. Diagnostic only; a consumer decides
    /// nothing from it and reads nothing into its absence.
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub parameter_premise_refusal: Arc<str>,
    /// For each call or construction the **premised** census walked whose
    /// callee is an identifier resolving to a declaration in the program's own
    /// runtime source, the type the twin's checker gave each informative
    /// written argument slot (handshake protocol 23). This is how a premise
    /// reaches a local helper: a consumer following the call copies the entry
    /// into the helper's [`ExportValueDemand::parameter_premises`]. Stated only
    /// beside a nonempty [`Self::parameter_premises`], only for a call with no
    /// spread, and only for the slots whose type is not `any`; each `call` is
    /// the location of a row of [`Self::calls`]. A consumer refuses an entry
    /// that names no row and any entry on an unpremised transcript.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub call_argument_premises: Vec<CallArgumentPremise>,
    /// The conjunction of seven independent gates, every one of which the
    /// producer clears before setting this — and nothing else.
    ///
    /// In the producer's own order
    /// (`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`,
    /// `exportImplementationTranscriptLocked`), each failed gate appends the
    /// named open reason and returns instead:
    ///
    /// 1. the queried node is an **exact identifier** — `identifierNotExact`;
    /// 2. `GetSymbolAtLocation` **resolves that identifier** —
    ///    `symbolUnresolved`;
    /// 3. the alias chain resolves to a **canonical target** —
    ///    `aliasUnresolved`;
    /// 4. the value's type has **exactly one call signature** —
    ///    `callSignatureNotUnique`;
    /// 5. the selected signature has an implementation declaration **with an
    ///    available body** — `implementationUnavailable`;
    /// 6. that implementation has a **resolved declaration** —
    ///    `declarationUnavailable`;
    /// 7. the control-flow census has **no unsupported branch** —
    ///    `controlFlowUnsupported`.
    ///
    /// (`sourceUnavailable` precedes all seven: without the source file there
    /// is no node to query.)
    ///
    /// **`complete` says nothing about the calls census being total.** Gate 7
    /// is about control flow, not about invoking forms, and
    /// [`Self::calls`] records `CallExpression` and `NewExpression` only. A
    /// transcript can be `complete: true` while a tagged template, an
    /// accessor, or a JSX lowering in its body invokes code that appears in no
    /// `calls` row. The enumeration guarantee lives entirely in
    /// [`Self::uncensused_invoking_forms`], and a census that read `complete`
    /// as one would be unsound.
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

/// One syntactic position that can invoke user code and that the
/// implementation call census does not record.
///
/// Two invoking forms are deliberately absent from [`UncensusedInvokingFormKind`]:
///
/// - **A `Proxy` trap.** A trap belongs to the object a value happens to be at
///   runtime, not to any syntax, so no walk of an implementation can see it:
///   `obj.x` on a proxy is the same `PropertyAccessExpression` as `obj.x` on a
///   plain object. It is out of the producer's reach entirely, and a marker for
///   it would claim a census the producer cannot perform. What the producer can
///   say is that a member it could not resolve is unresolved, which is
///   [`UncensusedInvokingFormKind::PropertyAccessUnknownAccessor`] — a
///   statement about the checker's knowledge, not about proxies. A consumer
///   whose claim requires that no trap ran must obtain that premise elsewhere.
/// - **An optional call, `f?.(x)`.** It is already a `CallExpression`, so the
///   call census records it like any other call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncensusedInvokingForm {
    /// The closed vocabulary. An **unrecognized** string is not mapped to a
    /// catch-all: [`UncensusedInvokingFormKind`] carries no `#[serde(other)]`
    /// arm, so deserialization fails and the whole transcript is rejected —
    /// exactly as an unrecognized [`crate::CallKind`] is (see
    /// [`ImplementationCall::kind`]). A producer that invented a kind is a
    /// producer this side does not understand, and reading its census as a set
    /// of unknown-kind rows would keep every *other* field of those rows in
    /// play. There is also no `Unknown` default: unlike `kind` on a call, an
    /// absent kind here is a malformed row rather than a degraded one.
    pub kind: UncensusedInvokingFormKind,
    /// The compiler's own name for the node's syntax kind, with the `Kind`
    /// prefix removed. Always populated, and the only description of the form
    /// that exists for
    /// [`UncensusedInvokingFormKind::UnclassifiedInvokingForm`].
    pub node_kind: Arc<str>,
    pub location: Location,
    /// From the same walk, and therefore the same reachability notion, as
    /// [`ImplementationCall::reach`].
    ///
    /// This census applies no jump withholding, unlike the call census. There,
    /// dropping a row keeps an over-optimistic `Reach` off the wire; here, a
    /// dropped row is silence, which is the failure this census exists to
    /// prevent, and an over-optimistic `Reach` can only make a consumer refuse
    /// a form that might not have run.
    pub reach: Reachability,
    /// The exact source range of the innermost callable containing this form,
    /// absent when the form sits directly in the implementation's own body.
    /// [`Self::captured`] is true exactly when this is present. Same
    /// discipline, and same reason, as [`ImplementationCall::enclosing_callable`]:
    /// lexical containment in a closure is not execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosing_callable: Option<Location>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub captured: bool,
    /// The parameter of the transcript's own declaration at which this form's
    /// subject — the receiver of a property or element access — is rooted
    /// through a chain of property and element reads (ADR 0034, handshake
    /// protocol 18; extended to write position by ADR 0040 at protocol 24).
    /// Stated only for a `get-accessor`, `set-accessor` or
    /// `property-access-unknown-accessor` form whose root is a plain,
    /// uninitialized, non-rest parameter binding written nowhere in its file,
    /// in a declaration mentioning neither `arguments` nor `eval`.
    ///
    /// **Absence is never "not rooted."** A protocol-17 producer states nothing
    /// here for a rooted form, so a consumer that reads this field must
    /// require protocol 18 first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_parameter: Option<usize>,
    /// Whether the access this form's [`Self::subject_parameter`] roots is in
    /// **write** position — an assignment target, a compound assignment, or an
    /// update expression — so the accessor that may run is a setter, and a
    /// compound or update form runs the getter too (ADR 0040, handshake
    /// protocol 24).
    ///
    /// The position is stated rather than folded into the subject because the
    /// two are one fact for `creates` — the accessor is the caller's code
    /// either way — and different facts for `writes` and `invalidates`, where
    /// the write is this export's own act. A consumer that cannot tell them
    /// apart must not read the subject, which is why the handshake moves.
    #[serde(default, skip_serializing_if = "is_false")]
    pub subject_write: bool,
}

/// The closed vocabulary of invoking forms the call census does not record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UncensusedInvokingFormKind {
    /// A `TaggedTemplateExpression`: the tag is invoked with the strings array
    /// and the substitutions. An `html` template counts here and nowhere else.
    TaggedTemplate,
    /// A property access, element access, or destructured member the checker
    /// resolved to a symbol with a get-accessor declaration, in a position
    /// that reads it.
    GetAccessor,
    /// The same, with a set-accessor declaration, in an assignment target
    /// position.
    SetAccessor,
    /// A member the producer cannot answer for. Three ways that happens: the
    /// checker resolved **no symbol** at all (an `any`-typed receiver, a
    /// computed key, an index signature, an element access whose key is not an
    /// exact literal); it resolved declarations that are **not the snapshot's
    /// runtime bytes**, such as a `.d.ts` `readonly value` that may perfectly
    /// well describe a `.js` getter (the default library excepted, since it
    /// describes the engine rather than user code); or the form reads every
    /// own enumerable property of a value whose shape is not statically known
    /// — object spread, JSX prop spread, and an object rest element.
    ///
    /// It is recorded rather than dropped because the producer genuinely
    /// cannot tell whether the member is an accessor: with no symbol there are
    /// no declarations to inspect, with only a declaration file there are no
    /// bytes, and neither absence is evidence of a plain data property. A
    /// member the checker *does* resolve, to runtime declarations none of
    /// which is an accessor, invokes nothing and is recorded nowhere.
    PropertyAccessUnknownAccessor,
    /// A `Decorator` application: the decorator expression is invoked when the
    /// decorated declaration is evaluated.
    Decorator,
    /// A position that reaches `Symbol.iterator` or `Symbol.asyncIterator` and
    /// then the iterator's own `next`/`return`: `for…of`, `for await…of`, a
    /// spread element, an array binding pattern, an array **assignment**
    /// pattern (`[a, b] = src`, which is an `ArrayLiteralExpression` the
    /// compiler reinterprets), and `yield*`.
    IterationProtocol,
    /// A `using` or `await using` declaration list: scope exit reaches
    /// `Symbol.dispose` or `Symbol.asyncDispose` on every declared value.
    UsingDispose,
    /// An `instanceof` operator, which reaches `Symbol.hasInstance` on its
    /// right operand when that operand defines it. Whether it does is a
    /// property of the runtime value, so the form is always recorded.
    #[serde(rename = "instanceof")]
    InstanceOf,
    /// An `await` whose operand is not provably resolved by the engine alone.
    /// Awaiting a thenable invokes that object's own `then`, so the form is
    /// recorded unless **every** constituent of the operand's type is either a
    /// primitive — which has no `then` to call — or a default-library
    /// `Promise`, whose `then` is the engine's own. A `PromiseLike` *is*
    /// recorded, because its `then` is whatever the value carries.
    ///
    /// "The type declares no `then`" is deliberately *not* a reason to stay
    /// silent: a union missing it in one constituent carries it in another, an
    /// unconstrained type parameter has no members the checker can enumerate,
    /// and an index-signature type such as `Record<string, unknown>` declares
    /// none while permitting one at runtime, whose `Get` would reach a getter
    /// and whose value `await` would call.
    AwaitThen,
    /// A template expression or an operator application whose operand is not
    /// provably a non-object, so evaluating it may reach `Symbol.toPrimitive`,
    /// `valueOf`, or `toString`. `any`, `unknown`, and a type parameter are
    /// *not* provably non-object and are recorded.
    Coercion,
    /// A JSX element, self-closing element, or fragment. Its compiler lowering
    /// invokes a component or an accessor, and the producer records neither the
    /// lowering nor what it invokes.
    JsxElement,
    /// The catch-all, and the row this whole field exists for: a node kind
    /// that is neither classified above nor on the producer's reviewed list of
    /// kinds that provably cannot invoke user code.
    /// [`UncensusedInvokingForm::node_kind`] carries the compiler's own name
    /// for it.
    ///
    /// A consumer refuses on this row unconditionally. There is nothing else
    /// it could soundly do with a form nobody has classified, and a kind a
    /// future compiler revision adds arrives here rather than passing in
    /// silence.
    UnclassifiedInvokingForm,
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
    /// The parameter-rooted iterable whose iteration produced this call's
    /// callee — the binding a `for…of` head declares, called inside the loop
    /// (ADR 0042, handshake protocol 26).
    ///
    /// What the caller's iterable yields is the caller's, so calling it runs
    /// the caller's code exactly as calling a parameter does. The producer
    /// states it only for a plain non-`await` `for…of` whose head declares
    /// this one binding, which nothing writes, over an expression rooted at a
    /// parameter of the censused declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callee_iterated_parameter: Option<ParameterValueSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argument_parameters: Vec<Option<ParameterValueSource>>,
    /// For a `.call` or `.apply` whose resolved callee is the default library's
    /// `Function.prototype` member: the resolved declaration of the *receiver*
    /// expression, e.g. `Object.prototype.toString` in
    /// `Object.prototype.toString.call(value)` (ADR 0034, protocol 18). A
    /// consumer may decide such a site by the receiver instead of refusing every
    /// by-reference transfer alike, but only for receivers it has reviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_receiver: Option<ResolvedDeclaration>,
    /// Beside `call_receiver`: the parameter of this declaration the `this`
    /// argument (slot 0) is rooted at, under exactly the premises of
    /// [`UncensusedInvokingForm::subject_parameter`]. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub this_parameter: Option<usize>,
    /// Per written argument slot, the traced value provenance of the expression
    /// written there — the same trace [`ReturnSite::sources`] carries for a
    /// returned expression. Parallel to `argument_parameters` and gated the
    /// same way, so a slot a spread has displaced carries an empty list rather
    /// than a trace of the expression written at that position.
    ///
    /// **An empty list means the producer traced nothing.** It is never "this
    /// argument is not an accessor", never "this argument is plain", and never
    /// a claim about the slot at all: the tracer follows array literals,
    /// callable expressions, call results and one hop through a
    /// single-assignment array binding element, and every other expression — a
    /// conditional, a property read, a reassigned or redeclared binding, a rest
    /// or defaulted element, a computed callee — leaves the slot empty. A short
    /// list is the same absence: a consumer reads the slot it wants and fails
    /// closed when it is missing or empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argument_sources: Vec<Vec<ImplementationValueSource>>,
    /// The traced value provenance of the *callee expression* — the same trace
    /// [`ReturnSite::sources`] carries for a returned expression and
    /// `argument_sources` carries for an argument, applied to the callee.
    ///
    /// It answers "what created the value being called", which nothing else on
    /// this struct answers. `target`, `target_name`, `target_module`,
    /// `declaration` and `callee_parameter` state the callee's *resolution* —
    /// which symbol, which declaration, which parameter — and they are
    /// unchanged by this field's presence: a consumer whose claim is "the callee
    /// is parameter N" keeps reading `callee_parameter`. The two coexist because
    /// they disagree usefully: `read()` for `const [read] = createSignal(0)`
    /// resolves to a `BindingElement` and traces to `createSignal`'s tuple slot
    /// 0.
    ///
    /// **An empty list means the producer traced nothing.** It is never "the
    /// callee is not an accessor", never "the callee is plain", and never a
    /// claim about the callee at all: an ordinary `arr.push(x)` traces nothing
    /// here, and so do a computed callee, a reassigned binding, and
    /// `(options.storage || createSignal)(…)`. Every consumer fails closed on
    /// an empty list, and a *short* trace is the same absence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callee_sources: Vec<ImplementationValueSource>,
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

/// One traced provenance of a value: what sits at `path` within the traced
/// value, and — for a [`ImplementationValueSourceKind::CallResult`] — which
/// callee and which slot of its result it came from.
///
/// A source's *presence* is a positive fact; its absence is not. A value the
/// producer declines to model contributes no source, so an empty list of these
/// is the producer's silence and never a claim about what the value is not.
/// `target_module` is the written import specifier text and is empty for a
/// locally declared callee: it is a fact about the source text, not a resolved
/// package identity, and a consumer that needs dialect identity must ask the
/// dialect whether that specifier exports that name.
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
    /// Positive identity of an unchanged whole input binding. Absence is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<ParameterValueSource>,
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

/// Which of two different things a control-flow `unsupported` marker means.
///
/// The enum is **closed**: it carries no `#[serde(other)]` arm, so an
/// unrecognized string fails deserialization and rejects the whole transcript,
/// exactly as for [`crate::UncensusedInvokingFormKind`]. Reading an unknown
/// class as either arm picks the unsound one half the time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlFlowIncompletenessClass {
    /// A construct the census walked in full: every site inside it is recorded
    /// by the shared body walk, and no site is called `unreachable` on its
    /// account — the sites inside carry `unknown`. What is missing is only the
    /// *lower* bound: control may not enter a loop body, a `catch` clause, or
    /// a selected `switch` clause.
    ///
    /// A consumer asking a **may-execute** question — "is every callable this
    /// body can reach enumerated here?" — is therefore answered. One asking for
    /// a guarantee is not, which is why the marker still opens the transcript.
    ReachabilityLowerBound,
    /// A construct whose flow the census cannot account for in either
    /// direction, so neither a may-execute nor a guarantee question is
    /// answered. It is also the producer's classifier default, so a marker
    /// nobody classified arrives here rather than as the admissible arm.
    FlowUnaccounted,
}

/// One construct whose flow a control-flow census does not fully model, at its
/// exact location, and what is missing.
///
/// One row per construct, so a body with two loops carries two rows.
/// [`ControlFlowCensus::unsupported`] stays one deduplicated marker string per
/// *kind*, for the consumers that already read it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlFlowIncompleteness {
    /// The same string `unsupported` carries for this construct, so the two
    /// lists can be joined.
    pub marker: Arc<str>,
    pub class: ControlFlowIncompletenessClass,
    pub location: Location,
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
    /// The deduplicated marker set, unchanged in meaning: any entry means this
    /// census is incomplete, and the producer appends `controlFlowUnsupported`
    /// to the transcript's open reasons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<Arc<str>>,
    /// `unsupported` with the two facts a marker string never carried: where
    /// the construct is, and which class the incompleteness belongs to.
    ///
    /// Every marker in `unsupported` has at least one row here and every row's
    /// marker is in `unsupported`;
    /// [`crate::session::validate_control_flow_incompleteness`] enforces both,
    /// so a producer cannot state an unclassified marker.
    ///
    /// An **absent** list beside a nonempty `unsupported` is a producer with no
    /// classification, which a consumer must refuse — and serde cannot separate
    /// it from a present empty one, so
    /// [`crate::v3::TYPE_FACTS_HANDSHAKE_PROTOCOL`] is the discriminator.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incompleteness: Vec<ControlFlowIncompleteness>,
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
