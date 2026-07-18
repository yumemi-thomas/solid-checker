package typefacts

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"
)

// Facts is the immutable query surface consumed by reactiveir. A Facts value
// represents exactly one analysis generation and must not perform compiler
// work while serving lookups.
type Facts interface {
	SourceFiles(context.Context) ([]SourceFile, error)
	SymbolAt(context.Context, Location) (SymbolID, error)
	ResolveAlias(context.Context, SymbolID) (SymbolID, error)
	Declarations(context.Context, SymbolID) ([]Declaration, error)
	References(context.Context, SymbolID) ([]Location, error)
	TypeAt(context.Context, Location) (TypeID, error)
	ResolvedCall(context.Context, Location) (Call, error)
}

// MaterializedProject can enumerate and materialize the complete finite fact
// universe for its current generation.
type MaterializedProject interface {
	Project
	Materialize(context.Context, DemandProfile) (FactTable, error)
}

// DemandProfile is a generation-independent record of canonical entity and
// bulk-file keys consumed by reactiveir. RefreshPaths requests complete entity
// enumeration for changed files whose source offsets may have shifted.
type DemandProfile struct {
	Entities      []EntityDemand `cbor:"entities,omitempty" json:"entities,omitempty"`
	Files         []string       `cbor:"files,omitempty" json:"files,omitempty"`
	RefreshPaths  []string       `cbor:"refreshPaths,omitempty" json:"refreshPaths,omitempty"`
	RefreshRanges []Location     `cbor:"refreshRanges,omitempty" json:"refreshRanges,omitempty"`
}

