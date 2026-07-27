package typefacts

import "errors"

// The v2-shaped fact table is the model the producer materializes and the
// packed v3 frame carries. Its row types are also the delta wire shapes in
// FactTableDeltaV3, so the "V2" suffix names the fact-table model version —
// not the retired v2 closure protocol.
//
// All offsets are unsigned 64-bit per the wire codec rules. Optional fields
// are omitted, never null. Field names are the cbor/json tags below.

// TypeFactsSchemaVersion is the schema stamped on the internal fact table
// materialized inside the producer; the transport model version is
// TypeFactsSchemaVersionV2.
const TypeFactsSchemaVersion uint64 = 1

var ErrGenerationMismatch = errors.New("type facts generation mismatch")

// TypeFactsSchemaVersionV2 identifies the fact-table model carried in the
// packed frame and echoed as FactTable.schema by the Rust client.
const TypeFactsSchemaVersionV2 uint64 = 2

// LocationV2 is a UTF-8 byte range in original source.
type LocationV2 struct {
	Path      string `cbor:"path" json:"path"`
	StartByte uint64 `cbor:"startByte" json:"startByte"`
	EndByte   uint64 `cbor:"endByte" json:"endByte"`
}

// DeclarationV2 is the source-only description of a symbol declaration.
type DeclarationV2 struct {
	Name     string     `cbor:"name" json:"name"`
	Kind     string     `cbor:"kind" json:"kind"`
	Location LocationV2 `cbor:"location" json:"location"`
}

// CallV2 describes a resolved call target. v1's opaque return-type identity
// is deleted (zero measured demand); the instantiated return type text stays.
type CallV2 struct {
	Target         string               `cbor:"target,omitempty" json:"target,omitempty"`
	ReturnTypeText string               `cbor:"returnTypeText,omitempty" json:"returnTypeText,omitempty"`
	Validity       ResolvedCallValidity `cbor:"validity" json:"validity"`
}

// TypeDescriptorV2 exposes source identity for named types.
type TypeDescriptorV2 struct {
	Text              string          `cbor:"text,omitempty" json:"text,omitempty"`
	OriginModule      string          `cbor:"originModule,omitempty" json:"originModule,omitempty"`
	AliasDeclarations []DeclarationV2 `cbor:"aliasDeclarations,omitempty" json:"aliasDeclarations,omitempty"`
}

// EntityFactV2 is one location-keyed entity. v1's opaque type identity field
// is deleted (zero measured demand).
type EntityFactV2 struct {
	Location        LocationV2        `cbor:"location" json:"location"`
	Symbol          string            `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	TypeDescriptor  *TypeDescriptorV2 `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall    *CallV2           `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
	Callability     Callability       `cbor:"callability,omitempty" json:"callability,omitempty"`
	ReferenceSpace  ReferenceSpace    `cbor:"referenceSpace,omitempty" json:"referenceSpace,omitempty"`
	RuntimeIdentity string            `cbor:"runtimeIdentity,omitempty" json:"runtimeIdentity,omitempty"`
}

// SymbolFactV2 carries a generation-scoped symbol's facts under canonical
// reference storage: reference lists live on non-alias symbols only, and
// alias symbols carry aliasTarget for lookups to chase. An alias symbol with
// a references field is a decode error (ErrAliasReferences).
type SymbolFactV2 struct {
	ID           string          `cbor:"id" json:"id"`
	AliasTarget  string          `cbor:"aliasTarget,omitempty" json:"aliasTarget,omitempty"`
	Declarations []DeclarationV2 `cbor:"declarations,omitempty" json:"declarations,omitempty"`
	References   []LocationV2    `cbor:"references,omitempty" json:"references,omitempty"`
}

// SourceCallV2 is one parsed call expression.
type SourceCallV2 struct {
	Location  LocationV2   `cbor:"location" json:"location"`
	Callee    LocationV2   `cbor:"callee" json:"callee"`
	Arguments []LocationV2 `cbor:"arguments,omitempty" json:"arguments,omitempty"`
	Target    string       `cbor:"target,omitempty" json:"target,omitempty"`
}

// SourceBindingV2 is one call-initialized variable declaration.
type SourceBindingV2 struct {
	Array       bool         `cbor:"array,omitempty" json:"array,omitempty"`
	Names       []LocationV2 `cbor:"names" json:"names"`
	Initializer SourceCallV2 `cbor:"initializer" json:"initializer"`
}

// SourceFunctionV2 is one named block-bodied function or identifier-bound
// arrow.
type SourceFunctionV2 struct {
	Name       LocationV2   `cbor:"name" json:"name"`
	Body       LocationV2   `cbor:"body" json:"body"`
	Parameters []LocationV2 `cbor:"parameters,omitempty" json:"parameters,omitempty"`
	Exported   bool         `cbor:"exported,omitempty" json:"exported,omitempty"`
	Async      bool         `cbor:"async,omitempty" json:"async,omitempty"`
	Arrow      bool         `cbor:"arrow,omitempty" json:"arrow,omitempty"`
}

// AsyncFunctionFactV2 is one function-like expression's async facts.
type AsyncFunctionFactV2 struct {
	Expression      LocationV2   `cbor:"expression" json:"expression"`
	Symbol          string       `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Target          string       `cbor:"target,omitempty" json:"target,omitempty"`
	CanReturnAsync  bool         `cbor:"canReturnAsync,omitempty" json:"canReturnAsync,omitempty"`
	CallsAfterAwait []LocationV2 `cbor:"callsAfterAwait,omitempty" json:"callsAfterAwait,omitempty"`
}

// FileFactV2 carries one file's bulk syntax and semantic tables.
type FileFactV2 struct {
	Path           string                `cbor:"path" json:"path"`
	Calls          []SourceCallV2        `cbor:"calls,omitempty" json:"calls,omitempty"`
	Bindings       []SourceBindingV2     `cbor:"bindings,omitempty" json:"bindings,omitempty"`
	Functions      []SourceFunctionV2    `cbor:"functions,omitempty" json:"functions,omitempty"`
	AsyncFunctions []AsyncFunctionFactV2 `cbor:"asyncFunctions,omitempty" json:"asyncFunctions,omitempty"`
}

// SourceDigestV2 is the per-generation source consistency handshake: v2
// ships hashes, never source bytes. A digest mismatch between consumer and
// service fails the generation closed (ErrSourceHash).
type SourceDigestV2 struct {
	Path   string `cbor:"path" json:"path"`
	SHA256 string `cbor:"sha256" json:"sha256"`
}

// FactTableV2 is one generation's closed fact table.
type FactTableV2 struct {
	Schema     uint64           `cbor:"schema" json:"schema"`
	Generation uint64           `cbor:"generation" json:"generation"`
	ProjectID  string           `cbor:"projectId" json:"projectId"`
	Sources    []SourceDigestV2 `cbor:"sources" json:"sources"`
	Entities   []EntityFactV2   `cbor:"entities" json:"entities"`
	Symbols    []SymbolFactV2   `cbor:"symbols" json:"symbols"`
	Files      []FileFactV2     `cbor:"files" json:"files"`
}
