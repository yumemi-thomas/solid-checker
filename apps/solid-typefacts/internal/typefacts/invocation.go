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
	OpenReasons      []string                   `cbor:"openReasons,omitempty" json:"openReasons,omitempty"`
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

type SelectedParameter struct {
	Index         int                 `cbor:"index" json:"index"`
	Symbol        SymbolID            `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Declaration   *Declaration        `cbor:"declaration,omitempty" json:"declaration,omitempty"`
	Rest          bool                `cbor:"rest,omitempty" json:"rest,omitempty"`
	Optional      bool                `cbor:"optional,omitempty" json:"optional,omitempty"`
	Defaulted     bool                `cbor:"defaulted,omitempty" json:"defaulted,omitempty"`
	Value         InvocationValueFact `cbor:"value" json:"value"`
	CallablePaths []CallablePathFact  `cbor:"callablePaths,omitempty" json:"callablePaths,omitempty"`
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
	ParameterIndex int              `cbor:"parameterIndex" json:"parameterIndex"`
	BindingPath    []PathSegment    `cbor:"bindingPath,omitempty" json:"bindingPath,omitempty"`
	Location       Location         `cbor:"location" json:"location"`
	Kind           ParameterUseKind `cbor:"kind" json:"kind"`
	Alias          bool             `cbor:"alias,omitempty" json:"alias,omitempty"`
	Captured       bool             `cbor:"captured,omitempty" json:"captured,omitempty"`
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
	Captures []int                `cbor:"captures,omitempty" json:"captures,omitempty"`
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

// InvocationAnalyzer is the optional exact compiler capability behind the
// invocation lifecycle operation. A backend without it must fail the request;
// it may not synthesize a partial transcript from weaker Project methods.
type InvocationAnalyzer interface {
	InvocationTranscripts(context.Context, []InvocationDemand) (InvocationAnswer, error)
}
