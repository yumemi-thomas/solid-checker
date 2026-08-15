// Package typefacts defines the compiler-independent seam through which a
// consumer asks questions about a configured TypeScript project, and the v3
// protocol the producer answers them over.
package typefacts

import (
	"context"
	"errors"
)

var ErrNotFound = errors.New("type fact not found")

// SymbolID is an opaque identity stable for one Project analysis version.
// Implementations may keep an ID resolvable across updates while its
// declaration is unchanged (durable symbol identity); holders must treat
// cross-update resolution as best-effort — it either answers for the same
// declaration or reports ErrNotFound, never a different symbol.
type SymbolID string

// RuntimeSymbolID is a declaration-derived identity used only for equality
// across aliases and reexports. Unlike SymbolID it is not a lookup handle.
type RuntimeSymbolID string

// TypeDescriptor exposes source identity for named types without leaking a
// backend AST. It is available through the optional TypeDescriber capability.
type TypeDescriptor struct {
	Text              string
	OriginModule      string
	AliasDeclarations []Declaration
}

type TypeDescriber interface {
	DescribeTypeAt(context.Context, Location) (TypeDescriptor, error)
}

// Callability is the compiler's call-signature classification for a demanded
// expression. It is derived from TypeChecker.GetSignaturesOfType over the
// actual union constituents, never from rendered type text.
type Callability string

const (
	CallabilityCallable    Callability = "callable"
	CallabilityNonCallable Callability = "nonCallable"
	CallabilityMixed       Callability = "mixed"
	CallabilityUnknown     Callability = "unknown"
)

// RuntimeValueDomain summarizes the possible runtime values of a demanded
// expression without exposing a compiler type or relying on rendered type
// text. Unknown means the checker could not provide a closed value domain; in
// that case the three MayBe fields are conservative possibilities rather than
// an exhaustive classification.
//
// The zero value is meaningful: it is the known empty domain produced by
// never. EntityFact uses a pointer so an undemanded fact remains distinct.
type RuntimeValueDomain struct {
	MayBeCallable  bool `cbor:"mayBeCallable,omitempty" json:"mayBeCallable,omitempty"`
	MayBeUndefined bool `cbor:"mayBeUndefined,omitempty" json:"mayBeUndefined,omitempty"`
	MayBeOther     bool `cbor:"mayBeOther,omitempty" json:"mayBeOther,omitempty"`
	Unknown        bool `cbor:"unknown,omitempty" json:"unknown,omitempty"`
}

// ReferenceSpace summarizes the semantic meaning of all compiler-resolved
// references to an imported or aliased symbol.
type ReferenceSpace string

const (
	ReferenceSpaceValue   ReferenceSpace = "value"
	ReferenceSpaceType    ReferenceSpace = "type"
	ReferenceSpaceBoth    ReferenceSpace = "both"
	ReferenceSpaceNeither ReferenceSpace = "neither"
)

// SemanticEntityLookup is the demand-shaped compiler-fact interface used by
// protocol producers and integration tests.
type SemanticEntityLookup interface {
	SemanticEntities(context.Context, []EntityDemand) ([]EntityFact, error)
}

// SourceCall describes one parsed call expression without exposing backend AST
// nodes. Target is alias-resolved for the current project generation.
type SourceCall struct {
	Location  Location
	Callee    Location
	Arguments []Location
	Target    SymbolID
}

// CallDiscoverer is an optional bulk syntax capability. Implementations return
// calls in source order with parser-derived callee and argument boundaries.
type CallDiscoverer interface {
	SourceCalls(context.Context, string) ([]SourceCall, error)
}

// SourceBinding describes a variable initialized directly by a resolved call.
// Names contains one entry for a direct identifier, or one entry per top-level
// array binding slot; omitted or nested slots have zero-value locations.
type SourceBinding struct {
	Array       bool
	Names       []Location
	Initializer SourceCall
}

// BindingDiscoverer is an optional bulk syntax capability for call-initialized
// variable declarations.
type BindingDiscoverer interface {
	SourceBindings(context.Context, string) ([]SourceBinding, error)
}

// SourceFunction describes a named block-bodied function without exposing its
// backend AST node. Parameters retain their complete declaration ranges.
type SourceFunction struct {
	Name       Location
	Body       Location
	Parameters []Location
	Exported   bool
	Async      bool
	Arrow      bool
}

// FunctionDiscoverer is an optional bulk syntax capability for named function
// declarations and direct identifier-bound arrow functions.
type FunctionDiscoverer interface {
	SourceFunctions(context.Context, string) ([]SourceFunction, error)
}

// AsyncFunctionFact describes a function-like expression or declaration using
// parser and checker facts. Target links a local identifier alias to the
// summarized function symbol. CallsAfterAwait contains call expressions whose
// execution is dominated by await on every reachable AST control-flow path;
// calls inside nested functions are excluded.
type AsyncFunctionFact struct {
	Expression      Location
	Symbol          SymbolID
	Target          SymbolID
	CanReturnAsync  bool
	CallsAfterAwait []Location
}

// AsyncFunctionDiscoverer is an optional semantic async/control-flow
// capability. It keeps backend AST details behind the Type Facts seam.
type AsyncFunctionDiscoverer interface {
	SourceAsyncFunctions(context.Context, string) ([]AsyncFunctionFact, error)
}

// AsyncFunctionLookup is the demand-shaped async/control-flow capability the
// retained analysis uses. Implementations return only the function and
// local-alias facts relevant at the requested locations.
type AsyncFunctionLookup interface {
	AsyncFunctionsAt(context.Context, []Location) ([]AsyncFunctionFact, error)
}