// EntityDemand selects only the fields consumed for one canonical entity.
type EntityDemand struct {
	Location           Location  `cbor:"location" json:"location"`
	QueryLocation      *Location `cbor:"queryLocation,omitempty" json:"queryLocation,omitempty"`
	Symbol             bool      `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Type               bool      `cbor:"type,omitempty" json:"type,omitempty"`
	TypeDescriptor     bool      `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall       bool      `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
	ResolveAlias       bool      `cbor:"resolveAlias,omitempty" json:"resolveAlias,omitempty"`
	Declarations       bool      `cbor:"declarations,omitempty" json:"declarations,omitempty"`
	References         bool      `cbor:"references,omitempty" json:"references,omitempty"`
	Async              bool      `cbor:"async,omitempty" json:"async,omitempty"`
	StructuralAccessor bool      `cbor:"structuralAccessor,omitempty" json:"structuralAccessor,omitempty"`
}

// EntityFact is one legal location-keyed entity in the finite fact universe.
// Location ranges are ordered from outermost to innermost during encoding.
type EntityFact struct {
	Location       Location        `cbor:"location" json:"location"`
	Symbol         SymbolID        `cbor:"symbol,omitempty" json:"symbol,omitempty"`
	Type           TypeID          `cbor:"type,omitempty" json:"type,omitempty"`
	TypeDescriptor *TypeDescriptor `cbor:"typeDescriptor,omitempty" json:"typeDescriptor,omitempty"`
	ResolvedCall   *Call           `cbor:"resolvedCall,omitempty" json:"resolvedCall,omitempty"`
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
	runtime    *factTableRuntime
	transport  *factTableTransportChanges
}

type factTableTransportChanges struct {
	baseGeneration uint64
	sourcePaths    map[string]struct{}
	entityPaths    map[string]struct{}
	filePaths      map[string]struct{}
	symbolIDs      map[SymbolID]struct{}
	exact          bool
}

type factTableRuntime struct {
	entities         map[string][]EntityFact
	entitiesAt       map[entityPoint][]EntityFact
	symbols          map[SymbolID]SymbolFact
	files            map[string]FileFact
	symbolOrigins    map[SymbolID][]Location
	accessedEntities map[Location]EntityDemand
	accessedFiles    map[string]struct{}
}

type entityPoint struct {
	path  string
	start int
}

// Prepare builds process-local lookup indexes after constructing or decoding a
// table. The indexes are excluded from the wire representation.
func (t *FactTable) Prepare() {
	runtime := &factTableRuntime{
		entities:         make(map[string][]EntityFact),
		entitiesAt:       make(map[entityPoint][]EntityFact),
		symbols:          make(map[SymbolID]SymbolFact, len(t.Symbols)),
		files:            make(map[string]FileFact, len(t.Files)),
		symbolOrigins:    make(map[SymbolID][]Location),
		accessedEntities: make(map[Location]EntityDemand),
		accessedFiles:    make(map[string]struct{}),
	}
	for _, entity := range t.Entities {
		path := filepath.Clean(entity.Location.Path)
		runtime.entities[path] = append(runtime.entities[path], entity)
		point := entityPoint{path: path, start: entity.Location.StartByte}
		runtime.entitiesAt[point] = append(runtime.entitiesAt[point], entity)
		if entity.Symbol != "" && len(runtime.symbolOrigins[entity.Symbol]) == 0 {
			runtime.symbolOrigins[entity.Symbol] = []Location{entity.Location}
		}
		if entity.ResolvedCall != nil && entity.ResolvedCall.Target != "" && len(runtime.symbolOrigins[entity.ResolvedCall.Target]) == 0 {
			runtime.symbolOrigins[entity.ResolvedCall.Target] = []Location{entity.Location}
		}
	}
	for _, fact := range t.Symbols {
		runtime.symbols[fact.ID] = fact
	}
	for range len(t.Symbols) + 1 {
		changed := false
		for _, fact := range t.Symbols {
			if fact.AliasTarget == "" || len(runtime.symbolOrigins[fact.ID]) == 0 {
				continue
			}
			if len(runtime.symbolOrigins[fact.AliasTarget]) == 0 {
				runtime.symbolOrigins[fact.AliasTarget] = append([]Location(nil), runtime.symbolOrigins[fact.ID]...)
				changed = true
			}
		}
		if !changed {
			break
		}
	}
	for _, fact := range t.Files {
		runtime.files[filepath.Clean(fact.Path)] = fact
		for _, call := range fact.Calls {
			if call.Target != "" && len(runtime.symbolOrigins[call.Target]) == 0 {
				runtime.symbolOrigins[call.Target] = []Location{call.Callee}
			}
		}
		for _, async := range fact.AsyncFunctions {
			if async.Symbol != "" && len(runtime.symbolOrigins[async.Symbol]) == 0 {
				runtime.symbolOrigins[async.Symbol] = []Location{async.Expression}
			}
			if async.Target != "" && len(runtime.symbolOrigins[async.Target]) == 0 {
				runtime.symbolOrigins[async.Target] = []Location{async.Expression}
			}
		}
	}
	t.runtime = runtime
}

// ReleaseWireStorage drops deterministic transport slices after they have
// been encoded. Prepared runtime indexes own the facts needed by immutable
// lookups, so retaining both representations would double the live
// generation's memory footprint.
func (t *FactTable) ReleaseWireStorage() {
	if t.runtime == nil {
		return
	}
	t.Entities = nil
	t.Symbols = nil
	t.Files = nil
}

func (t FactTable) SourceFiles(ctx context.Context) ([]SourceFile, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	return cloneSources(t.Sources), nil
}

func (t FactTable) SymbolAt(ctx context.Context, location Location) (SymbolID, error) {
	entity, err := t.entityAt(ctx, location, "symbol", func(entity EntityFact) bool { return entity.Symbol != "" })
	if err != nil {
		return "", err
	}
	return entity.Symbol, nil
}

func (t FactTable) TypeAt(ctx context.Context, location Location) (TypeID, error) {
	entity, err := t.entityAt(ctx, location, "type", func(entity EntityFact) bool { return entity.Type != "" })
	if err != nil {
		return "", err
	}
	return entity.Type, nil
}

func (t FactTable) DescribeTypeAt(ctx context.Context, location Location) (TypeDescriptor, error) {
	entity, err := t.entityAt(ctx, location, "type-descriptor", func(entity EntityFact) bool { return entity.TypeDescriptor != nil })
	if err != nil {
		return TypeDescriptor{}, err
	}
	return *entity.TypeDescriptor, nil
}

func (t FactTable) ResolvedCall(ctx context.Context, location Location) (Call, error) {
	entity, err := t.entityAt(ctx, location, "resolved-call", func(entity EntityFact) bool { return entity.ResolvedCall != nil })
	if err != nil {
		return Call{}, err
	}
	return *entity.ResolvedCall, nil
}

func (t FactTable) entityAt(ctx context.Context, location Location, field string, usable func(EntityFact) bool) (EntityFact, error) {
	if err := ctx.Err(); err != nil {
		return EntityFact{}, err
	}
	path := filepath.Clean(location.Path)
	entities := t.Entities
	pointLookup := false
	if t.runtime != nil {
		entities = t.runtime.entitiesAt[entityPoint{path: path, start: location.StartByte}]
		if len(entities) == 0 {
			entities = t.runtime.entities[path]
		} else {
			pointLookup = true
		}
	}
	findBest := func(candidates []EntityFact) int {
		best := -1
		for index, entity := range candidates {
			if !usable(entity) {
				continue
			}
			if entity.Location.StartByte > location.StartByte || entity.Location.EndByte <= location.StartByte {
				continue
			}
			if best < 0 || entity.Location.EndByte-entity.Location.StartByte < candidates[best].Location.EndByte-candidates[best].Location.StartByte {
				best = index
			}
		}
		return best
	}
	best := findBest(entities)
	if best < 0 && pointLookup {
		entities = t.runtime.entities[path]
		best = findBest(entities)
	}
	if best < 0 {
		return EntityFact{}, fmt.Errorf("%w: entity at %s:%d (%d materialized entities for file)", ErrNotFound, path, location.StartByte, len(entities))
	}
	if t.runtime != nil {
		selected := entities[best].Location
		demand := t.runtime.accessedEntities[selected]
		demand.Location = selected
		switch field {
		case "symbol":
			demand.Symbol = true
		case "type":
			demand.Type = true
		case "type-descriptor":
			demand.TypeDescriptor = true
		case "resolved-call":
			demand.ResolvedCall = true
		}
		t.runtime.accessedEntities[selected] = demand
	}
	return entities[best], nil
}

func (t FactTable) ResolveAlias(ctx context.Context, id SymbolID) (SymbolID, error) {
	t.recordSymbolDemand(id, "resolve-alias")
	fact, err := t.symbol(ctx, id)
	if err != nil {
		return "", err
	}
	if fact.AliasTarget == "" {
		return "", fmt.Errorf("%w: symbol %s is not an alias", ErrNotFound, id)
	}
	return fact.AliasTarget, nil
}

func (t FactTable) Declarations(ctx context.Context, id SymbolID) ([]Declaration, error) {
	t.recordSymbolDemand(id, "declarations")
	fact, err := t.symbol(ctx, id)
	if err != nil {
		return nil, err
	}
	if len(fact.Declarations) == 0 {
		return nil, fmt.Errorf("%w: declarations for symbol %s", ErrNotFound, id)
	}
	return append([]Declaration(nil), fact.Declarations...), nil
}

func (t FactTable) References(ctx context.Context, id SymbolID) ([]Location, error) {
	t.recordSymbolDemand(id, "references")
	fact, err := t.symbol(ctx, id)
	if err != nil {
		return nil, err
	}
	return append([]Location(nil), fact.References...), nil
}

func (t FactTable) recordSymbolDemand(id SymbolID, field string) {
	if t.runtime == nil || id == "" {
		return
	}
	locations := t.runtime.symbolOrigins[id]
	if fact, ok := t.runtime.symbols[id]; ok && len(fact.Declarations) != 0 {
		locations = []Location{fact.Declarations[0].Location}
	}
	for _, location := range locations {
		demand := t.runtime.accessedEntities[location]
		demand.Location = location
		demand.Symbol = true
		switch field {
		case "resolve-alias":
			demand.ResolveAlias = true
		case "declarations":
			demand.Declarations = true
		case "references":
			demand.References = true
		}
		t.runtime.accessedEntities[location] = demand
	}
}

func (t FactTable) symbol(ctx context.Context, id SymbolID) (SymbolFact, error) {
	if err := ctx.Err(); err != nil {
		return SymbolFact{}, err
	}
	if t.runtime != nil {
		fact, ok := t.runtime.symbols[id]
		if !ok {
			return SymbolFact{}, fmt.Errorf("%w: symbol %s", ErrNotFound, id)
		}
		return fact, nil
	}
	index := sort.Search(len(t.Symbols), func(index int) bool { return t.Symbols[index].ID >= id })
	if index == len(t.Symbols) || t.Symbols[index].ID != id {
		return SymbolFact{}, fmt.Errorf("%w: symbol %s", ErrNotFound, id)
	}
	return t.Symbols[index], nil
}

func (t FactTable) SourceCalls(ctx context.Context, path string) ([]SourceCall, error) {
	file, err := t.file(ctx, path)
	return append([]SourceCall(nil), file.Calls...), err
}

func (t FactTable) SourceBindings(ctx context.Context, path string) ([]SourceBinding, error) {
	file, err := t.file(ctx, path)
	return append([]SourceBinding(nil), file.Bindings...), err
}

func (t FactTable) SourceFunctions(ctx context.Context, path string) ([]SourceFunction, error) {
	file, err := t.file(ctx, path)
	return append([]SourceFunction(nil), file.Functions...), err
}

func (t FactTable) SourceAsyncFunctions(ctx context.Context, path string) ([]AsyncFunctionFact, error) {
	file, err := t.file(ctx, path)
	return append([]AsyncFunctionFact(nil), file.AsyncFunctions...), err
}

func (t FactTable) file(ctx context.Context, path string) (FileFact, error) {
	if err := ctx.Err(); err != nil {
		return FileFact{}, err
	}
	path = filepath.Clean(path)
	if t.runtime != nil {
		fact, ok := t.runtime.files[path]
		if !ok {
			return FileFact{}, fmt.Errorf("%w: source file %s", ErrNotFound, path)
		}
		t.runtime.accessedFiles[path] = struct{}{}
		return fact, nil
	}
	index := sort.Search(len(t.Files), func(index int) bool { return t.Files[index].Path >= path })
	if index == len(t.Files) || t.Files[index].Path != path {
		return FileFact{}, fmt.Errorf("%w: source file %s", ErrNotFound, path)
	}
	return t.Files[index], nil
}

// DemandProfile returns the canonical keys observed since Prepare.
func (t FactTable) DemandProfile() DemandProfile {
	if t.runtime == nil {
		return DemandProfile{}
	}
	profile := DemandProfile{
		Entities: make([]EntityDemand, 0, len(t.runtime.accessedEntities)),
		Files:    make([]string, 0, len(t.runtime.accessedFiles)),
	}
	for _, demand := range t.runtime.accessedEntities {
		profile.Entities = append(profile.Entities, demand)
	}
	for path := range t.runtime.accessedFiles {
		profile.Files = append(profile.Files, path)
	}
	sort.Slice(profile.Entities, func(i, j int) bool {
		left, right := profile.Entities[i].Location, profile.Entities[j].Location
		if left.Path != right.Path {
			return left.Path < right.Path
		}
		if left.StartByte != right.StartByte {
			return left.StartByte < right.StartByte
		}
		return left.EndByte < right.EndByte
	})
	sort.Strings(profile.Files)
	return profile
}

func cloneSources(sources []SourceFile) []SourceFile {
	result := make([]SourceFile, len(sources))
	for index, source := range sources {
		// FactTable owns the source byte slices for the generation. Facts is an
		// immutable interface, so callers only need an independent descriptor
		// slice; copying every file body again on each SourceFiles lookup adds
		// work without adding isolation.
		result[index] = SourceFile{Path: source.Path, Source: source.Source}
	}
	return result
}
