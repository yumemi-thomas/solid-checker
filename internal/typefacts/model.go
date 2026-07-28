package typefacts

import "crypto/sha256"

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

// SemanticDemandRun is one source file's canonically ordered demands.
// Path is the ownership key for every demand and query location in the run.
type SemanticDemandRun struct {
	Path    string
	Demands []EntityDemand
}

// SemanticScope carries the batch-wide state that makes a recomputed subset
// equivalent to resolving the complete demand closure.
type SemanticScope struct {
	Suppression    map[SymbolID]struct{}
	DescriptorSeed map[SymbolID]*TypeDescriptor
}

// SemanticDemandRunResult is one input run's aligned semantic answer.
// Dependencies excludes the run's own path and Durable covers every symbol
// identity embedded anywhere in the result.
type SemanticDemandRunResult struct {
	Entities     []EntityFact
	Structural   []SymbolID
	Dependencies []string
	Durable      bool
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
	// stateID identifies one private retained table state. Multiple demand
	// snapshots can materialize within the same source generation, so the
	// generation alone cannot authenticate an exact transport manifest.
	stateID   uint64
	transport *factTableTransportChanges
	symbols   *symbolFactStore
	// sourceDigests is the retained source representation. Source bodies are
	// needed only by LifecycleSources; retaining them in every semantic table
	// duplicated the compiler's immutable text solely to hash it for Rust.
	// Hand-built tables may still populate Sources; the wire layer derives
	// this compact form lazily for compatibility.
	sourceDigests []SourceDigest
	// pathSymbols is the compact transport/invalidation view retained by v6
	// after Rust has taken ownership of the expanded path rows.
	pathSymbols map[string][]SymbolID
	// entityRuns borrows the canonical per-file contribution rows until the
	// v6 transition has streamed them into Rust. It avoids copying those rows
	// into the generation-wide public Entities slice solely for transport.
	entityRuns []factTableEntityRun
}

type factTableEntityRun struct {
	entities []EntityFact
}

func (t *FactTable) wireEntityCount() int {
	if t == nil {
		return 0
	}
	if t.entityRuns == nil {
		return len(t.Entities)
	}
	count := 0
	for index := range t.entityRuns {
		count += len(t.entityRuns[index].entities)
	}
	return count
}

func (t *FactTable) rangeWireEntities(visit func(EntityFact)) {
	if t.entityRuns == nil {
		for index := range t.Entities {
			visit(t.Entities[index])
		}
		return
	}
	for runIndex := range t.entityRuns {
		for entityIndex := range t.entityRuns[runIndex].entities {
			visit(t.entityRuns[runIndex].entities[entityIndex])
		}
	}
}

// SourceDigest is the source identity transferred to Rust's retained table.
// It is deliberately not part of the public FactTable schema.
type SourceDigest struct {
	Path   string
	SHA256 [sha256.Size]byte
}

func (t *FactTable) wireSourceDigests() []SourceDigest {
	if t == nil {
		return nil
	}
	if t.sourceDigests != nil || len(t.Sources) == 0 {
		return t.sourceDigests
	}
	t.sourceDigests = make([]SourceDigest, len(t.Sources))
	for index := range t.Sources {
		t.sourceDigests[index] = SourceDigest{
			Path:   t.Sources[index].Path,
			SHA256: sha256.Sum256(t.Sources[index].Source),
		}
	}
	return t.sourceDigests
}

type factTableTransportChanges struct {
	baseGeneration uint64
	baseStateID    uint64
	sourcePaths    map[string]struct{}
	entityPaths    map[string]struct{}
	filePaths      map[string]struct{}
	symbolIDs      map[SymbolID]struct{}
	exact          bool
}
