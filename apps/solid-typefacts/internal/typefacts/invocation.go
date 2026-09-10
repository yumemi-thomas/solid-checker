package typefacts

import "context"

const MaxInvocationCallableDepth = 8

// InvocationDomain is one independently closable sibling domain of an
// invocation transcript. Completion is deliberately local: closing bindings
// cannot close uses, and closing parameters cannot close nested value paths.
type InvocationDomain string

const (
	InvocationDomainSignature   InvocationDomain = "signature"
	InvocationDomainBindings    InvocationDomain = "bindings"
	InvocationDomainOmissions   InvocationDomain = "omissions"
	InvocationDomainParameters  InvocationDomain = "parameters"
	InvocationDomainResult      InvocationDomain = "result"
	InvocationDomainUses        InvocationDomain = "uses"
	InvocationDomainControlFlow InvocationDomain = "controlFlow"
)

type InvocationCompleteness []InvocationDomain

func (c InvocationCompleteness) Contains(domain InvocationDomain) bool {
	for _, candidate := range c {
		if candidate == domain {
			return true
		}
	}
	return false
}

// InvocationDemand asks for proof facts about one exact call or construct
// expression. CallableDepth bounds recursive fixed-path discovery; Census asks
// for implementation parameter-use and control-flow censuses.
type InvocationDemand struct {
	Location      Location `cbor:"location" json:"location"`
	CallableDepth int      `cbor:"callableDepth,omitempty" json:"callableDepth,omitempty"`
	Census        bool     `cbor:"census,omitempty" json:"census,omitempty"`
}

// ExportValueDemand asks for the exact value of one expression. Certification
// points it at a deterministic imported binding, never at an invented call.
type ExportValueDemand struct {
	Location               Location  `cbor:"location" json:"location"`
	ImplementationLocation *Location `cbor:"implementationLocation,omitempty" json:"implementationLocation,omitempty"`
	// LocalDeclarationLocation asks for an implementation transcript of the
	// function-like declaration whose source range is *exactly* this location,
	// inside the analyzed snapshot. It is the only way to reach a declaration
	// that no export names: ImplementationLocation above starts from an
	// identifier and resolves through the export's runtime binding, so a
	// module-local helper is unreachable through it.
	//
	// The location names the *declaration node*, not an identifier, and the
	// match is exact in both bytes: a location that merely contains a
	// function-like declaration, or that names one whose span differs by a
	// byte, refuses. So does a location in a file the program holds but the
	// snapshot does not carry as runtime source — every `lib.*.d.ts` and every
	// dependency `.d.ts` is a program file, and none of them has bytes to
	// census.
	//
	// The answer's LocalDeclaration carries the requested location back as its
	// own Location. That echo is *not* the identity binding: a producer
	// answering about a different helper would copy the demand just the same.
	// The binding is the answer's Declaration, whose location the checker
	// derives from the located declaration's own symbol and which must name the
	// demanded file and lie inside the demanded span, together with the
	// agreement of QueryName and that declaration's name. See
	// localDeclarationImplementationTranscriptLocked.
	LocalDeclarationLocation *Location `cbor:"localDeclarationLocation,omitempty" json:"localDeclarationLocation,omitempty"`
	CallableDepth            int       `cbor:"callableDepth,omitempty" json:"callableDepth,omitempty"`
	// ParameterPremises, stated only beside LocalDeclarationLocation, asks that
	// the local declaration's uncensused-form census be classified with each
	// named parameter bound to the given type: the type the *caller's*
	// premised census found in that argument slot at the call that reached
	// this declaration (ExportImplementationTranscript.CallArgumentPremises),
	// carried back by the consumer as the premise for the callee (ADR 0038,
	// handshake protocol 23). A parameter no entry names keeps its own type.
	// The producer answers with the same entries in
	// ExportImplementationTranscript.ParameterPremises when every one of them
	// was re-established on the callee's twin, and with none — the strictly
	// more refusing census — otherwise. Indexes are strictly increasing; a
	// demand carrying premises for anything but a local declaration is refused
	// whole, because an export's root has a declared signature to bind and a
	// consumer has no business restating it.
	ParameterPremises []ParameterPremise `cbor:"parameterPremises,omitempty" json:"parameterPremises,omitempty"`
}

type ArgumentBindingDisposition string

const (
	ArgumentBindingDirect              ArgumentBindingDisposition = "direct"
	ArgumentBindingExactTupleSpread    ArgumentBindingDisposition = "exactTupleSpread"
	ArgumentBindingUnknownLengthSpread ArgumentBindingDisposition = "unknownLengthSpread"
	ArgumentBindingUnmapped            ArgumentBindingDisposition = "unmapped"
)

type ExpandedArgumentSlot struct {
	ExpandedIndex  int  `cbor:"expandedIndex" json:"expandedIndex"`
	TupleIndex     *int `cbor:"tupleIndex,omitempty" json:"tupleIndex,omitempty"`
	ParameterIndex int  `cbor:"parameterIndex" json:"parameterIndex"`
	Rest           bool `cbor:"rest,omitempty" json:"rest,omitempty"`
}

type FormalRange struct {
	Start        int  `cbor:"start" json:"start"`
	EndExclusive *int `cbor:"endExclusive,omitempty" json:"endExclusive,omitempty"`
	Unbounded    bool `cbor:"unbounded,omitempty" json:"unbounded,omitempty"`
}

type ArgumentBinding struct {
	ArgumentIndex int                        `cbor:"argumentIndex" json:"argumentIndex"`
	Location      Location                   `cbor:"location" json:"location"`
	Disposition   ArgumentBindingDisposition `cbor:"disposition" json:"disposition"`
	Slots         []ExpandedArgumentSlot     `cbor:"slots,omitempty" json:"slots,omitempty"`
	Possible      *FormalRange               `cbor:"possible,omitempty" json:"possible,omitempty"`
	Reason        string                     `cbor:"reason,omitempty" json:"reason,omitempty"`
}

type ValueProtocol string

const (
	ValueProtocolPlain         ValueProtocol = "plain"
	ValueProtocolPromise       ValueProtocol = "promise"
	ValueProtocolAsyncIterable ValueProtocol = "asyncIterable"
)

// InvocationConstructability is the ordinary string wire form. The retained
// entity table uses a compact numeric representation whose zero value means
// absence; invocation facts are always present and must not inherit that
// storage-only encoding.
type InvocationConstructability string

const (
	InvocationConstructable    InvocationConstructability = "constructable"
	InvocationNonConstructable InvocationConstructability = "nonConstructable"
	InvocationConstructMixed   InvocationConstructability = "mixed"
	InvocationConstructUnknown InvocationConstructability = "unknown"
)

type PathSegmentKind string

const (
	PathSegmentProperty PathSegmentKind = "property"
	PathSegmentTuple    PathSegmentKind = "tuple"
)

type PathSegment struct {
	Kind     PathSegmentKind `cbor:"kind" json:"kind"`
	Property string          `cbor:"property,omitempty" json:"property,omitempty"`
	Index    *int            `cbor:"index,omitempty" json:"index,omitempty"`
}

type PathPresence string

const (
	PathRequired PathPresence = "required"
	PathOptional PathPresence = "optional"
	PathAbsent   PathPresence = "absent"
	PathUnknown  PathPresence = "unknown"
)

