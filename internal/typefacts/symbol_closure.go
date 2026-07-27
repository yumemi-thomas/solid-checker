package typefacts

import (
	"context"
	"errors"
	"path/filepath"
	"runtime"
	"sort"
	"sync"
	"time"
)

// Symbol closure: draining the worklist of symbols a demand set reaches to a
// fixed point, then ordering the result canonically. Alias and declaration
// facts come from the backend; reference lists come from the batch capability
// and, where the backend reports an exact delta, are patched into the previous
// generation's immutable chunks rather than rebuilt.

// closureBuilder accumulates one generation's symbol closure: the queue of
// symbols reached from the demanded entities, the facts resolved for them, and
// the retained state carried over from the preceding generation.
type closureBuilder struct {
	backend     ClosureBackend
	trace       Trace
	cleanPaths  map[string]string
	entities    map[Location]*EntityFact
	symbolQueue []SymbolID
	symbolSeen  map[SymbolID]struct{}
	// fullTier marks the symbols whose reference lists this generation must
	// carry: those reached by a demand that asked for references. Symbols
	// reached only to be classified stay out and get no list. Full tier
	// propagates along alias-target edges, because a caller canonicalizes
	// before fanning out on references.
	fullTier                map[SymbolID]struct{}
	descriptors             map[SymbolID]*TypeDescriptor
	cachedSymbolFacts       map[SymbolID]SymbolFact
	cachedReferences        map[SymbolID][]Location
	cachedSymbolOrder       []SymbolID
	cachedCanonicalStore    *symbolFactStore
	symbolFactsBuffer       []SymbolFact
	symbolOrderBuffer       []SymbolFact
	closedSymbolStore       *symbolFactStore
	closedReferences        map[SymbolID][]Location
	cachedSymbolHits        int
	recomputedSymbolFacts   int
	cachedReferenceHits     int
	recomputedReferences    int
	patchedSymbolRows       int
	sharedSymbolChunks      int
	changedSymbolIDs        map[SymbolID]struct{}
	removedSymbolCandidates map[SymbolID]struct{}
	referenceChangesExact   bool
}

func (b *closureBuilder) cleanPath(path string) string {
	if cleaned, ok := b.cleanPaths[path]; ok {
		return cleaned
	}
	cleaned := filepath.Clean(path)
	b.cleanPaths[path] = cleaned
	return cleaned
}

func (b *closureBuilder) entity(location Location) *EntityFact {
	location.Path = b.cleanPath(location.Path)
	if existing, ok := b.entities[location]; ok {
		return existing
	}
	created := &EntityFact{Location: location}
	b.entities[location] = created
	return created
}

func (b *closureBuilder) enqueueSymbol(id SymbolID) {
	if id == "" {
		return
	}
	if _, ok := b.symbolSeen[id]; ok {
		return
	}
	b.symbolSeen[id] = struct{}{}
	b.symbolQueue = append(b.symbolQueue, id)
}