// Location identifies a UTF-8 byte range in original source.
type Location struct {
	Path      string `cbor:"path" json:"path"`
	StartByte int    `cbor:"startByte" json:"startByte"`
	EndByte   int    `cbor:"endByte" json:"endByte"`
}

// Declaration is the source-only description of a symbol declaration.
type Declaration struct {
	Name     string   `cbor:"name" json:"name"`
	Kind     string   `cbor:"kind" json:"kind"`
	Location Location `cbor:"location" json:"location"`
}

// ResolvedCallValidity distinguishes a compiler-selected signature from the
// recovery signatures TypeScript creates while reporting failed resolution.
type ResolvedCallValidity string

const (
	ResolvedCallValid      ResolvedCallValidity = "valid"
	ResolvedCallRecovery   ResolvedCallValidity = "recovery"
	ResolvedCallUnresolved ResolvedCallValidity = "unresolved"
)

// CallKind distinguishes ordinary invocation from construction.
type CallKind string

const (
	CallKindUnknown   CallKind = "unknown"
	CallKindCall      CallKind = "call"
	CallKindConstruct CallKind = "construct"
)

// ResolvedDeclaration identifies the declaration selected by overload
// resolution. Symbol and each owner symbol are compiler-resolved identities;
// names and QualifiedName are display metadata.
type ResolvedDeclaration struct {
	Symbol          SymbolID
	Name            string
	Kind            string
	Location        Location
	Owners          []DeclarationOwner
	QualifiedName   string
	OriginModule    string
	SourceFile      string
	StandardLibrary bool
}

// DeclarationOwner is one compiler declaration containing a selected
// signature declaration, ordered outermost to innermost.
type DeclarationOwner struct {
	Symbol   SymbolID
	Name     string
	Kind     string
	Location Location
}

// ArgumentMappingStatus says whether TypeScript exposes one exact formal
// parameter for a supplied argument.
type ArgumentMappingStatus string

const (
	ArgumentMappingResolved   ArgumentMappingStatus = "resolved"
	ArgumentMappingUnresolved ArgumentMappingStatus = "unresolved"
)

// ArgumentMappingReason explains why a supplied argument has no exact formal
// parameter mapping.
type ArgumentMappingReason string

const (
	ArgumentMappingCallUnresolved       ArgumentMappingReason = "callUnresolved"
	ArgumentMappingRecoverySignature    ArgumentMappingReason = "recoverySignature"
	ArgumentMappingCompositeSignature   ArgumentMappingReason = "compositeSignature"
	ArgumentMappingSpreadArgument       ArgumentMappingReason = "spreadArgument"
	ArgumentMappingParameterUnavailable ArgumentMappingReason = "parameterUnavailable"
)

// ParameterFact describes the selected signature's formal parameter after
// generic substitution at one argument position.
type ParameterFact struct {
	Index          int
	Symbol         SymbolID
	Declaration    *Declaration
	Rest           bool
	Optional       bool
	Callability    Callability
	TypeDescriptor *TypeDescriptor
}

// ArgumentMapping relates one supplied argument to its exact formal parameter,
// or carries an explicit reason that no exact mapping exists.
type ArgumentMapping struct {
	ArgumentIndex int
	Status        ArgumentMappingStatus
	Unresolved    ArgumentMappingReason
	Parameter     *ParameterFact
}

// CallTargetSet is a finite set of exact callable declarations for one call.
// Exhaustive is an explicit compiler proof that Candidates cover every call
// signature of the callee's apparent type: every union constituent was a
// closed concrete callable and every one of its signatures named one exact
// implementation declaration. A set without that proof bit must never be
// treated as the complete runtime dispatch set. Candidates are deduplicated
// and ordered deterministically by declaration location, then symbol.
type CallTargetSet struct {
	Exhaustive bool
	Candidates []ResolvedDeclaration
}

// Call describes the target and instantiated return type of a demanded call.
// The return type is carried as text only: the opaque per-generation identity
// that used to accompany it had no consumer, and because it embedded the
// generation number it made every entity row holding a resolved call compare as
// changed on every generation, inflating each delta.
type Call struct {
	Target         SymbolID
	ReturnTypeText string
	Validity       ResolvedCallValidity
	Kind           CallKind
	Declaration    *ResolvedDeclaration
	// Targets carries the exact candidate declarations of a composite
	// (union) callee when the compiler proves the set exhaustive. It is
	// complementary to Declaration, which stays nil for composite callees
	// because no single signature was selected.
	Targets   *CallTargetSet
	Arguments []ArgumentMapping
}

// FileChange is one monotonically-versioned editor overlay change.
type FileChange struct {
	Path    string
	Version uint64
	Source  []byte
	Deleted bool
}

// AffectedSet lists normalized source paths invalidated by an update.
type AffectedSet struct {
	Files []string
}

// SourceFile is an original project source and its normalized path. This bulk
// view lets compiler adapters analyze project inputs without exposing TS ASTs.
type SourceFile struct {
	Path   string
	Source []byte
}

// Project provides type facts for one configured TypeScript project.
type Project interface {
	SourceFiles(context.Context) ([]SourceFile, error)
	Update(context.Context, []FileChange) (AffectedSet, error)
	SymbolAt(context.Context, Location) (SymbolID, error)
	ResolveAlias(context.Context, SymbolID) (SymbolID, error)
	Declarations(context.Context, SymbolID) ([]Declaration, error)
	References(context.Context, SymbolID) ([]Location, error)
	Close() error
}
