package typefacts

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"time"
)

// DemandResponseFor serves the experimental production-shaped path. Rust
// supplies structural demand; TS-Go contributes only checker semantics and
// async-flow metadata.
func (p *ClosureProject) DemandResponseFor(
	ctx context.Context,
	projectID string,
	generation uint64,
	demands []EntityDemand,
) (ClosureResponse, error) {
	table, err := p.DemandTableFor(ctx, generation, demands)
	if err != nil {
		return ClosureResponse{}, err
	}
	return ClosureResponse{
		Schema:     TypeFactsSchemaVersionV2,
		ProjectID:  projectID,
		Generation: generation,
		Table:      FactTableV2From(*table, projectID, generation),
	}, nil
}

// DemandTableFor materializes the canonical transport-only table without
// eagerly converting every row to its v2 wire counterpart. Stateful v3
// callers retain this immutable snapshot and convert only changed rows.
func (p *ClosureProject) DemandTableFor(
	ctx context.Context,
	generation uint64,
	demands []EntityDemand,
) (*FactTable, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if generation != p.generation {
		return nil, ErrGenerationMismatch
	}
	demands = canonicalDemands(demands)
	// Update already invalidated the table and recorded every source path
	// whose source-derived demands may differ. Do not linearly compare the
	// complete retained demand set merely to rediscover that fact.
	if p.demandTable == nil && p.previousDemandTable != nil {
		p.demands = demands
	} else if !reflect.DeepEqual(p.demands, demands) {
		if p.demandTable != nil {
			p.previousDemandTable = p.demandTable
		}
		if p.transportChangedPaths == nil {
			p.transportChangedPaths = make(map[string]struct{})
		}
		for path := range changedDemandPaths(p.demands, demands) {
			p.transportChangedPaths[path] = struct{}{}
		}
		p.demands = demands
		p.demandTable = nil
	}
	if p.demandTable == nil {
		if err := p.materializeDemandTableLocked(ctx, generation, demands, nil); err != nil {
			return nil, err
		}
	}
	return p.demandTable, nil
}

// DemandGroup is one canonical per-file demand run. Stateful protocol
// adapters retain these runs and can pass them without flattening and
// re-sorting the complete demand universe on every edit.
type DemandGroup struct {
	Path    string
	Demands []EntityDemand
}

// DemandTableForGroups is the retained-v3 interface. changedPaths must name
// every run replaced or removed since the preceding successful call. Update
// contributes source-affected paths independently, so callers only add
// demand-set changes here.
func (p *ClosureProject) DemandTableForGroups(
	ctx context.Context,
	generation uint64,
	groups []DemandGroup,
	changedPaths []string,
) (*FactTable, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if generation != p.generation {
		return nil, ErrGenerationMismatch
	}
	if len(changedPaths) != 0 {
		if p.demandTable != nil {
			p.previousDemandTable = p.demandTable
			p.demandTable = nil
		}
		if p.transportChangedPaths == nil {
			p.transportChangedPaths = make(map[string]struct{}, len(changedPaths))
		}
		for _, path := range changedPaths {
			p.transportChangedPaths[filepath.Clean(path)] = struct{}{}
		}
	}
	if p.demandTable == nil {
		retainedGroups := make([]demandGroup, 0, len(groups))
		for _, group := range groups {
			retainedGroups = append(retainedGroups, demandGroup{
				path:    filepath.Clean(group.Path),
				demands: group.Demands,
			})
		}
		sort.Slice(retainedGroups, func(i, j int) bool {
			return retainedGroups[i].path < retainedGroups[j].path
		})
		if err := p.materializeDemandTableLocked(ctx, generation, nil, retainedGroups); err != nil {
			return nil, err
		}
	}
	return p.demandTable, nil
}