// closeSymbols drains the worklist to a fixed point: every reached symbol
// gets its alias target (enqueueing it), declarations, and reference list.
// Termination is by construction — a symbol enters the queue at most once and
// the generation's symbol universe is finite. Afterwards, full tier
// propagates along alias-target edges to its own fixed point.
func (b *closureBuilder) closeSymbols(ctx context.Context) ([]SymbolFact, error) {
	started := time.Now()
	initialSymbolCount := len(b.symbolQueue)
	facts := b.symbolFactsBuffer[:0]
	if cap(facts) < len(b.symbolQueue) {
		facts = make([]SymbolFact, len(b.symbolQueue))
	} else {
		facts = facts[:len(b.symbolQueue)]
	}
	cached := make([]bool, initialSymbolCount)
	workers := min(runtime.GOMAXPROCS(0), initialSymbolCount)
	if workers > 1 && initialSymbolCount >= 1024 {
		chunkSize := (initialSymbolCount + workers - 1) / workers
		var wait sync.WaitGroup
		for start := 0; start < initialSymbolCount; start += chunkSize {
			end := min(start+chunkSize, initialSymbolCount)
			wait.Add(1)
			go func() {
				defer wait.Done()
				for index := start; index < end; index++ {
					id := b.symbolQueue[index]
					if retained, ok := b.cachedSymbolFacts[id]; ok {
						facts[index] = SymbolFact{
							ID:           id,
							AliasTarget:  retained.AliasTarget,
							Declarations: retained.Declarations,
						}
						cached[index] = true
					} else {
						facts[index] = SymbolFact{ID: id}
					}
				}
			}()
		}
		wait.Wait()
	} else {
		for index := 0; index < initialSymbolCount; index++ {
			id := b.symbolQueue[index]
			if retained, ok := b.cachedSymbolFacts[id]; ok {
				facts[index] = SymbolFact{
					ID:           id,
					AliasTarget:  retained.AliasTarget,
					Declarations: retained.Declarations,
				}
				cached[index] = true
			} else {
				facts[index] = SymbolFact{ID: id}
			}
		}
	}
	for index := 0; index < initialSymbolCount; index++ {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		id := b.symbolQueue[index]
		fact := &facts[index]
		if cached[index] {
			b.enqueueSymbol(fact.AliasTarget)
			b.cachedSymbolHits++
			continue
		}
		b.recomputedSymbolFacts++
		if b.changedSymbolIDs == nil {
			b.changedSymbolIDs = make(map[SymbolID]struct{})
		}
		b.changedSymbolIDs[id] = struct{}{}
		target, err := b.backend.ResolveAlias(ctx, id)
		switch {
		case err == nil:
			fact.AliasTarget = target
			b.enqueueSymbol(target)
		case !errors.Is(err, ErrNotFound):
			return nil, err
		}
		declarations, err := b.backend.Declarations(ctx, id)
		switch {
		case err == nil:
			fact.Declarations = declarations
		case !errors.Is(err, ErrNotFound):
			return nil, err
		}
	}
	for index := initialSymbolCount; index < len(b.symbolQueue); index++ {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		id := b.symbolQueue[index]
		fact := SymbolFact{ID: id}
		if retained, ok := b.cachedSymbolFacts[id]; ok {
			fact.AliasTarget = retained.AliasTarget
			fact.Declarations = retained.Declarations
			b.enqueueSymbol(fact.AliasTarget)
			b.cachedSymbolHits++
			facts = append(facts, fact)
			continue
		}
		b.recomputedSymbolFacts++
		if b.changedSymbolIDs == nil {
			b.changedSymbolIDs = make(map[SymbolID]struct{})
		}
		b.changedSymbolIDs[id] = struct{}{}
		target, err := b.backend.ResolveAlias(ctx, id)
		switch {
		case err == nil:
			fact.AliasTarget = target
			b.enqueueSymbol(target)
		case !errors.Is(err, ErrNotFound):
			return nil, err
		}
		declarations, err := b.backend.Declarations(ctx, id)
		switch {
		case err == nil:
			fact.Declarations = declarations
		case !errors.Is(err, ErrNotFound):
			return nil, err
		}
		facts = append(facts, fact)
	}
	factsDuration := time.Since(started)
	started = time.Now()
	for changed := true; changed; {
		changed = false
		for _, fact := range facts {
			if fact.AliasTarget == "" {
				continue
			}
			if _, full := b.fullTier[fact.ID]; !full {
				continue
			}
			if _, full := b.fullTier[fact.AliasTarget]; !full {
				b.fullTier[fact.AliasTarget] = struct{}{}
				changed = true
			}
		}
	}
	fullTierDuration := time.Since(started)
	started = time.Now()
	if patched, ok, err := b.patchCanonicalSymbolStore(ctx, facts); err != nil {
		return nil, err
	} else if ok {
		b.closedSymbolStore = patched
		referencesDuration := time.Since(started)
		if b.trace != nil {
			b.trace.Metrics("closeSymbols",
				Nanos("factsNs", factsDuration), Nanos("fullTierNs", fullTierDuration),
				Nanos("referencesNs", referencesDuration), Nanos("sortNs", 0),
				Count("initialSymbols", initialSymbolCount), Count("symbols", patched.Len()),
				Count("fullTier", len(b.fullTier)), Count("references", len(b.closedReferences)))
		}
		return nil, nil
	}
	factByID := make(map[SymbolID]*SymbolFact, len(facts))
	for index := range facts {
		fact := &facts[index]
		factByID[fact.ID] = fact
	}
	// Reference lists always come from the batch capability: one backend
	// lock instead of tens of thousands of round trips. There is no
	// per-symbol fallback, because no backend lacks it.
	ids := make([]SymbolID, 0, len(facts))
	for index := range facts {
		fact := &facts[index]
		if fact.AliasTarget != "" {
			continue
		}
		// Only full-tier symbols carry reference lists; a symbol reached
		// solely to be classified gets none.
		if _, demanded := b.fullTier[fact.ID]; !demanded {
			continue
		}
		ids = append(ids, fact.ID)
	}

	refresh := ids
	// A first build has nothing retained to compare against, so an exact
	// delta only means something once cachedReferences exists.
	if b.cachedReferences != nil {
		changedIDs, exact, err := b.backend.ChangedReferences(ctx)
		if err != nil {
			return nil, err
		}
		if exact {
			changedSet := make(map[SymbolID]struct{}, len(changedIDs))
			for _, id := range changedIDs {
				changedSet[id] = struct{}{}
				if b.changedSymbolIDs == nil {
					b.changedSymbolIDs = make(map[SymbolID]struct{})
				}
				b.changedSymbolIDs[id] = struct{}{}
			}
			b.referenceChangesExact = true
			referenceWorkers := min(runtime.GOMAXPROCS(0), len(ids))
			if referenceWorkers > 1 && len(ids) >= 1024 {
				type refreshChunk struct {
					ids  []SymbolID
					hits int
				}
				chunkSize := (len(ids) + referenceWorkers - 1) / referenceWorkers
				chunks := make([]refreshChunk, (len(ids)+chunkSize-1)/chunkSize)
				var wait sync.WaitGroup
				for chunkIndex, start := 0, 0; start < len(ids); chunkIndex, start = chunkIndex+1, start+chunkSize {
					end := min(start+chunkSize, len(ids))
					wait.Add(1)
					go func() {
						defer wait.Done()
						chunk := &chunks[chunkIndex]
						chunk.ids = make([]SymbolID, 0, len(changedSet))
						for _, id := range ids[start:end] {
							cached, cachedOK := b.cachedReferences[id]
							_, referenceChanged := changedSet[id]
							if !cachedOK || referenceChanged {
								chunk.ids = append(chunk.ids, id)
								continue
							}
							factByID[id].References = cached
							chunk.hits++
						}
					}()
				}
				wait.Wait()
				refresh = make([]SymbolID, 0, len(changedSet))
				for _, chunk := range chunks {
					refresh = append(refresh, chunk.ids...)
					b.cachedReferenceHits += chunk.hits
				}
			} else {
				refresh = make([]SymbolID, 0, len(changedSet))
				for _, id := range ids {
					cached, cachedOK := b.cachedReferences[id]
					_, referenceChanged := changedSet[id]
					if !cachedOK || referenceChanged {
						refresh = append(refresh, id)
						continue
					}
					factByID[id].References = cached
					b.cachedReferenceHits++
				}
			}
		}
	}

	references, err := b.backend.ReferencesBatch(ctx, refresh)
	if err != nil {
		return nil, err
	}
	b.recomputedReferences += len(refresh)
	for _, id := range refresh {
		if b.changedSymbolIDs == nil {
			b.changedSymbolIDs = make(map[SymbolID]struct{})
		}
		b.changedSymbolIDs[id] = struct{}{}
		// Absence is a known-empty list, not an unresolved cache miss.
		factByID[id].References = references[id]
	}
	if b.referenceChangesExact {
		// The exact delta makes the preceding map itself reusable.
		// Prune departed/non-reference symbols and overwrite only the
		// refreshed rows instead of copying every retained slice header.
		b.closedReferences = b.cachedReferences
		for id := range b.closedReferences {
			fact := factByID[id]
			if fact == nil || fact.AliasTarget != "" {
				delete(b.closedReferences, id)
				continue
			}
			if _, demanded := b.fullTier[id]; !demanded {
				delete(b.closedReferences, id)
			}
		}
		for _, id := range refresh {
			b.closedReferences[id] = factByID[id].References
		}
	} else {
		b.closedReferences = make(map[SymbolID][]Location, len(ids))
		for _, id := range ids {
			b.closedReferences[id] = factByID[id].References
		}
	}
	referencesDuration := time.Since(started)
	started = time.Now()
	var spare []SymbolFact
	facts, spare = orderSymbolFacts(facts, factByID, b.cachedSymbolOrder, b.symbolOrderBuffer[:0])
	b.symbolFactsBuffer = spare[:0]
	sortDuration := time.Since(started)
	if b.trace != nil {
		b.trace.Metrics("closeSymbols",
			Nanos("factsNs", factsDuration), Nanos("fullTierNs", fullTierDuration),
			Nanos("referencesNs", referencesDuration), Nanos("sortNs", sortDuration),
			Count("initialSymbols", initialSymbolCount), Count("symbols", len(facts)),
			Count("fullTier", len(b.fullTier)), Count("references", len(b.closedReferences)))
	}
	return facts, nil
}

