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
	CallableDepth          int       `cbor:"callableDepth,omitempty" json:"callableDepth,omitempty"`
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
	Alternative       int                        `cbor:"alternative" json:"alternative"`
	Path              []PathSegment              `cbor:"path,omitempty" json:"path,omitempty"`
	Presence          PathPresence               `cbor:"presence" json:"presence"`
	Callability       Callability                `cbor:"callability" json:"callability"`
	Constructability  InvocationConstructability `cbor:"constructability" json:"constructability"`
	Declaration       *Declaration               `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Complete          bool                       `cbor:"complete,omitempty" json:"complete,omitempty"`
	SubtreeEnumerated bool                       `cbor:"subtreeEnumerated" json:"subtreeEnumerated"`
	OpenReasons       []string                   `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
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
	Complete       bool                            `cbor:"complete,omitempty" json:"complete,omitempty"`
	OpenReasons    []string                        `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
}

type ExportImplementationTranscript struct {
	Location      Location             `cbor:"location" json:"location"`
	QueryName     string               `cbor:"queryName,omitempty" json:"queryName,omitempty"`
	Target        SymbolID             `cbor:"target,omitempty" json:"target,omitempty"`
	Declaration   *ResolvedDeclaration `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Signature     *SelectedSignature   `cbor:"signature,omitempty" json:"signature,omitempty"`
	ParameterUses []ParameterUse       `cbor:"parameterUses,omitempty" json:"parameterUses,omitempty"`
	ControlFlow   *ControlFlowCensus   `cbor:"controlFlow,omitempty" json:"controlFlow,omitempty"`
	Calls         []ImplementationCall `cbor:"calls,omitempty" json:"calls,omitempty"`
	Complete      bool                 `cbor:"complete,omitempty" json:"complete,omitempty"`
	OpenReasons   []string             `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
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
	Kind               CallKind                `cbor:"kind" json:"kind"`
	Target             SymbolID                `cbor:"target,omitempty" json:"target,omitempty"`
	TargetName         string                  `cbor:"targetName,omitempty" json:"targetName,omitempty"`
	TargetModule       string                  `cbor:"targetModule,omitempty" json:"targetModule,omitempty"`
	Declaration        *ResolvedDeclaration    `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	CalleeParameter    *ParameterValueSource   `cbor:"calleeParameter,omitempty" json:"calleeParameter,omitempty"`
	ArgumentParameters []*ParameterValueSource `cbor:"argumentParameters,omitempty" json:"argumentParameters,omitempty"`
	Captured           bool                    `cbor:"captured,omitempty" json:"captured,omitempty"`
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
	// CarriedCallables are the exact source ranges of the callables this
	// returned value provably carries. A consumer asking whether a call inside
	// a nested callable is reachable through the returned value answers it by
	// containment: the call site lies within one of these ranges, or it does
	// not. An empty list is never proof that nothing is carried.
	CarriedCallables []Location                  `cbor:"carriedCallables,omitempty" json:"carriedCallables,omitempty"`
	Sources          []ImplementationValueSource `cbor:"sources,omitempty" json:"sources,omitempty"`
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

type ControlFlowCensus struct {
	Returns     []ReturnSite `cbor:"returns,omitempty" json:"returns,omitempty"`
	Throws      []ThrowSite  `cbor:"throws,omitempty" json:"throws,omitempty"`
	Branches    []BranchSite `cbor:"branches,omitempty" json:"branches,omitempty"`
	Unsupported []string     `cbor:"unsupported,omitempty" json:"unsupported,omitempty"`
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
