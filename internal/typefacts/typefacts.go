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
	Path      string
	StartByte int
	EndByte   int
}

// Declaration is the source-only description of a symbol declaration.
type Declaration struct {
	Name     string
	Kind     string
	Location Location
}

// ResolvedCallValidity distinguishes a compiler-selected signature from the
// recovery signatures TypeScript creates while reporting failed resolution.
type ResolvedCallValidity string

const (
	ResolvedCallValid      ResolvedCallValidity = "valid"
	ResolvedCallRecovery   ResolvedCallValidity = "recovery"
	ResolvedCallUnresolved ResolvedCallValidity = "unresolved"
)

// Call describes the target and instantiated return type of a demanded call.
// The return type is carried as text only: the opaque per-generation identity
// that used to accompany it had no consumer, and because it embedded the
// generation number it made every entity row holding a resolved call compare as
// changed on every generation, inflating each delta.
type Call struct {
	Target         SymbolID
	ReturnTypeText string
	Validity       ResolvedCallValidity
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
	ResolvedCall(context.Context, Location) (Call, error)
	Close() error
}