// patchCanonicalSymbolStore is the retained-session fast path. It patches
// immutable chunks named by exact symbol/reference deltas and shares every
// untouched chunk with the preceding generation. An unexplained
// reference-tier change falls back to the general closure path.
func (b *closureBuilder) patchCanonicalSymbolStore(ctx context.Context, facts []SymbolFact) (*symbolFactStore, bool, error) {
	previous := b.cachedCanonicalStore
	if previous == nil || previous.Len() == 0 || b.cachedReferences == nil {
		return nil, false, nil
	}
	for index := range facts {
		fact := &facts[index]
		eligible := fact.AliasTarget == ""
		if eligible {
			_, eligible = b.fullTier[fact.ID]
		}
		_, previouslyEligible := b.cachedReferences[fact.ID]
		if eligible != previouslyEligible {
			if _, changed := b.changedSymbolIDs[fact.ID]; !changed {
				return nil, false, nil
			}
		}
	}

	changedReferences, exact, err := b.backend.ChangedReferences(ctx)
	if err != nil {
		return nil, false, err
	}
	if !exact {
		return nil, false, nil
	}
	b.referenceChangesExact = true
	for _, id := range changedReferences {
		if b.changedSymbolIDs == nil {
			b.changedSymbolIDs = make(map[SymbolID]struct{})
		}
		b.changedSymbolIDs[id] = struct{}{}
	}

	patches := make(map[SymbolID]SymbolFact, len(b.changedSymbolIDs))
	for index := range facts {
		fact := facts[index]
		if _, changed := b.changedSymbolIDs[fact.ID]; !changed {
			continue
		}
		if retained, present := previous.Get(fact.ID); present {
			fact.References = retained.References
		}
		patches[fact.ID] = fact
	}

	refreshSet := make(map[SymbolID]struct{}, len(changedReferences)+len(b.changedSymbolIDs))
	for id := range b.changedSymbolIDs {
		fact, present := patches[id]
		if !present {
			fact, present = previous.Get(id)
		}
		if !present {
			continue
		}
		eligible := fact.AliasTarget == ""
		if eligible {
			_, eligible = b.fullTier[id]
		}
		if !eligible {
			fact.References = nil
			patches[id] = fact
			delete(b.cachedReferences, id)
			continue
		}
		if _, cached := b.cachedReferences[id]; !cached {
			refreshSet[id] = struct{}{}
		}
	}
	for _, id := range changedReferences {
		if _, eligible := b.cachedReferences[id]; eligible {
			refreshSet[id] = struct{}{}
		}
	}
	refresh := make([]SymbolID, 0, len(refreshSet))
	for id := range refreshSet {
		refresh = append(refresh, id)
	}
	references, err := b.backend.ReferencesBatch(ctx, refresh)
	if err != nil {
		return nil, false, err
	}
	b.recomputedReferences += len(refresh)
	for _, id := range refresh {
		fact, present := patches[id]
		if !present {
			fact, present = previous.Get(id)
		}
		if !present {
			continue
		}
		fact.References = references[id]
		patches[id] = fact
		b.cachedReferences[id] = references[id]
	}

	next, shared, removed, complete := previous.Patch(b.symbolSeen, patches, b.removedSymbolCandidates)
	if !complete {
		return nil, false, nil
	}
	for _, id := range removed {
		delete(b.cachedReferences, id)
	}
	b.cachedReferenceHits += len(b.cachedReferences) - len(refresh)
	b.closedReferences = b.cachedReferences
	b.symbolFactsBuffer = facts[:0]
	b.patchedSymbolRows = len(patches)
	b.sharedSymbolChunks = shared
	return next, true, nil
}

