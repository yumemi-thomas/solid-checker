package typefacts

import (
	"errors"
	"fmt"
	"regexp"
)

// TypeFacts v2 wire model is the frozen closure-request schema consumed by the
// Rust checker. It replaces v1's client-enumerated demand keys with a request:
// seeds plus a pinned expansion-ruleset version in, one closed fact table per
// generation out. The v1 types in protocol.go and materialized.go remain
// frozen evidence of the rejected eager-materialization candidate.
//
// All offsets are unsigned 64-bit per the wire codec rules. Optional fields
// are omitted, never null. Field names are the cbor/json tags below; the
// golden fixtures under benchmarks/phase1/ pin the deterministic bytes.
const (
	// TypeFactsSchemaVersionV2 identifies the closure-request wire schema.
	TypeFactsSchemaVersionV2 uint64 = 2
	// ExpansionRulesetVersionV1 identifies the normative seed and expansion
	// canonical expansion rules. Changing the rules bumps this version.
	ExpansionRulesetVersionV1 uint64 = 1
)

var (
	ErrRulesetMismatch  = errors.New("type facts expansion ruleset version mismatch")
	ErrSourceHash       = errors.New("type facts source hash mismatch")
	ErrAliasReferences  = errors.New("type facts alias symbol carries references")
	ErrClosureGap       = errors.New("type facts closure gap")
	ErrReferencesUnkept = errors.New("type facts references not included")
)

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
	Target         string `cbor:"target,omitempty" json:"target,omitempty"`
	ReturnTypeText string `cbor:"returnTypeText,omitempty" json:"returnTypeText,omitempty"`
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
	Location       LocationV2        `cbor:"location" json:"location"`
	Symbol         string            `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	TypeDescriptor *TypeDescriptorV2 `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall   *CallV2           `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
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

// ClosureRequest asks the type-facts service for one generation's demand
// closure. compilerSpans carries the ExecutionMap-derived seed spans
// (callback roles, JSX operations) sorted by path, start, end; every other
// seed class is derived service-side from sources the service owns.
type ClosureRequest struct {
	Schema         uint64       `cbor:"schema" json:"schema"`
	ProjectID      string       `cbor:"projectId" json:"projectId"`
	Generation     uint64       `cbor:"generation" json:"generation"`
	RulesetVersion uint64       `cbor:"rulesetVersion" json:"rulesetVersion"`
	CompilerSpans  []LocationV2 `cbor:"compilerSpans,omitempty" json:"compilerSpans,omitempty"`
}

// ClosureResponse answers a ClosureRequest with the closed table.
type ClosureResponse struct {
	Schema     uint64      `cbor:"schema" json:"schema"`
	ProjectID  string      `cbor:"projectId" json:"projectId"`
	Generation uint64      `cbor:"generation" json:"generation"`
	Table      FactTableV2 `cbor:"table" json:"table"`
}

var sourceDigestPattern = regexp.MustCompile(`^sha256:[0-9a-f]{64}$`)

// ValidateClosureRequest enforces schema and ruleset identity and canonical
// seed-span ordering.
func ValidateClosureRequest(request ClosureRequest) error {
	if request.Schema != TypeFactsSchemaVersionV2 {
		return fmt.Errorf("unsupported TypeFacts schema %d", request.Schema)
	}
	if request.RulesetVersion != ExpansionRulesetVersionV1 {
		return fmt.Errorf("%w: %d", ErrRulesetMismatch, request.RulesetVersion)
	}
	if request.ProjectID == "" || request.Generation == 0 {
		return ErrGenerationMismatch
	}
	for index := 1; index < len(request.CompilerSpans); index++ {
		left, right := request.CompilerSpans[index-1], request.CompilerSpans[index]
		if left.Path > right.Path || (left.Path == right.Path && (left.StartByte > right.StartByte ||
			(left.StartByte == right.StartByte && left.EndByte > right.EndByte))) {
			return fmt.Errorf("compilerSpans not in canonical order at index %d", index)
		}
	}
	return nil
}

// ValidateClosureResponse enforces schema and generation identity, source
// digest shape, and the canonical-reference-storage invariant.
func ValidateClosureResponse(request ClosureRequest, response ClosureResponse) error {
	if response.Schema != TypeFactsSchemaVersionV2 || response.Table.Schema != TypeFactsSchemaVersionV2 {
		return fmt.Errorf("unsupported TypeFacts schema %d", response.Schema)
	}
	if response.ProjectID != request.ProjectID || response.Generation != request.Generation ||
		response.Table.ProjectID != request.ProjectID || response.Table.Generation != request.Generation {
		return ErrGenerationMismatch
	}
	for _, source := range response.Table.Sources {
		if !sourceDigestPattern.MatchString(source.SHA256) {
			return fmt.Errorf("%w: %s", ErrSourceHash, source.Path)
		}
	}
	for _, symbol := range response.Table.Symbols {
		if symbol.AliasTarget != "" && len(symbol.References) != 0 {
			return fmt.Errorf("%w: %s", ErrAliasReferences, symbol.ID)
		}
	}
	return nil
}
