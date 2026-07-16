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