// orderSymbolFacts preserves canonical ID ordering while avoiding a complete
// O(n log n) sort when the preceding generation already established the order.
// Surviving rows take their current values in the prior order; only new IDs
// need sorting before the two sorted runs are merged. Missing prior IDs simply
// drop out.
func orderSymbolFacts(
	facts []SymbolFact,
	factByID map[SymbolID]*SymbolFact,
	previous []SymbolID,
	output []SymbolFact,
) ([]SymbolFact, []SymbolFact) {
	if len(previous) == 0 {
		sort.Slice(facts, func(i, j int) bool { return facts[i].ID < facts[j].ID })
		return facts, output
	}
	if len(previous) == len(facts) {
		ordered := output
		if cap(ordered) < len(facts) {
			ordered = make([]SymbolFact, 0, len(facts))
		}
		for _, id := range previous {
			fact, ok := factByID[id]
			if !ok {
				ordered = ordered[:0]
				break
			}
			ordered = append(ordered, *fact)
		}
		if len(ordered) == len(facts) {
			return ordered, facts
		}
	}

	retained := make([]SymbolFact, 0, len(facts))
	for _, id := range previous {
		if fact, ok := factByID[id]; ok {
			retained = append(retained, *fact)
			delete(factByID, id)
		}
	}
	if len(factByID) == 0 {
		output = append(output[:0], retained...)
		return output, facts
	}

	added := make([]SymbolFact, 0, len(factByID))
	for _, fact := range factByID {
		added = append(added, *fact)
	}
	sort.Slice(added, func(i, j int) bool { return added[i].ID < added[j].ID })

	ordered := make([]SymbolFact, 0, len(facts))
	left, right := 0, 0
	for left < len(retained) && right < len(added) {
		if retained[left].ID < added[right].ID {
			ordered = append(ordered, retained[left])
			left++
		} else {
			ordered = append(ordered, added[right])
			right++
		}
	}
	ordered = append(ordered, retained[left:]...)
	ordered = append(ordered, added[right:]...)
	output = append(output[:0], ordered...)
	return output, facts
}