func (p *ClosureProject) materializeDemandTableLocked(
	ctx context.Context,
	generation uint64,
	demands []EntityDemand,
	groups []demandGroup,
) error {
	started := time.Now()
	var table *FactTable
	var closed, full map[SymbolID]struct{}
	var stages semanticDemandStages
	var retention ClosureRetention
	var err error
	if scoped, ok := p.backend.(ScopedSemanticEntityDiscoverer); ok {
		if groups == nil {
			groups = groupDemands(demands)
		}
		table, closed, full, stages, retention, err = p.materializeSemanticDemandRetained(ctx, scoped, groups, generation)
	} else {
		if demands == nil {
			for _, group := range groups {
				demands = append(demands, group.demands...)
			}
		}
		table, closed, full, stages, err = materializeSemanticDemand(ctx, p.backend, demands, generation)
	}
	if err != nil {
		return err
	}
	p.demandTable = table
	p.closedSyms = closed
	p.fullTier = full
	p.stats = ClosureStats{
		BuildSequence:    p.stats.BuildSequence + 1,
		Generation:       generation,
		Files:            len(table.Files),
		Entities:         len(table.Entities),
		Symbols:          table.symbolFactsCount(),
		FullTierSymbols:  len(full),
		BuildDuration:    time.Since(started),
		AsyncDuration:    stages.async,
		DemandDuration:   stages.demand,
		SymbolDuration:   stages.symbol,
		AssemblyDuration: stages.assembly,
		SortDuration:     stages.sort,
		CloseDuration:    stages.close,
		PrepareDuration:  stages.prepare,
		Retention:        retention,
	}
	if os.Getenv("SOLID_TYPEFACTS_TIMINGS") != "" {
		descriptors, calls, references := 0, 0, 0
		for _, entity := range table.Entities {
			if entity.TypeDescriptor != nil {
				descriptors++
			}
			if entity.ResolvedCall != nil {
				calls++
			}
		}
		table.rangeSymbolFacts(func(symbol SymbolFact) {
			references += len(symbol.References)
		})
		fmt.Fprintf(os.Stderr, "{\"typefactsCounts\":{\"entities\":%d,\"symbols\":%d,\"descriptors\":%d,\"calls\":%d,\"references\":%d,\"cachedSymbolFacts\":%d,\"recomputedSymbolFacts\":%d,\"cachedReferenceFacts\":%d,\"recomputedReferences\":%d,\"patchedSymbolRows\":%d,\"sharedSymbolChunks\":%d}}\n",
			len(table.Entities), table.symbolFactsCount(), descriptors, calls, references,
			retention.CachedSymbolFacts, retention.RecomputedSymbolFacts,
			retention.CachedReferenceFacts, retention.RecomputedReferences,
			retention.PatchedSymbolRows, retention.SharedSymbolChunks)
	}
	return nil
}

func changedDemandPaths(previous, next []EntityDemand) map[string]struct{} {
	group := func(demands []EntityDemand) map[string][]EntityDemand {
		result := make(map[string][]EntityDemand)
		for _, demand := range demands {
			path := filepath.Clean(demand.Location.Path)
			result[path] = append(result[path], demand)
		}
		return result
	}
	oldGroups, newGroups := group(previous), group(next)
	changed := make(map[string]struct{})
	for path, demands := range newGroups {
		if !reflect.DeepEqual(oldGroups[path], demands) {
			changed[path] = struct{}{}
		}
	}
	for path := range oldGroups {
		if _, ok := newGroups[path]; !ok {
			changed[path] = struct{}{}
		}
	}
	return changed
}

func canonicalDemands(input []EntityDemand) []EntityDemand {
	result := append([]EntityDemand(nil), input...)
	sort.Slice(result, func(i, j int) bool {
		left, right := result[i], result[j]
		if left.Location.Path != right.Location.Path {
			return left.Location.Path < right.Location.Path
		}
		if left.Location.StartByte != right.Location.StartByte {
			return left.Location.StartByte < right.Location.StartByte
		}
		if left.Location.EndByte != right.Location.EndByte {
			return left.Location.EndByte < right.Location.EndByte
		}
		if left.QueryLocation == nil {
			return right.QueryLocation != nil
		}
		if right.QueryLocation == nil {
			return false
		}
		if left.QueryLocation.Path != right.QueryLocation.Path {
			return left.QueryLocation.Path < right.QueryLocation.Path
		}
		if left.QueryLocation.StartByte != right.QueryLocation.StartByte {
			return left.QueryLocation.StartByte < right.QueryLocation.StartByte
		}
		return left.QueryLocation.EndByte < right.QueryLocation.EndByte
	})
	return result
}