type ValueAlternative struct {
	Index         int            `cbor:"index" json:"index"`
	Discriminants []Discriminant `cbor:"discriminants,omitempty" json:"discriminants,omitempty"`
	OpenReasons   []string       `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type Discriminant struct {
	Property string                    `cbor:"property" json:"property"`
	Value    PrimitiveLiteralCandidate `cbor:"value" json:"value"`
}

type CallablePathFact struct {
	Alternative      int                        `cbor:"alternative" json:"alternative"`
	Path             []PathSegment              `cbor:"path,omitempty" json:"path,omitempty"`
	Presence         PathPresence               `cbor:"presence" json:"presence"`
	Callability      Callability                `cbor:"callability" json:"callability"`
	Constructability InvocationConstructability `cbor:"constructability" json:"constructability"`
	Declaration      *Declaration               `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Complete         bool                       `cbor:"complete,omitempty" json:"complete,omitempty"`
	// Apparent marks a member the value carries only through the compiler's
	// apparent-type augmentation — the global `Function` interface's members on
	// a node with call or construct signatures. Such a member exists (the
	// compiler's getPropertyOfType answers it, and `tsc` type-checks the
	// access), but it is library-owned rather than declared by the package
	// under analysis, it is never caller-supplied, and it is emitted as a leaf.
	// The declared-member census closure therefore excludes it, while exact
	// path lookups still find it. Every declared member carries false.
	//
	// Required on the wire in both directions: a producer that omits it would
	// have its leaves read as census members.
	Apparent          bool     `cbor:"apparent" json:"apparent"`
	SubtreeEnumerated bool     `cbor:"subtreeEnumerated" json:"subtreeEnumerated"`
	OpenReasons       []string `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type FinitePartitionAxis string

const (
	FinitePartitionLiteral      FinitePartitionAxis = "literal"
	FinitePartitionCallability  FinitePartitionAxis = "callability"
	FinitePartitionProtocol     FinitePartitionAxis = "protocol"
	FinitePartitionTuple        FinitePartitionAxis = "tuple"
	FinitePartitionDiscriminant FinitePartitionAxis = "discriminant"
)

type FiniteCase struct {
	Kind          string                     `cbor:"kind" json:"kind"`
	Literal       *PrimitiveLiteralCandidate `cbor:"literal,omitempty" json:"literal,omitempty"`
	Protocol      ValueProtocol              `cbor:"protocol,omitempty" json:"protocol,omitempty"`
	TupleLength   *int                       `cbor:"tupleLength,omitempty" json:"tupleLength,omitempty"`
	Discriminants []Discriminant             `cbor:"discriminants,omitempty" json:"discriminants,omitempty"`
}

type FinitePartition struct {
	Axis     FinitePartitionAxis `cbor:"axis" json:"axis"`
	Complete bool                `cbor:"complete,omitempty" json:"complete,omitempty"`
	Cases    []FiniteCase        `cbor:"cases,omitempty" json:"cases,omitempty"`
}

// ValuePrimitiveDomain is the ordinary-serialization form of the compact
// retained-table PrimitiveValueDomain.
type ValuePrimitiveDomain struct {
	MayBeString    bool `cbor:"mayBeString,omitempty" json:"mayBeString,omitempty"`
	MayBeNumber    bool `cbor:"mayBeNumber,omitempty" json:"mayBeNumber,omitempty"`
	MayBeBoolean   bool `cbor:"mayBeBoolean,omitempty" json:"mayBeBoolean,omitempty"`
	MayBeBigInt    bool `cbor:"mayBeBigInt,omitempty" json:"mayBeBigInt,omitempty"`
	MayBeSymbol    bool `cbor:"mayBeSymbol,omitempty" json:"mayBeSymbol,omitempty"`
	MayBeNull      bool `cbor:"mayBeNull,omitempty" json:"mayBeNull,omitempty"`
	MayBeUndefined bool `cbor:"mayBeUndefined,omitempty" json:"mayBeUndefined,omitempty"`
	MayBeObject    bool `cbor:"mayBeObject,omitempty" json:"mayBeObject,omitempty"`
	NumbersFinite  bool `cbor:"numbersFinite,omitempty" json:"numbersFinite,omitempty"`
	Unknown        bool `cbor:"unknown,omitempty" json:"unknown,omitempty"`
}

type InvocationValueFact struct {
	Type             *TypeDescriptor            `cbor:"type,omitempty" json:"type,omitempty"`
	Callability      Callability                `cbor:"callability" json:"callability"`
	Constructability InvocationConstructability `cbor:"constructability" json:"constructability"`
	Primitive        ValuePrimitiveDomain       `cbor:"primitive" json:"primitive"`
	Alternatives     []ValueAlternative         `cbor:"alternatives,omitempty" json:"alternatives,omitempty"`
	Partitions       []FinitePartition          `cbor:"partitions,omitempty" json:"partitions,omitempty"`
	OpenReasons      []string                   `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type ExportValueTranscript struct {
	Location      Location             `cbor:"location" json:"location"`
	QueryName     string               `cbor:"queryName,omitempty" json:"queryName,omitempty"`
	Target        SymbolID             `cbor:"target,omitempty" json:"target,omitempty"`
	Declaration   *ResolvedDeclaration `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Value         InvocationValueFact  `cbor:"value" json:"value"`
	CallablePaths []CallablePathFact   `cbor:"callablePaths,omitempty" json:"callablePaths,omitempty"`
	// The exported value's one call signature, present only when its type has
	// exactly one. An overload set reports CallSignatures instead; the two are
	// never both populated, so no consumer can mistake one overload for "the"
	// signature.
	CallSignature *SelectedSignature `cbor:"callSignature,omitempty" json:"callSignature,omitempty"`
	// Every call signature of an overloaded exported value, in declaration
	// order. Populated only when the type has more than one, and only when
	// every one of them could be described.
	CallSignatures []SelectedSignature             `cbor:"callSignatures,omitempty" json:"callSignatures,omitempty"`
	Implementation *ExportImplementationTranscript `cbor:"implementation,omitempty" json:"implementation,omitempty"`
	// Initializer binds the runtime query to a source initializer. It does not
	// classify the result of the imported call or authorize dependency behavior.
	Initializer *ExportInitializerTranscript `cbor:"initializer,omitempty" json:"initializer,omitempty"`
	// LocalDeclaration answers ExportValueDemand.LocalDeclarationLocation. It
	// is present exactly when that field was set, and its Location is the
	// requested location verbatim — which is why the Location alone binds
	// nothing, and why the client also requires the resolved Declaration to
	// sit inside the demanded span. See LocalDeclarationLocation above.
	LocalDeclaration *ExportImplementationTranscript `cbor:"localDeclaration,omitempty" json:"localDeclaration,omitempty"`
	Complete         bool                            `cbor:"complete,omitempty" json:"complete,omitempty"`
	OpenReasons      []string                        `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type ExportInitializerBinding struct {
	Declaration ResolvedDeclaration `cbor:"declaration" json:"declaration"`
	Location    Location            `cbor:"location" json:"location"`
	Initializer Location            `cbor:"initializer" json:"initializer"`
}

type ExportInitializerObjectArgument struct {
	Index    int      `cbor:"index" json:"index"`
	Location Location `cbor:"location" json:"location"`
}

// An exact, positively unwritten variable chain ending in an ordinary imported
// call. Only direct object-literal arguments are listed. Their properties and
// the call's result remain unclassified. Absence grants no premise.
type ExportInitializerTranscript struct {
	Location        Location                          `cbor:"location" json:"location"`
	Target          SymbolID                          `cbor:"target" json:"target"`
	Bindings        []ExportInitializerBinding        `cbor:"bindings" json:"bindings"`
	Call            Location                          `cbor:"call" json:"call"`
	Callee          Location                          `cbor:"callee" json:"callee"`
	Declaration     ResolvedDeclaration               `cbor:"declaration" json:"declaration"`
	Specifier       string                            `cbor:"specifier" json:"specifier"`
	ExportName      string                            `cbor:"exportName" json:"exportName"`
	ObjectArguments []ExportInitializerObjectArgument `cbor:"objectArguments" json:"objectArguments"`
}

// ImplementationCompletionForm classifies, from syntax alone, what an
// implementation hands its caller when it completes (ADR 0035). A plain
// callable completes with whatever its return sites carry — nothing, for a
// bare `return;` or a body that falls off its end. An `async` function hands
// back a promise on every completion, a generator an iterator, and an async
// generator an async iterator, whatever the body does. Unclassified is the
// default for a declaration kind this classifier does not review, so a
// consumer refuses it rather than reading it as plain.
type ImplementationCompletionForm string

const (
	CompletionPlain          ImplementationCompletionForm = "plain"
	CompletionAsync          ImplementationCompletionForm = "async"
	CompletionGenerator      ImplementationCompletionForm = "generator"
	CompletionAsyncGenerator ImplementationCompletionForm = "async-generator"
	CompletionUnclassified   ImplementationCompletionForm = "unclassified"
)

// UnwrittenParameterBinding is affirmative source identity, not type evidence.
// Protocol 39: a plain parameter without rest/default syntax, duplicate names,
// writes, arguments or eval references retains the caller's exact slot value.
type UnwrittenParameterBinding struct {
	ParameterIndex int      `cbor:"parameterIndex" json:"parameterIndex"`
	Declaration    Location `cbor:"declaration" json:"declaration"`
}

// OriginalHelperRead is a one-hop, possible synchronous member read. The
// caller's plain argument still carries its original input at Call; the exact
// stable local helper reads that slot without replacing it. It proves neither
// guaranteed execution nor member shape, and never describes later uses.
type OriginalHelperRead struct {
	ParameterIndex       int      `cbor:"parameterIndex" json:"parameterIndex"`
	Declaration          Location `cbor:"declaration" json:"declaration"`
	Call                 Location `cbor:"call" json:"call"`
	ArgumentIndex        int      `cbor:"argumentIndex" json:"argumentIndex"`
	Argument             Location `cbor:"argument" json:"argument"`
	Helper               Location `cbor:"helper" json:"helper"`
	HelperImplementation Location `cbor:"helperImplementation" json:"helperImplementation"`
	HelperParameter      Location `cbor:"helperParameter" json:"helperParameter"`
	Read                 Location `cbor:"read" json:"read"`
	Property             string   `cbor:"property" json:"property"`
}

// InitialParameterRead binds a direct property use in a write-free opening
// declaration prefix. It proves the caller's root at this use, not later uses
// or a property's runtime shape. Absence is not a negative origin fact.
// FirstIterationOnly restricts a supported loop-body use to its first entry;
// it is not a guarantee that this use executes or retains origin on later entries.
// Positional marks a row established by order alone (ADR 0069): the read
// precedes every store this body performs, with no iteration statement
// enclosing both. It is the caller's value at that exact use, and only a
// consumer that binds this use to its own operation may take it.
type InitialParameterRead struct {
	ParameterIndex     int      `cbor:"parameterIndex" json:"parameterIndex"`
	Declaration        Location `cbor:"declaration" json:"declaration"`
	Use                Location `cbor:"use" json:"use"`
	FirstIterationOnly bool     `cbor:"firstIterationOnly,omitempty" json:"firstIterationOnly,omitempty"`
	Positional         bool     `cbor:"positional,omitempty" json:"positional,omitempty"`
	// UndefinedDefault limits origin to calls whose original argument is not
	// undefined. It is distinct from an unwritten or positional binding.
	UndefinedDefault *UndefinedParameterDefault `cbor:"undefinedDefault,omitempty" json:"undefinedDefault,omitempty"`
}

type UndefinedParameterDefault struct {
	Guard      Location `cbor:"guard" json:"guard"`
	Assignment Location `cbor:"assignment" json:"assignment"`
}

type ExportImplementationTranscript struct {
	Location    Location             `cbor:"location" json:"location"`
	QueryName   string               `cbor:"queryName,omitempty" json:"queryName,omitempty"`
	Target      SymbolID             `cbor:"target,omitempty" json:"target,omitempty"`
	Declaration *ResolvedDeclaration `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Signature   *SelectedSignature   `cbor:"signature,omitempty" json:"signature,omitempty"`
	// CompletionForm is stated on every transcript that reached its
	// implementation, beside Declaration; a consumer never reads its absence
	// as plain (handshake protocol 19).
	CompletionForm ImplementationCompletionForm `cbor:"completionForm,omitempty" json:"completionForm,omitempty"`
	// ImplementationOf names the declaration whose body this transcript's
	// census walked when Declaration is a binding that merely aliases it by
	// identity -- `const defaultScheduler = systemSetTimeoutZero` (handshake
	// protocol 20). Absent when the body is Declaration's own. Never stated
	// unless the alias is exact: an identifier initializer, the binding never
	// assigned in its file, and the aliased function never assigned in its own.
	ImplementationOf      *ResolvedDeclaration        `cbor:"implementationOf,omitempty" json:"implementationOf,omitempty"`
	ParameterUses         []ParameterUse              `cbor:"parameterUses,omitempty" json:"parameterUses,omitempty"`
	UnwrittenParameters   []UnwrittenParameterBinding `cbor:"unwrittenParameters,omitempty" json:"unwrittenParameters,omitempty"`
	InitialParameterReads []InitialParameterRead      `cbor:"initialParameterReads,omitempty" json:"initialParameterReads,omitempty"`
	OriginalHelperReads   []OriginalHelperRead        `cbor:"originalHelperReads,omitempty" json:"originalHelperReads,omitempty"`
	ControlFlow           *ControlFlowCensus          `cbor:"controlFlow,omitempty" json:"controlFlow,omitempty"`
	// CallableReturns records the return-carry edges owned by each nested
	// callable in this implementation. The top-level implementation's return
	// sites remain in ControlFlow; these rows let a consumer compose a returned
	// callable that itself returns another callable, without treating lexical
	// nesting as execution.
	CallableReturns []CallableReturnCensus `cbor:"callableReturns,omitempty" json:"callableReturns,omitempty"`
	Calls           []ImplementationCall   `cbor:"calls,omitempty" json:"calls,omitempty"`
	// UncensusedInvokingForms names, per form, every syntactic position in this
	// implementation that can invoke user code and that Calls does *not*
	// record. Calls holds CallExpression and NewExpression only; everything
	// else that reaches a callable — a tagged template, an accessor behind a
	// property access, the iteration protocol, a decorator, `Symbol.dispose`,
	// `Symbol.hasInstance`, a thenable's `then`, a coercion, a JSX lowering —
	// appears here instead, so a consumer that must enumerate every invoking
	// form refuses by name rather than concluding from silence.
	//
	// Its classifier's default is refusal, not silence: a node kind that is
	// neither classified into one of the named kinds nor on the reviewed
	// list of kinds that provably cannot invoke user code is recorded as
	// UncensusedUnclassifiedInvokingForm with its node kind name. A kind a
	// future compiler revision adds therefore refuses on arrival.
	//
	// Complete says nothing about this field, and this field says nothing
	// about Complete: the two are independent. An *empty* list on a transcript
	// that carries the field is the positive claim "every form I walked was
	// either a call, a construction, or provably non-invoking"; an *absent*
	// field is a producer that has no opinion, which every consumer must
	// refuse. The handshake protocol is what separates the two — see
	// TypeFactsHandshakeProtocol.
	UncensusedInvokingForms []UncensusedInvokingForm `cbor:"uncensusedInvokingForms,omitempty" json:"uncensusedInvokingForms,omitempty"`
	// ParameterPremises states, when present, that UncensusedInvokingForms was
	// classified with each of this implementation's parameters bound to the
	// type the export's *declared* call signature gives that position — the
	// signature a consumer compiles against and the one a synthesized veto
	// samples from — rather than to the implicit `any` an unannotated
	// JavaScript parameter carries (ADR 0038, handshake protocol 22). One entry
	// per parameter, in position order; Type is the declared type's printed
	// form, byte-identical to the corresponding
	// SelectedSignature.Parameters[i].Value.Type.Text of the export's own
	// transcript, which is how a consumer binds the premise to the signature
	// it already holds.
	//
	// The producer states it only for the export's root implementation: a
	// JavaScript function whose declared signature lives in a declaration
	// file, with as many parameters as the signature, no rest parameter on
	// either side, and every one of those types re-established on a checked
	// twin of the file (see declared_signature_premise.go). Absent, the forms
	// were classified over the parameters' own types, which is the strictly
	// more refusing reading; an absent list is never a premise, and a consumer
	// records a present one as a condition of whatever it concludes.
	ParameterPremises []ParameterPremise `cbor:"parameterPremises,omitempty" json:"parameterPremises,omitempty"`
	// ParameterPremiseRefusal names, when the producer had a declared signature
	// to bind and a form that a type could have cleared but did not state
	// ParameterPremises, why the binding was refused. Diagnostic only: a
	// consumer decides nothing from it, and its absence means nothing.
	ParameterPremiseRefusal string `cbor:"parameterPremiseRefusal,omitempty" json:"parameterPremiseRefusal,omitempty"`
	// CallArgumentPremises states, for a call or construction this
	// implementation's *premised* form census walked whose callee resolves to
	// a declaration in the program's own runtime source, the type the twin's
	// checker gave each written argument slot (handshake protocol 23). It is
	// how a premise reaches a local helper: the consumer carries the entry for
	// the call that reached the helper back as ExportValueDemand.ParameterPremises
	// of the helper's own local-declaration demand, and the helper's census is
	// classified under exactly those types. Stated only beside a nonempty
	// ParameterPremises — an argument's type under the parameters' own `any`
	// is nothing a premise could carry — only for a call whose every argument
	// is a plain written slot (no spread), and only for the slots whose type is
	// informative; a slot the list omits is `any` to the callee. Each Call is
	// the location of a row of Calls, in the original file's bytes.
	CallArgumentPremises []CallArgumentPremise `cbor:"callArgumentPremises,omitempty" json:"callArgumentPremises,omitempty"`
	// PrimitiveCompletion says the value this implementation hands its caller
	// is **provably a primitive**: the checker's return type for the
	// declaration, on the very program this census was classified over — the
	// premise twin when the census was premised — is a union of primitive
	// types alone (ADR 0045, handshake protocol 29). An explicit never type
	// also states this fact: no normal completion can hand back an object
	// (ADR 0051, handshake protocol 35). Missing type information cannot.
	//
	// A coercion of that value therefore reaches no `Symbol.toPrimitive`,
	// `valueOf` or `toString` of anyone's. The completion form is covered by
	// the same fact rather than beside it: an async function's return type is
	// a `Promise` and a generator's is a `Generator`, neither of which is a
	// primitive, so neither can set this.
	//
	// **It is a fact about the census's own premise, not about the
	// declaration.** The same helper censused under two argument premises may
	// state it under one and not the other, which is why a consumer may read it
	// only from the transcript it demanded under the premise it recorded.
	PrimitiveCompletion bool     `cbor:"primitiveCompletion,omitempty" json:"primitiveCompletion,omitempty"`
	Complete            bool     `cbor:"complete,omitempty" json:"complete,omitempty"`
	OpenReasons         []string `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

// ParameterPremise is one parameter's type binding under which an
// implementation's uncensused-form census was classified (ADR 0038). See
// ExportImplementationTranscript.ParameterPremises.
//
// Type is the bound type's printed form. Identity is the producer's own
// binding of that text to a declaration — the type's flags and, for a class,
// interface, enum or alias, the declaration it resolves to, in the original
// file's coordinates — stated on a call-argument premise and echoed on the
// demand and the callee's premise so a spelling that resolves to a *different*
// declaration of the same name on the callee's twin is refused rather than
// bound. It is opaque to a consumer, which compares it byte for byte and reads
// nothing into it; a root premise, bound to the declared signature by its text
// alone, carries none.
type ParameterPremise struct {
	Index    int    `cbor:"index" json:"index"`
	Type     string `cbor:"type" json:"type"`
	Identity string `cbor:"identity,omitempty" json:"identity,omitempty"`
	// Spelling is a form of Type that resolves from a module which cannot name
	// it directly — `import("<specifier>").<Name>` — stated when the printed
	// text names a type declared in a declaration file (ADR 0046, handshake
	// protocol 30).
	//
	// It exists because a helper premise is written into a **JavaScript**
	// module as a JSDoc `@param`, and `@param {Axis}` resolves to nothing
	// there however precisely the package's declarations define `Axis`. The
	// caller's twin, which does resolve it, is where the spelling can be
	// computed.
	//
	// **It is a hint, never the premise.** Type and Identity remain the whole
	// falsifier: the twin's own printed type must still equal Type and its
	// declaration identity must still equal Identity, so a spelling that
	// resolved to anything else refuses the twin exactly as a bad printed text
	// does. A consumer echoes it byte for byte and interprets none of it.
	Spelling string `cbor:"spelling,omitempty" json:"spelling,omitempty"`
}

// CallArgumentPremise is the type each written argument slot of one call
// carried under a premised census (handshake protocol 23). See
// ExportImplementationTranscript.CallArgumentPremises.
type CallArgumentPremise struct {
	Call Location `cbor:"call" json:"call"`
	// Arguments covers the slots the call **writes**, and since ADR 0049 the
	// slots it does not: a parameter the call leaves unwritten receives
	// `undefined` at run time, which is a primitive and so a stronger premise
	// than the `any` an unannotated parameter would otherwise carry. A slot
	// whose parameter has an initializer, is a rest parameter, is not a plain
	// identifier, or is written in the callee's own file is skipped rather
	// than stated. The list stays strictly increasing either way.
	Arguments []ParameterPremise `cbor:"arguments" json:"arguments"`
}

// UncensusedInvokingFormKind is a closed enumeration. A consumer that receives
// a string outside it must reject the whole transcript rather than treat the
// row as unknown: a producer that invented a kind is a producer this vocabulary
// does not describe.
type UncensusedInvokingFormKind string

const (
	// UncensusedTaggedTemplate is a TaggedTemplateExpression. The tag function
	// is invoked with the strings array and the substitutions. An `html`
	// template counts here and nowhere else.
	UncensusedTaggedTemplate UncensusedInvokingFormKind = "tagged-template"
	// UncensusedGetAccessor is a property access, element access, or
	// destructured member the checker resolved to a symbol with a get-accessor
	// declaration, in a position that reads it.
	UncensusedGetAccessor UncensusedInvokingFormKind = "get-accessor"
	// UncensusedSetAccessor is the same, with a set-accessor declaration, in an
	// assignment target position.
	UncensusedSetAccessor UncensusedInvokingFormKind = "set-accessor"
	// UncensusedPropertyAccessUnknownAccessor is a member the producer cannot
	// answer for. Three ways that happens: the checker resolved no symbol at
	// all (an `any`-typed receiver, a computed key, an index signature, an
	// element access whose key is not an exact literal); it resolved
	// declarations that are not the snapshot's runtime bytes, such as a `.d.ts`
	// `readonly value` that may perfectly well describe a `.js` getter (the
	// default library excepted, since it describes the engine rather than user
	// code); or the form reads every own enumerable property of a value whose
	// shape is not statically known — object spread, JSX prop spread, and an
	// object rest element.
	//
	// It is recorded rather than dropped because the producer genuinely cannot
	// tell whether the member is an accessor: without a symbol there are no
	// declarations to inspect, with only a declaration file there are no bytes,
	// and neither absence is evidence that the property is a plain data
	// property. Dropping it would make silence carry the claim, which is the
	// one thing this field exists to prevent. A member the checker *does*
	// resolve, to runtime declarations none of which is an accessor, is a plain
	// data property and is recorded nowhere.
	UncensusedPropertyAccessUnknownAccessor UncensusedInvokingFormKind = "property-access-unknown-accessor"
	// UncensusedDecorator is a Decorator application. The decorator expression
	// is invoked when the decorated declaration is evaluated.
	UncensusedDecorator UncensusedInvokingFormKind = "decorator"
	// UncensusedIterationProtocol is a position that reaches
	// `Symbol.iterator`/`Symbol.asyncIterator` and the `next`/`return` of the
	// iterator it answers: `for…of`, `for await…of`, a spread element, an
	// array binding pattern, an array *assignment* pattern (`[a, b] = src`,
	// which is an ArrayLiteralExpression the compiler reinterprets), and
	// `yield*`.
	UncensusedIterationProtocol UncensusedInvokingFormKind = "iteration-protocol"
	// UncensusedUsingDispose is a `using` or `await using` declaration list.
	// Scope exit reaches `Symbol.dispose` or `Symbol.asyncDispose` on every
	// declared value.
	UncensusedUsingDispose UncensusedInvokingFormKind = "using-dispose"
	// UncensusedInstanceOf is an `instanceof` operator, which reaches
	// `Symbol.hasInstance` on its right operand when that operand defines it.
	UncensusedInstanceOf UncensusedInvokingFormKind = "instanceof"
	// UncensusedAwaitThen is an `await` whose operand is not provably resolved
	// by the engine alone. Awaiting a thenable invokes that object's own
	// `then`, so the form is recorded unless *every* constituent of the
	// operand's type is either a primitive — which has no `then` to call — or
	// a default-library `Promise`, whose `then` is the engine's own. A
	// `PromiseLike` is recorded, because its `then` is whatever object the
	// value carries.
	//
	// "The type declares no `then`" is deliberately *not* a reason to stay
	// silent: a union missing it in one constituent carries it in another, an
	// unconstrained type parameter has no members the checker can enumerate,
	// and an index-signature type declares none while permitting one at
	// runtime.
	UncensusedAwaitThen UncensusedInvokingFormKind = "await-then"
	// UncensusedCoercion is a template expression or an operator application
	// whose operand is not provably a non-object, so evaluating it may reach
	// `Symbol.toPrimitive`, `valueOf`, or `toString`.
	UncensusedCoercion UncensusedInvokingFormKind = "coercion"
	// UncensusedJSXElement is a JSX element, self-closing element, or
	// fragment. Its compiler lowering invokes a component or an accessor, and
	// the producer records neither the lowering nor what it invokes.
	UncensusedJSXElement UncensusedInvokingFormKind = "jsx-element"
	// UncensusedUnclassifiedInvokingForm is the catch-all, and the row this
	// whole field exists for: a node kind that is neither classified above nor
	// on the reviewed list of kinds that provably cannot invoke user code.
	// NodeKind carries the compiler's own name for it. A consumer refuses on
	// this row unconditionally — there is nothing else it could soundly do
	// with a form nobody has classified.
	UncensusedUnclassifiedInvokingForm UncensusedInvokingFormKind = "unclassified-invoking-form"
)

// UncensusedInvokingForm is one syntactic position that can invoke user code
// and that the implementation call census does not record.
//
// Two invoking forms are deliberately *not* in the enumeration:
//
//   - A `Proxy` trap. A trap is a property of the object a value happens to
//     be at runtime, not of any syntax, so no walk of this implementation can
//     see it: `obj.x` on a proxy is the same PropertyAccessExpression as
//     `obj.x` on a plain object. It is out of the producer's reach entirely,
//     and inventing a marker for it would claim a census the producer cannot
//     perform. What the producer *can* say is that a property access it could
//     not resolve is unresolved, which is
//     UncensusedPropertyAccessUnknownAccessor — that is a statement about the
//     checker's knowledge, not about proxies. A consumer whose claim requires
//     that no proxy trap ran must obtain that premise elsewhere.
//   - An optional call, `f?.(x)`. It is already a CallExpression in the AST,
//     so the call census records it like any other call.
//
// SubjectRootDerivation names how an uncensused invoking form's subject was
// rooted at a parameter. See UncensusedInvokingForm.SubjectRoot for the
// premise each spelling carries and for why a consumer must refuse one it has
// not reviewed.
type SubjectRootDerivation string

const (
	// SubjectRootParameter is ADR 0034's premise: the value at that parameter
	// slot is the one the caller passed.
	SubjectRootParameter SubjectRootDerivation = "parameter"
	// SubjectRootParameterDefault is ADR 0043's: the slot carries a default
	// that names another such slot, so the value is caller-supplied whether or
	// not an argument was passed here.
	SubjectRootParameterDefault SubjectRootDerivation = "parameter-default"
	// SubjectRootOwnLiteral is ADR 0044's: the subject is a binding this
	// program initialized from an **object or array literal**, or from a rest
	// element of a destructuring, whose every own property the specification
	// creates with CreateDataPropertyOrThrow. No SubjectParameter accompanies
	// it — the value is not the caller's, it is this program's — and
	// SubjectDeclaration names the binding instead.
	SubjectRootOwnLiteral SubjectRootDerivation = "own-literal"
	// SubjectRootParameterResult is ADR 0048's: the subject is the value a
	// call **to a caller-supplied callee** handed back. What the caller's own
	// function returned is the caller's, exactly as what its iterable yielded
	// is (ADR 0042's `parameter-rooted-element`), so an accessor on it was
	// installed by the caller and is analyzed in the caller's own artifact.
	// The SubjectParameter is the slot the callee was rooted at.
	SubjectRootParameterResult SubjectRootDerivation = "parameter-result"
	// SubjectRootDefaultLibrary is ADR 0047's, for an `instanceof` whose
	// constructor the checker resolves to a declaration of the default
	// library. `Symbol.hasInstance` on such a constructor is the engine's own,
	// inherited from `Function.prototype`, so the operator reaches no user
	// code. No SubjectParameter accompanies it.
	SubjectRootDefaultLibrary SubjectRootDerivation = "default-library"
	// SubjectRootOwnClass is ADR 0047's other half: a class **this program**
	// declares, with no heritage clause and no static computed member, so
	// nothing on its prototype chain can carry a `Symbol.hasInstance` and the
	// operator runs OrdinaryHasInstance alone. SubjectDeclaration names the
	// class, and no SubjectParameter accompanies it.
	SubjectRootOwnClass SubjectRootDerivation = "own-class"
)

// CoercionPremise is what one `coercion` form's clearance would rest on: the
// calls whose results the coercion applies ToPrimitive to. Every other operand
// of the form is provably a primitive by its own type, which is why it is not
// listed — a value that is already a primitive has no `Symbol.toPrimitive`,
// `valueOf` or `toString` for the coercion to reach.
type CoercionPremise struct {
	// Calls are locations of rows of the same transcript's Calls list, in
	// source order and without duplicates. A consumer matches each to its row,
	// takes that row's own callee and premise, and requires the callee's
	// transcript to state a primitive completion.
	Calls []Location `cbor:"calls" json:"calls"`
}

type UncensusedInvokingForm struct {
	Kind UncensusedInvokingFormKind `cbor:"kind" json:"kind"`
	// NodeKind is the compiler's own name for the node's syntax kind, with the
	// "Kind" prefix removed. Always populated, and load-bearing for
	// UncensusedUnclassifiedInvokingForm, where it is the only description of
	// the form the producer has.
	NodeKind string   `cbor:"nodeKind" json:"nodeKind"`
	Location Location `cbor:"location" json:"location"`
	// Reach comes from the same walk, and therefore the same reachability
	// notion, as ImplementationCall.Reach.
	//
	// Unlike the call census this census applies no jump withholding. A call
	// inside a region a `break` makes non-universal is dropped there because
	// its positive Reach would overstate execution; here a dropped row is
	// silence, which is the failure mode the field exists to prevent, and an
	// over-optimistic Reach can only cause a consumer to refuse a form that
	// might not have run. Over-refusal is the safe direction; silence is not.
	Reach Reachability `cbor:"reach" json:"reach"`
	// EnclosingCallable is the exact source range of the innermost callable
	// containing this form, absent when the form sits directly in the
	// implementation's own body. Captured is true exactly when it is present.
	// Same discipline as ImplementationCall's two fields, and for the same
	// reason: lexical containment in a closure is not execution.
	EnclosingCallable *Location `cbor:"enclosingCallable,omitempty" json:"enclosingCallable,omitempty"`
	Captured          bool      `cbor:"captured,omitempty" json:"captured,omitempty"`
	// SubjectParameter, when present, is the index of a parameter of the
	// transcript's own declaration at which the form's subject — the receiver
	// of a property or element access — is rooted, through a chain of property
	// and element reads only (ADR 0034, handshake protocol 18). The producer
	// states it only for a get-accessor, set-accessor or
	// property-access-unknown-accessor form whose root parameter is a plain
	// identifier binding with no initializer and no rest token, written
	// nowhere in its file, in a declaration that mentions neither `arguments`
	// nor `eval`. Absent otherwise; absence is never "not rooted", and a
	// consumer that reads this fact must require protocol 18 first.
	SubjectParameter *int `cbor:"subjectParameter,omitempty" json:"subjectParameter,omitempty"`
	// SubjectWrite accompanies SubjectParameter and says the access is in
	// **write position** — an assignment target, a compound assignment, or an
	// update expression — so the accessor that may run is a setter, and a
	// compound or update form runs the getter too (ADR 0040, handshake
	// protocol 24). False, and absent from the wire, for a plain read.
	//
	// The distinction is stated rather than folded into the subject because
	// the two positions are the same fact for one question and different facts
	// for another: for `creates` the accessor is the caller's code either way,
	// while for `writes` and `invalidates` the write is this export's own act
	// and a census of those domains must refuse exactly what this field marks.
	// A consumer that cannot tell the positions apart must not read the
	// subject at all, which is why the number moves with the field.
	SubjectWrite bool `cbor:"subjectWrite,omitempty" json:"subjectWrite,omitempty"`
	// SubjectRoot accompanies SubjectParameter and names **how** the subject
	// was rooted at that parameter (ADR 0043, handshake protocol 27). It is
	// present exactly when SubjectParameter is, and its value is one of a
	// closed, reviewed set:
	//
	//   - SubjectRootParameter — the subject is the parameter itself, or a
	//     chain of property and element reads whose innermost receiver is, or
	//     a name the parameter's own binding pattern bound, or a name a local
	//     declaration bound from such a value. Every one of these reads a
	//     value the caller passed at that slot, which is ADR 0034's premise
	//     unchanged: naming an intermediate does not change whose value it is.
	//   - SubjectRootParameterDefault — the subject is rooted at a parameter
	//     that carries a **default**, whose default expression is itself a
	//     reference to a parameter rooted the first way. The value is then the
	//     caller's argument at this slot when one was passed and the caller's
	//     argument at the default's slot when none was, so it is caller-supplied
	//     under either branch — but it is a *different* claim from the first,
	//     and a consumer that has reviewed only the first must refuse it.
	//
	// A consumer must treat an unknown spelling as unreviewed and refuse the
	// form, exactly as the control-flow incompleteness classes are read: a
	// value a later revision adds arrives as a refusal rather than as silence.
	SubjectRoot SubjectRootDerivation `cbor:"subjectRoot,omitempty" json:"subjectRoot,omitempty"`
	// SubjectDeclaration is the exact source range of the declaration the
	// subject is rooted at, stated for SubjectRootOwnLiteral (the variable, ADR
	// 0044) and SubjectRootOwnClass (the class, ADR 0047), and for those
	// derivations alone.
	//
	// It is what a consumer can check for itself. Every other derivation names
	// a *parameter* of the transcript's own declaration, which the consumer
	// already holds; this one names a binding elsewhere in the file, and the
	// premise turns on that binding sitting in the analyzed artifact's own
	// runtime source rather than in a declaration file or a dependency. A
	// consumer must refuse an own-literal subject whose declaration it cannot
	// place there.
	SubjectDeclaration *Location `cbor:"subjectDeclaration,omitempty" json:"subjectDeclaration,omitempty"`
	// CoercionPremise, present only on a `coercion` form, states that every
	// operand the coercion applies ToPrimitive to is either **provably a
	// primitive** by its type or the **result of a call** to a declaration in
	// this program's own runtime source, and lists those calls (ADR 0045,
	// handshake protocol 29).
	//
	// It is a statement of what the form's clearance would rest on, never a
	// claim that it clears: a consumer grants it only by asking each named
	// call's callee whether *its* completion is a primitive
	// (ExportImplementationTranscript.PrimitiveCompletion), under the very
	// premise the consumer demanded that callee's census under. Absent when
	// some operand is neither, which is every case no ADR has reviewed.
	CoercionPremise *CoercionPremise `cbor:"coercionPremise,omitempty" json:"coercionPremise,omitempty"`
	// LocalLiteralResult names a call whose every normal completion returns
	// the same unwritten local data-only literal binding. This is a source
	// identity fact, not a structural return-type assertion. Its callee must
	// still receive a complete execution census before a consumer uses it.
	LocalLiteralResult *LocalLiteralResultPremise `cbor:"localLiteralResult,omitempty" json:"localLiteralResult,omitempty"`
}

type LocalLiteralResultPremise struct {
	Call       Location   `cbor:"call" json:"call"`
	Callee     Location   `cbor:"callee" json:"callee"`
	Allocation Location   `cbor:"allocation" json:"allocation"`
	Returns    []Location `cbor:"returns" json:"returns"`
}

type ParameterValueSource struct {
	ParameterIndex int           `cbor:"parameterIndex" json:"parameterIndex"`
	Path           []PathSegment `cbor:"path,omitempty" json:"path,omitempty"`
}

type ImplementationCall struct {
	Location Location     `cbor:"location" json:"location"`
	Reach    Reachability `cbor:"reach" json:"reach"`
	// Kind separates `f(x)` from `new F(x)`. Both are recorded, because both
	// run the callables they are handed — `new Promise(executor)` runs its
	// executor synchronously, and a census that omitted it would leave the
	// executor's body unprovable — but they are not interchangeable: a
	// consumer whose claim is "this implementation *calls* the value" must be
	// able to refuse a construction, so the distinction is always transmitted
	// rather than left implicit in the absence of a field.
	//
	// A construct site deliberately carries no callee-parameter facts: the
	// three CalleeXxxParameters fields below are about the body of a resolved
	// *function*, and a constructor's resolution was not reviewed here.
	Kind            CallKind              `cbor:"kind" json:"kind"`
	Target          SymbolID              `cbor:"target,omitempty" json:"target,omitempty"`
	TargetName      string                `cbor:"targetName,omitempty" json:"targetName,omitempty"`
	TargetModule    string                `cbor:"targetModule,omitempty" json:"targetModule,omitempty"`
	Declaration     *ResolvedDeclaration  `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	CalleeParameter *ParameterValueSource `cbor:"calleeParameter,omitempty" json:"calleeParameter,omitempty"`
	// CalleeIteratedParameter names the parameter-rooted iterable whose
	// iteration produced this call's callee: the binding a `for…of` head
	// declares, called inside the loop (ADR 0042, handshake protocol 26).
	//
	// The value called is one the caller's iterable yielded, so it is the
	// caller's code exactly as a callee that *is* a parameter is — the
	// distinction the census draws between "this export performs it" and "the
	// caller's own code performs it", which the callbacks domain owns.
	//
	// Stated only for a plain, non-`await` `for…of` whose declaration list
	// declares exactly this one binding, which nothing in its file writes, and
	// whose iterated expression is parameter-rooted by the same walk
	// CalleeParameter uses. Absent otherwise, and absence is never "not
	// iterated".
	CalleeIteratedParameter *ParameterValueSource   `cbor:"calleeIteratedParameter,omitempty" json:"calleeIteratedParameter,omitempty"`
	ArgumentParameters      []*ParameterValueSource `cbor:"argumentParameters,omitempty" json:"argumentParameters,omitempty"`
	// CallReceiver is stated for a `.call` or `.apply` whose resolved callee is
	// the default library's Function.prototype member: the resolved declaration
	// of the *receiver* expression — `Object.prototype.toString` in
	// `Object.prototype.toString.call(value)` — so a consumer can decide the
	// invocation by that member rather than refusing every by-reference
	// transfer alike (ADR 0034, handshake protocol 18). Absent otherwise.
	CallReceiver *ResolvedDeclaration `cbor:"callReceiver,omitempty" json:"callReceiver,omitempty"`
	// ThisParameter accompanies CallReceiver: the parameter of this declaration
	// the `this` argument (slot 0) is rooted at, under exactly the premises
	// UncensusedInvokingForm.SubjectParameter states. Absent otherwise.
	ThisParameter *int `cbor:"thisParameter,omitempty" json:"thisParameter,omitempty"`
	// ArgumentSources is, per written argument slot, the value provenance of
	// the expression written in that slot: the same trace ReturnSite.Sources
	// carries for a returned expression, applied to an argument. One entry per
	// written argument slot, parallel to ArgumentParameters and subject to the
	// same exactArgumentSlots gate, so a slot a spread has displaced gets an
	// empty list rather than a trace of the expression written there.
	//
	// An empty list means the producer traced nothing. It never means "this
	// argument is not an accessor", never means "this argument is plain", and
	// never means the slot carries no provenance — the tracer follows array
	// literals, callable expressions, call results and one hop through a
	// single-assignment array binding element that is neither a rest element
	// nor defaulted, and everything else it declines to model leaves the list
	// empty. Every consumer must fail closed on an empty list.
	ArgumentSources [][]ImplementationValueSource `cbor:"argumentSources,omitempty" json:"argumentSources,omitempty"`
	// CalleeSources is the value provenance of the *callee expression* — the
	// same trace ReturnSite.Sources carries for a returned expression and
	// ArgumentSources carries for an argument, applied to node.Expression().
	//
	// It answers "what created the value being called", which is a different
	// question from every other callee fact on this struct. Target, TargetName,
	// TargetModule, Declaration and CalleeParameter state the *resolution* of
	// the callee — which symbol it is, which declaration, which parameter — and
	// they stay exactly as they are; a consumer that wants "the callee is
	// parameter N" keeps reading CalleeParameter. This field states instead
	// that the value in callee position came out of some other call: for
	// `const [read] = createSignal(0); … read()` the resolution is a
	// BindingElement and the provenance is `createSignal`'s tuple slot 0.
	//
	// An empty list means the producer traced nothing. It never means the
	// callee is not an accessor, never means the callee is plain, and never
	// means the callee carries no provenance — an ordinary `arr.push(x)` traces
	// nothing here, and so does a computed callee, a reassigned binding and
	// `(options.storage || createSignal)(…)`. Every consumer must fail closed
	// on an empty list.
	CalleeSources []ImplementationValueSource `cbor:"calleeSources,omitempty" json:"calleeSources,omitempty"`
	Captured      bool                        `cbor:"captured,omitempty" json:"captured,omitempty"`
	// EnclosingCallable is the exact source range of the *innermost* callable
	// that contains this call, or nil when the call sits directly in the
	// implementation's own body. It is the link a consumer needs to compose a
	// chain: a call is reached through a carried callable only when that
	// callable is the one immediately containing it, so that every callable
	// between must be shown to run on its own merits rather than assumed from
	// byte nesting. Captured is true exactly when this is non-nil.
	EnclosingCallable *Location `cbor:"enclosingCallable,omitempty" json:"enclosingCallable,omitempty"`
	// ArgumentCallables are, per argument slot, the exact source ranges of the
	// callables that slot provably carries — and carries by identity: the
	// callable expression itself, the wrappers that erase at runtime, and a
	// single-declaration binding naming exactly one callable. A literal that
	// stores several callables in one value is deliberately absent, because a
	// slot whose runtime picks one named property does not run the others.
	// Absence is never proof that a slot carries nothing.
	ArgumentCallables []ImplementationArgumentCallable `cbor:"argumentCallables,omitempty" json:"argumentCallables,omitempty"`
	// DefaultLibraryInvoker names the exact standard-library member this call's
	// callee resolves to, and InvokedArguments the argument slots that member's
	// runtime invokes zero or more times. Both are emitted only for a member of
	// a fixed reviewed table, resolved by default-library symbol identity
	// rather than by spelling; every other callee emits neither.
	DefaultLibraryInvoker DefaultLibraryInvoker `cbor:"defaultLibraryInvoker,omitempty" json:"defaultLibraryInvoker,omitempty"`
	InvokedArguments      []int                 `cbor:"invokedArguments,omitempty" json:"invokedArguments,omitempty"`
	// CalleeDirectlyCalledParameters are the parameter indices this call's
	// callee calls directly in its own body — the strongest of the three, and
	// the only one that by itself proves the argument at that slot is used as a
	// function.
	CalleeDirectlyCalledParameters []int `cbor:"calleeDirectlyCalledParameters,omitempty" json:"calleeDirectlyCalledParameters,omitempty"`
	// CalleeInvokedParameters are the parameter indices the callee's body sends
	// to *some* proven invoking position: called directly, forwarded to a
	// further local callee that invokes that slot, or handed to a reviewed
	// default-library invoker. Reaching a default-library invoker proves the
	// value runs; it does not prove the callee itself calls it.
	CalleeInvokedParameters []int `cbor:"calleeInvokedParameters,omitempty" json:"calleeInvokedParameters,omitempty"`
	// CalleeStronglyInvokedParameters are the parameter indices whose forwarding
	// chain is a plain identifier forward at every hop and *terminates in a
	// direct call*. A chain that ends at `addEventListener` is invoked but not
	// strongly invoked: it proves execution, not that this callee treats the
	// position as a function.
	CalleeStronglyInvokedParameters []int `cbor:"calleeStronglyInvokedParameters,omitempty" json:"calleeStronglyInvokedParameters,omitempty"`
	// CalleePendingInvocations are the same two claims, each still missing one
	// premise this producer may not decide: whether a named argument slot of a
	// named imported function is a callback position. The callee's body calls
	// its parameter from inside a callable it hands to that slot, so the claim
	// holds exactly when the slot invokes what it is given.
	//
	// The producer states the syntax and refuses to state the semantics: it
	// knows no framework vocabulary, and inferring one from a module and a
	// name is exactly the shortcut the precision contract forbids. The
	// verifier owns that table and answers each requirement itself; a
	// requirement it does not recognize leaves the claim unproven.
	//
	// An entry with no requirements is not an unconditional claim — it is a
	// malformed one, and a consumer must refuse it rather than read it as a
	// fact that needs nothing.
	CalleePendingInvocations []CalleePendingInvocation `cbor:"calleePendingInvocations,omitempty" json:"calleePendingInvocations,omitempty"`
}

// CalleePendingInvocation is one conditional callee-parameter claim: parameter
// Parameter of this call's callee is invoked (and, when Strong, invoked by a
// chain of plain forwards terminating in a direct call) provided every slot in
// Requires really does invoke the callable handed to it.
type CalleePendingInvocation struct {
	Parameter int                   `cbor:"parameter" json:"parameter"`
	Strong    bool                  `cbor:"strong,omitempty" json:"strong,omitempty"`
	Requires  []InvokingSlotPremise `cbor:"requires,omitempty" json:"requires,omitempty"`
}

// InvokingSlotPremise names one argument slot of one resolved imported callee,
// exactly as the source spells it: the module the callee was imported from,
// the name it was exported under, the slot, and the call's argument count —
// everything a dialect owner needs to answer "does this position run what it
// is given", and nothing that presumes the answer.
type InvokingSlotPremise struct {
	Module        string `cbor:"module" json:"module"`
	Name          string `cbor:"name" json:"name"`
	Slot          int    `cbor:"slot" json:"slot"`
	ArgumentCount int    `cbor:"argumentCount" json:"argumentCount"`
}

// ImplementationArgumentCallable binds one argument slot of a call to the exact
// source ranges of the callables that slot provably carries.
type ImplementationArgumentCallable struct {
	Argument  int        `cbor:"argument" json:"argument"`
	Locations []Location `cbor:"locations,omitempty" json:"locations,omitempty"`
}

// DefaultLibraryInvoker is the closed set of standard-library members this
// producer will vouch for as invoking one of their arguments. It is a closed
// enum rather than a free string because a consumer must be able to refuse an
// unrecognized value outright: an invoker nobody reviewed is not evidence.
//
// Membership is a reviewed act. `EventTarget.removeEventListener` is absent
// because removing a handler is not evidence anything runs, and
// `navigator.geolocation.watchPosition` is absent because "the browser probably
// calls it" is not a premise — growing this table means auditing the member,
// not noticing it.
type DefaultLibraryInvoker string

const (
	DefaultLibraryInvokerSetTimeout            DefaultLibraryInvoker = "setTimeout"
	DefaultLibraryInvokerSetInterval           DefaultLibraryInvoker = "setInterval"
	DefaultLibraryInvokerQueueMicrotask        DefaultLibraryInvoker = "queueMicrotask"
	DefaultLibraryInvokerRequestAnimationFrame DefaultLibraryInvoker = "requestAnimationFrame"
	DefaultLibraryInvokerRequestIdleCallback   DefaultLibraryInvoker = "requestIdleCallback"
	DefaultLibraryInvokerAddEventListener      DefaultLibraryInvoker = "addEventListener"
	DefaultLibraryInvokerPromiseThen           DefaultLibraryInvoker = "promiseThen"
	DefaultLibraryInvokerPromiseCatch          DefaultLibraryInvoker = "promiseCatch"
	DefaultLibraryInvokerPromiseFinally        DefaultLibraryInvoker = "promiseFinally"
	DefaultLibraryInvokerArrayIteration        DefaultLibraryInvoker = "arrayIteration"
	// DefaultLibraryInvokerPromiseConstructor is the one construct-expression
	// row: `new Promise(executor)` runs its executor synchronously, before the
	// constructor returns. It is emitted only for the exact default-library
	// `Promise` symbol, so a user class of that name and a locally shadowed
	// binding both stay open.
	DefaultLibraryInvokerPromiseConstructor DefaultLibraryInvoker = "promiseConstructor"
)

type ImplementationValueSourceKind string

const (
	ImplementationValueDirectCallable ImplementationValueSourceKind = "directCallable"
	ImplementationValueCallResult     ImplementationValueSourceKind = "callResult"
)

// ImplementationValueSource is one traced provenance of a value: what the
// expression at Path within the traced value is, and — for a call result —
// which callee and which slot of its result it came from.
//
// A *presence* is a positive fact. An *absence* is not: a value the tracer
// declined to model (a conditional, a property read, a reassignable binding, a
// computed callee) contributes no source, and a consumer that read an empty
// list as "not a call result" or "plain" would be reading the producer's
// silence as a claim. TargetModule is the written import specifier text and is
// empty for a locally declared callee, which is a fact about the source text
// rather than a resolved package identity.
type ImplementationValueSource struct {
	Path         []PathSegment                 `cbor:"path,omitempty" json:"path,omitempty"`
	Kind         ImplementationValueSourceKind `cbor:"kind" json:"kind"`
	Target       SymbolID                      `cbor:"target,omitempty" json:"target,omitempty"`
	TargetName   string                        `cbor:"targetName,omitempty" json:"targetName,omitempty"`
	TargetModule string                        `cbor:"targetModule,omitempty" json:"targetModule,omitempty"`
	TargetPath   []PathSegment                 `cbor:"targetPath,omitempty" json:"targetPath,omitempty"`
}

type DeclaredTypeReference struct {
	Name   string `cbor:"name" json:"name"`
	Module string `cbor:"module" json:"module"`
}

type SelectedParameter struct {
	Index         int                    `cbor:"index" json:"index"`
	Symbol        SymbolID               `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Declaration   *Declaration           `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Rest          bool                   `cbor:"rest,omitempty" json:"rest,omitempty"`
	Optional      bool                   `cbor:"optional,omitempty" json:"optional,omitempty"`
	Defaulted     bool                   `cbor:"defaulted,omitempty" json:"defaulted,omitempty"`
	Value         InvocationValueFact    `cbor:"value" json:"value"`
	DeclaredType  *DeclaredTypeReference `cbor:"declaredType,omitempty" json:"declaredType,omitempty"`
	CallablePaths []CallablePathFact     `cbor:"callablePaths,omitempty" json:"callablePaths,omitempty"`
}

type SelectedSignature struct {
	Identity             string              `cbor:"identity" json:"identity"`
	Declaration          ResolvedDeclaration `cbor:"declaration" json:"declaration"`
	OverloadOrdinal      int                 `cbor:"overloadOrdinal" json:"overloadOrdinal"`
	OverloadCount        int                 `cbor:"overloadCount" json:"overloadCount"`
	MinimumArgumentCount int                 `cbor:"minimumArgumentCount" json:"minimumArgumentCount"`
	HasRest              bool                `cbor:"hasRest,omitempty" json:"hasRest,omitempty"`
	Parameters           []SelectedParameter `cbor:"parameters,omitempty" json:"parameters,omitempty"`
	Result               InvocationValueFact `cbor:"result" json:"result"`
	ResultCallablePaths  []CallablePathFact  `cbor:"resultCallablePaths,omitempty" json:"resultCallablePaths,omitempty"`
}

type ParameterUseKind string

const (
	ParameterUseDirectCall      ParameterUseKind = "directCall"
	ParameterUseAliasCall       ParameterUseKind = "aliasCall"
	ParameterUseArgumentKnown   ParameterUseKind = "argumentKnown"
	ParameterUseArgumentUnknown ParameterUseKind = "argumentUnknown"
	ParameterUsePropertyAccess  ParameterUseKind = "propertyAccess"
	ParameterUseReturn          ParameterUseKind = "return"
	ParameterUseStorage         ParameterUseKind = "storage"
	ParameterUseCapture         ParameterUseKind = "capture"
	ParameterUseUnknownEscape   ParameterUseKind = "unknownEscape"
)

type ParameterUse struct {
	ParameterIndex int           `cbor:"parameterIndex" json:"parameterIndex"`
	BindingPath    []PathSegment `cbor:"bindingPath,omitempty" json:"bindingPath,omitempty"`
	Location       Location      `cbor:"location" json:"location"`
	// Reach is whether invoking the implementation reaches this use, answered by
	// the same body walk that answers it for a call in the same position. A use
	// after a `return`, after a `throw`, or in a branch a literal condition
	// excludes is `unreachable`; a use inside a loop body, a `switch`, or a
	// `try` is `unknown`, because control may not enter. Without it a consumer
	// cannot tell an executed read from one in dead code.
	Reach    Reachability     `cbor:"reach" json:"reach"`
	Kind     ParameterUseKind `cbor:"kind" json:"kind"`
	Alias    bool             `cbor:"alias,omitempty" json:"alias,omitempty"`
	Captured bool             `cbor:"captured,omitempty" json:"captured,omitempty"`
}

type Reachability string

const (
	Reachable    Reachability = "reachable"
	Unreachable  Reachability = "unreachable"
	ReachUnknown Reachability = "unknown"
)

type ReturnSite struct {
	Location Location             `cbor:"location" json:"location"`
	Reach    Reachability         `cbor:"reach" json:"reach"`
	Value    *InvocationValueFact `cbor:"value,omitempty" json:"value,omitempty"`
	// Parameter identifies an unchanged whole input binding, not its type.
	// Absence carries no identity premise.
	Parameter *ParameterValueSource `cbor:"parameter,omitempty" json:"parameter,omitempty"`
	// CarriedCallables are the exact source ranges of the callables this
	// returned value provably carries. A consumer asking whether a call inside
	// a nested callable is reachable through the returned value answers it by
	// containment: the call site lies within one of these ranges, or it does
	// not. An empty list is never proof that nothing is carried.
	CarriedCallables []Location `cbor:"carriedCallables,omitempty" json:"carriedCallables,omitempty"`
	// CarryReach is the lower-bound strength of this particular value-return
	// edge. It is absent only for a bare return. Reach remains the
	// optimistic control-flow observation used by existing consumers; a
	// conditional statement return therefore needs this separate premise before
	// it may authorize execution of the value it returns.
	CarryReach *Reachability               `cbor:"carryReach,omitempty" json:"carryReach,omitempty"`
	Sources    []ImplementationValueSource `cbor:"sources,omitempty" json:"sources,omitempty"`
}

// CallableReturnCensus is the exact return-carry census for one nested
// callable. Absence is never proof that the callable returns nothing.
type CallableReturnCensus struct {
	Callable Location                  `cbor:"callable" json:"callable"`
	Returns  []CallableReturnCarrySite `cbor:"returns,omitempty" json:"returns,omitempty"`
}

type CallableReturnCarrySite struct {
	Location         Location               `cbor:"location" json:"location"`
	Reach            Reachability           `cbor:"reach" json:"reach"`
	CarryReach       *Reachability          `cbor:"carryReach,omitempty" json:"carryReach,omitempty"`
	CarriedCallables []CallableCarryBinding `cbor:"carriedCallables,omitempty" json:"carriedCallables,omitempty"`
}

// CallableCarryBinding names one exact callable and whether the returned value
// carries it on every path through the return expression or only on a possible
// alternative. Unknown is usable only by a consumer asking a may-execute
// question; Unreachable carries no authority.
type CallableCarryBinding struct {
	Location Location     `cbor:"location" json:"location"`
	Reach    Reachability `cbor:"reach" json:"reach"`
}

type ThrowSite struct {
	Location Location     `cbor:"location" json:"location"`
	Reach    Reachability `cbor:"reach" json:"reach"`
}

type BranchSite struct {
	Location   Location          `cbor:"location" json:"location"`
	Reach      Reachability      `cbor:"reach" json:"reach"`
	Partitions []FinitePartition `cbor:"partitions,omitempty" json:"partitions,omitempty"`
}

// ControlFlowIncompletenessClass says *which* of two different things a
// control-flow marker means. The distinction is the whole point of the field:
// `unsupported` used to absorb both, and a consumer reading it could only
// refuse everything.
//
// The enum is closed. A consumer that receives a string outside it must reject
// the whole transcript rather than treat the row as unknown, exactly as for
// UncensusedInvokingFormKind: a producer that invented a class is a producer
// this vocabulary does not describe, and reading an unknown class as either arm
// picks the unsound one half the time.
type ControlFlowIncompletenessClass string

const (
	// ControlFlowReachabilityLowerBound is a construct this census walked in
	// full, and whose every enclosed site the shared body walk therefore
	// records — what is missing is only the *lower bound*: a loop body may
	// never run, a `catch` may never be entered, a `switch` clause may not be
	// selected. Every site inside such a construct carries reach `unknown`,
	// and no site is called `unreachable` on the strength of it.
	//
	// So a consumer asking a **may-execute** question — "is every callable
	// this body can reach enumerated here?" — is answered. A consumer asking a
	// *guarantee* question is not, which is why the marker still opens the
	// transcript.
	ControlFlowReachabilityLowerBound ControlFlowIncompletenessClass = "reachability-lower-bound"
	// ControlFlowUnaccounted is a construct whose flow this census cannot
	// account for at all: the reach rows around it may be wrong in either
	// direction, so neither a may-execute nor a guarantee question is
	// answered. Nothing a consumer can do with it but refuse.
	//
	// It is also the classifier's default. A marker with no reviewed class
	// arrives here, so a construct a future revision marks unsupported without
	// classifying refuses on arrival rather than passing as the admissible arm.
	ControlFlowUnaccounted ControlFlowIncompletenessClass = "flow-unaccounted"
)

// ControlFlowIncompleteness names one construct this control-flow census does
// not fully model, at its exact location, and classifies what is missing.
//
// One row per construct, so a body with two loops carries two rows; Unsupported
// stays one deduplicated marker string per *kind* for the consumers that
// already read it.
type ControlFlowIncompleteness struct {
	// Marker is the same string Unsupported carries for this construct, so a
	// consumer can join the two.
	Marker   string                         `cbor:"marker" json:"marker"`
	Class    ControlFlowIncompletenessClass `cbor:"class" json:"class"`
	Location Location                       `cbor:"location" json:"location"`
}

type ControlFlowCensus struct {
	Returns  []ReturnSite `cbor:"returns,omitempty" json:"returns,omitempty"`
	Throws   []ThrowSite  `cbor:"throws,omitempty" json:"throws,omitempty"`
	Branches []BranchSite `cbor:"branches,omitempty" json:"branches,omitempty"`
	// Unsupported is the deduplicated set of marker strings, unchanged: any
	// entry still means this census is incomplete and still appends
	// `controlFlowUnsupported` to the transcript's open reasons.
	Unsupported []string `cbor:"unsupported,omitempty" json:"unsupported,omitempty"`
	// Incompleteness is Unsupported with the two facts the marker string never
	// carried: *where* the construct is, and *which* of the two classes above
	// the incompleteness belongs to. Every marker in Unsupported has at least
	// one row here and every row's marker is in Unsupported; the client
	// enforces both, so a producer cannot state an unclassified marker.
	//
	// An *absent* list beside a nonempty Unsupported is a producer with no
	// classification at all, which a consumer must refuse. No decoder can
	// separate that from a present empty one, so the handshake protocol is the
	// discriminator — see TypeFactsHandshakeProtocol.
	Incompleteness []ControlFlowIncompleteness `cbor:"incompleteness,omitempty" json:"incompleteness,omitempty"`
}

type InvocationTranscript struct {
	Location          Location               `cbor:"location" json:"location"`
	Validity          ResolvedCallValidity   `cbor:"validity" json:"validity"`
	Kind              CallKind               `cbor:"kind" json:"kind"`
	Target            SymbolID               `cbor:"target,omitempty" json:"target,omitempty"`
	Targets           *CallTargetSet         `cbor:"targets,omitempty" json:"targets,omitempty"`
	SelectedSignature *SelectedSignature     `cbor:"selectedSignature,omitempty" json:"selectedSignature,omitempty"`
	Bindings          []ArgumentBinding      `cbor:"bindings,omitempty" json:"bindings,omitempty"`
	OmittedParameters []int                  `cbor:"omittedParameters,omitempty" json:"omittedParameters,omitempty"`
	ParameterUses     []ParameterUse         `cbor:"parameterUses,omitempty" json:"parameterUses,omitempty"`
	ControlFlow       *ControlFlowCensus     `cbor:"controlFlow,omitempty" json:"controlFlow,omitempty"`
	Completeness      InvocationCompleteness `cbor:"complete,omitempty" json:"complete,omitempty"`
	OpenReasons       []string               `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type TranscriptSourceDigest struct {
	Path   string `cbor:"path" json:"path"`
	SHA256 string `cbor:"sha256" json:"sha256"`
}

type InvocationEnvelope struct {
	ProjectID         string                   `cbor:"projectId,omitempty" json:"projectId,omitempty"`
	Generation        uint64                   `cbor:"generation" json:"generation"`
	DemandSHA256      string                   `cbor:"demandSha256" json:"demandSha256"`
	ModuleGraphSHA256 string                   `cbor:"moduleGraphSha256" json:"moduleGraphSha256"`
	SchemaSHA256      string                   `cbor:"schemaSha256,omitempty" json:"schemaSha256,omitempty"`
	ProducerBuild     string                   `cbor:"producerBuild,omitempty" json:"producerBuild,omitempty"`
	Sources           []TranscriptSourceDigest `cbor:"sources,omitempty" json:"sources,omitempty"`
	OpenReasons       []string                 `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type InvocationAnswer struct {
	Transcripts []InvocationTranscript `cbor:"transcripts,omitempty" json:"transcripts,omitempty"`
	Envelope    InvocationEnvelope     `cbor:"envelope" json:"envelope"`
}

type ExportValueAnswer struct {
	Transcripts []ExportValueTranscript `cbor:"transcripts,omitempty" json:"transcripts,omitempty"`
	Envelope    InvocationEnvelope      `cbor:"envelope" json:"envelope"`
}

// InvocationAnalyzer is the optional exact compiler capability behind the
// invocation lifecycle operation. A backend without it must fail the request;
// it may not synthesize a partial transcript from weaker Project methods.
type InvocationAnalyzer interface {
	InvocationTranscripts(context.Context, []InvocationDemand) (InvocationAnswer, error)
}

// ExportValueAnalyzer is intentionally separate from InvocationAnalyzer: an
// implementation cannot answer exported-value proof from a selected call.
type ExportValueAnalyzer interface {
	ExportValueTranscripts(context.Context, []ExportValueDemand) (ExportValueAnswer, error)
}
