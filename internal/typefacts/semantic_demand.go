package typefacts

import (
	"context"
	"path/filepath"
	"sort"
	"time"
)

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
func (p *DemandClosure) DemandTableForGroups(
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
		if p.table != nil {
			p.previousTable = p.table
			p.table = nil
		}
		if p.transportChangedPaths == nil {
			p.transportChangedPaths = make(map[string]struct{}, len(changedPaths))
		}
		for _, path := range changedPaths {
			p.transportChangedPaths[filepath.Clean(path)] = struct{}{}
		}
	}
	if p.table == nil {
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
		if err := p.materializeDemandTableLocked(ctx, generation, retainedGroups); err != nil {
			return nil, err
		}
	}
	return p.table, nil
}

func (p *DemandClosure) materializeDemandTableLocked(
	ctx context.Context,
	generation uint64,
	groups []demandGroup,
) error {
	started := time.Now()
	table, closed, full, stages, retention, err := p.materializeSemanticDemandRetained(ctx, groups, generation)
	if err != nil {
		return err
	}
	p.table = table
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
	if p.trace != nil {
		// These walks are why Trace is nilable: counting descriptors, calls and
		// references is a full pass over the table, and it must not run when
		// nobody is listening.
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
		p.trace.Stage("analyze-materialize", p.stats.BuildDuration)
		p.trace.Stage("analyze-async", stages.async)
		p.trace.Stage("analyze-demand", stages.demand)
		p.trace.Stage("analyze-symbols", stages.symbol)
		p.trace.Metrics("counts",
			Count("entities", len(table.Entities)), Count("symbols", table.symbolFactsCount()),
			Count("descriptors", descriptors), Count("calls", calls), Count("references", references),
			Count("cachedSymbolFacts", retention.CachedSymbolFacts),
			Count("recomputedSymbolFacts", retention.RecomputedSymbolFacts),
			Count("cachedReferenceFacts", retention.CachedReferenceFacts),
			Count("recomputedReferences", retention.RecomputedReferences),
			Count("patchedSymbolRows", retention.PatchedSymbolRows),
			Count("sharedSymbolChunks", retention.SharedSymbolChunks))
		p.trace.Metrics("retention",
			Count("retained", retention.RetainedFiles), Count("recomputed", retention.RecomputedFiles),
			Flag("suppressionRecompute", retention.SuppressionRecompute))
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
	// Demand-shaped async lookup only. The per-file whole-table walk it
	// replaced served the retired seed-based materializer.
	facts, err := backend.AsyncFunctionsAt(ctx, locations)
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

type semanticDemandStages struct {
	async    time.Duration
	demand   time.Duration
	assembly time.Duration
	sort     time.Duration
	close    time.Duration
	prepare  time.Duration
	symbol   time.Duration
}