func materializeSemanticDemand(
	ctx context.Context,
	backend ClosureBackend,
	demands []EntityDemand,
	generation uint64,
) (*FactTable, map[SymbolID]struct{}, map[SymbolID]struct{}, semanticDemandStages, error) {
	sources, err := backend.SourceFiles(ctx)
	if err != nil {
		return nil, nil, nil, semanticDemandStages{}, err
	}
	sort.Slice(sources, func(i, j int) bool { return sources[i].Path < sources[j].Path })
	builder := &closureBuilder{
		backend:                backend,
		entities:               make(map[Location]*EntityFact),
		symbolSeen:             make(map[SymbolID]struct{}),
		fullTier:               make(map[SymbolID]struct{}),
		descriptors:            make(map[SymbolID]*TypeDescriptor),
		cleanPaths:             make(map[string]string),
		referencesOnlyFullTier: true,
	}
	table := &FactTable{
		Schema:     TypeFactsSchemaVersion,
		Generation: generation,
		ProjectID:  "semantic-demand",
		Sources:    sources,
	}
	stages := semanticDemandStages{}
	started := time.Now()
	asyncByPath, err := asyncFunctionsForDemands(ctx, backend, demands)
	if err != nil {
		return nil, nil, nil, stages, err
	}
	for _, source := range sources {
		if err := ctx.Err(); err != nil {
			return nil, nil, nil, stages, err
		}
		path := filepath.Clean(source.Path)
		asyncFunctions := asyncByPath[path]
		for _, function := range asyncFunctions {
			builder.enqueueSymbol(function.Symbol)
			builder.enqueueSymbol(function.Target)
		}
		table.Files = append(table.Files, FileFact{
			Path:           path,
			AsyncFunctions: asyncFunctions,
		})
	}
	stages.async = time.Since(started)
	started = time.Now()
	if discoverer, ok := backend.(SemanticEntityDiscoverer); ok {
		entities, err := discoverer.SemanticEntities(ctx, demands)
		if err != nil {
			return nil, nil, nil, stages, err
		}
		for index, entity := range entities {
			target := builder.entity(entity.Location)
			if entity.Symbol != "" {
				target.Symbol = entity.Symbol
				builder.enqueueSymbol(entity.Symbol)
				if index < len(demands) && demands[index].References {
					builder.fullTier[entity.Symbol] = struct{}{}
				}
			}
			if entity.TypeDescriptor != nil {
				target.TypeDescriptor = entity.TypeDescriptor
			}
			if entity.ResolvedCall != nil {
				target.ResolvedCall = entity.ResolvedCall
				builder.enqueueSymbol(entity.ResolvedCall.Target)
			}
		}
	} else {
		if err := materializeEntityDemands(ctx, builder, demands); err != nil {
			return nil, nil, nil, stages, err
		}
	}
	stages.demand = time.Since(started)
	started = time.Now()
	symbols, err := builder.closeSymbols(ctx)
	if err != nil {
		return nil, nil, nil, stages, err
	}
	stages.close = time.Since(started)
	table.Symbols = symbols
	started = time.Now()
	table.Entities = builder.sortedEntities()
	stages.sort = time.Since(started)
	// DemandResponseFor retains this table only as canonical ordered slices
	// for tableV2 transport conversion. It is never exposed through the Facts
	// lookup surface, so process-local runtime indexes would be dead work.
	stages.symbol = stages.close + stages.sort
	return table, builder.symbolSeen, builder.fullTier, stages, nil
}

func materializeEntityDemands(ctx context.Context, builder *closureBuilder, demands []EntityDemand) error {
	for _, demand := range demands {
		if err := ctx.Err(); err != nil {
			return err
		}
		location := demand.Location
		location.Path = filepath.Clean(location.Path)
		query := location
		if demand.QueryLocation != nil {
			query = *demand.QueryLocation
			query.Path = filepath.Clean(query.Path)
		}
		if demand.Symbol {
			if err := builder.addTieredSymbolEntity(ctx, location, demand.References); err != nil {
				return err
			}
		}
		if demand.TypeDescriptor {
			descriptor, err := builder.backend.DescribeTypeAt(ctx, query)
			switch {
			case err == nil:
				builder.entity(location).TypeDescriptor = &descriptor
			case !errors.Is(err, ErrNotFound):
				return err
			}
		}
		if demand.ResolvedCall {
			call, err := builder.backend.ResolvedCall(ctx, query)
			switch {
			case err == nil:
				builder.entity(location).ResolvedCall = &call
				builder.enqueueSymbol(call.Target)
			case !errors.Is(err, ErrNotFound):
				return err
			}
		}
	}
	return nil
}

func asyncFunctionsForDemands(
	ctx context.Context,
	backend ClosureBackend,
	demands []EntityDemand,
) (map[string][]AsyncFunctionFact, error) {
	locations := make([]Location, 0)
	demandedPaths := make(map[string]struct{})
	for _, demand := range demands {
		if !demand.Async {
			continue
		}
		location := demand.Location
		location.Path = filepath.Clean(location.Path)
		locations = append(locations, location)
		demandedPaths[location.Path] = struct{}{}
	}
	byPath := make(map[string][]AsyncFunctionFact, len(demandedPaths))
	if lookup, ok := backend.(AsyncFunctionLookup); ok {
		facts, err := lookup.AsyncFunctionsAt(ctx, locations)
		if err != nil {
			return nil, err
		}
		for _, fact := range facts {
			path := filepath.Clean(fact.Expression.Path)
			fact.Expression.Path = path
			byPath[path] = append(byPath[path], fact)
		}
		return byPath, nil
	}
	paths := make([]string, 0, len(demandedPaths))
	for path := range demandedPaths {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	for _, path := range paths {
		facts, err := backend.SourceAsyncFunctions(ctx, path)
		if err != nil {
			return nil, err
		}
		byPath[path] = facts
	}
	return byPath, nil
}

type semanticDemandStages struct {
	async    time.Duration
	demand   time.Duration
	assembly time.Duration
	sort     time.Duration
	close    time.Duration
	prepare  time.Duration
	symbol   time.Duration
}
