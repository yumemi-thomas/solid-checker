// Package typefacts defines the compiler-independent seam through which the
// certification engine asks TypeScript project questions.
package typefacts

import (
	"context"
	"errors"
)

var ErrNotFound = errors.New("type fact not found")

// OpenProjectFunc constructs a Type Facts project at the engine seam.
type OpenProjectFunc func(context.Context, string) (Project, error)

// SymbolID is an opaque identity stable for one Project analysis version.
type SymbolID string

// TypeID is an opaque identity stable for one Project analysis version.
type TypeID string

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

// Call describes the target and instantiated return type of a resolved call.
type Call struct {
	Target         SymbolID
	ReturnType     TypeID
	ReturnTypeText string
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
	TypeAt(context.Context, Location) (TypeID, error)
	ResolvedCall(context.Context, Location) (Call, error)
	Close() error
}
