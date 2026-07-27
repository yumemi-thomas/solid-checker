package typefacts

// EntityDemand selects only the fields consumed for one canonical entity.
//
// Every flag here is honoured. Alias targets and declarations are not flags:
// they arrive unconditionally through symbol closure, so the request has no way
// to ask for them and no reason to.
type EntityDemand struct {
	Location           Location  `cbor:"location" json:"location"`
	QueryLocation      *Location `cbor:"queryLocation,omitempty" json:"queryLocation,omitempty"`
	Symbol             bool      `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	TypeDescriptor     bool      `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall       bool      `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
	Callability        bool      `cbor:"callability,omitempty" json:"callability,omitempty"`
	ReferenceSpace     bool      `cbor:"referenceSpace,omitempty" json:"referenceSpace,omitempty"`
	RuntimeIdentity    bool      `cbor:"runtimeIdentity,omitempty" json:"runtimeIdentity,omitempty"`
	References         bool      `cbor:"references,omitempty" json:"references,omitempty"`
	Async              bool      `cbor:"async,omitempty" json:"async,omitempty"`
	StructuralAccessor bool      `cbor:"structuralAccessor,omitempty" json:"structuralAccessor,omitempty"`
}

// EntityFact is one legal location-keyed entity in the finite fact universe.
// Location ranges are ordered from outermost to innermost during encoding.
type EntityFact struct {
	Location        Location        `cbor:"location" json:"location"`
	Symbol          SymbolID        `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	TypeDescriptor  *TypeDescriptor `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall    *Call           `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
	Callability     Callability     `cbor:"callability,omitempty" json:"callability,omitempty"`
	ReferenceSpace  ReferenceSpace  `cbor:"referenceSpace,omitempty" json:"referenceSpace,omitempty"`
	RuntimeIdentity RuntimeSymbolID `cbor:"runtimeIdentity,omitempty" json:"runtimeIdentity,omitempty"`
}

// SymbolFact contains every legal symbol-keyed response for a generation.
type SymbolFact struct {
	ID           SymbolID      `cbor:"id" json:"id"`
	AliasTarget  SymbolID      `cbor:"aliasTarget,omitempty" json:"aliasTarget,omitempty"`
	Declarations []Declaration `cbor:"declarations,omitempty" json:"declarations,omitempty"`
	References   []Location    `cbor:"references,omitempty" json:"references,omitempty"`
}

// FileFact contains bulk syntax and semantic tables for one source file.
type FileFact struct {
	Path           string              `cbor:"path" json:"path"`
	Calls          []SourceCall        `cbor:"calls,omitempty" json:"calls,omitempty"`
	Bindings       []SourceBinding     `cbor:"bindings,omitempty" json:"bindings,omitempty"`
	Functions      []SourceFunction    `cbor:"functions,omitempty" json:"functions,omitempty"`
	AsyncFunctions []AsyncFunctionFact `cbor:"asyncFunctions,omitempty" json:"asyncFunctions,omitempty"`
}

// FactTable is the deterministic TypeFactsSchema v1 payload. Slices, rather
// than maps, are used on the wire so ordering is explicit across languages.
type FactTable struct {
	Schema     uint64       `cbor:"schema" json:"schema"`
	Generation uint64       `cbor:"generation" json:"generation"`
	ProjectID  string       `cbor:"projectId" json:"projectId"`
	Sources    []SourceFile `cbor:"sources" json:"sources"`
	Entities   []EntityFact `cbor:"entities" json:"entities"`
	Symbols    []SymbolFact `cbor:"symbols" json:"symbols"`
	Files      []FileFact   `cbor:"files" json:"files"`
	transport  *factTableTransportChanges
	symbols    *symbolFactStore
}

type factTableTransportChanges struct {
	baseGeneration uint64
	sourcePaths    map[string]struct{}
	entityPaths    map[string]struct{}
	filePaths      map[string]struct{}
	symbolIDs      map[SymbolID]struct{}
	exact          bool
}
