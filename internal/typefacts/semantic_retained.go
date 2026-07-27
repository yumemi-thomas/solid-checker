package typefacts

import (
	"context"
	"encoding/binary"
	"hash/maphash"
	"maps"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// demandGroup is one file's slice of the canonical demand list.
type demandGroup struct {
	path         string // clean path, the retention key
	demands      []EntityDemand
	hash         uint64
	contribution *fileClosureContribution
}

// canonicalDemandRun returns the run sorted by location, allocating only when
// the input is not already in that order — which is the common case, since both
// the Rust client and the protocol adapter build runs from sorted input.
func canonicalDemandRun(demands []EntityDemand) []EntityDemand {
	sorted := true
	for index := 1; index < len(demands); index++ {
		previous, current := demands[index-1].Location, demands[index].Location
		if previous.StartByte > current.StartByte ||
			(previous.StartByte == current.StartByte && previous.EndByte > current.EndByte) {
			sorted = false
			break
		}
	}
	if sorted {
		return demands
	}
	canonical := make([]EntityDemand, len(demands))
	copy(canonical, demands)
	sort.SliceStable(canonical, func(i, j int) bool {
		left, right := canonical[i].Location, canonical[j].Location
		if left.StartByte != right.StartByte {
			return left.StartByte < right.StartByte
		}
		return left.EndByte < right.EndByte
	})
	return canonical
}

// demandListHash digests one file's demand run. The hash only ever compares
// runs within one process (retained state never crosses processes), so a
// process-seeded maphash is sufficient and fast. Paths are excluded: every
// demand in a run belongs to the group's file, and cross-file query
// locations do not occur.
func demandListHash(demands []EntityDemand, seed maphash.Seed) uint64 {
	var digest maphash.Hash
	digest.SetSeed(seed)
	buffer := make([]byte, 0, 64)
	flag := func(value bool) byte {
		if value {
			return 1
		}
		return 0
	}
	for index := range demands {
		demand := &demands[index]
		buffer = buffer[:0]
		buffer = binary.LittleEndian.AppendUint64(buffer, uint64(demand.Location.StartByte))
		buffer = binary.LittleEndian.AppendUint64(buffer, uint64(demand.Location.EndByte))
		if demand.QueryLocation != nil {
			buffer = append(buffer, 1)
			buffer = binary.LittleEndian.AppendUint64(buffer, uint64(demand.QueryLocation.StartByte))
			buffer = binary.LittleEndian.AppendUint64(buffer, uint64(demand.QueryLocation.EndByte))
		} else {
			buffer = append(buffer, 0)
		}
		buffer = append(buffer,
			flag(demand.Symbol), flag(demand.TypeDescriptor), flag(demand.ResolvedCall),
			flag(demand.Callability), flag(demand.References), flag(demand.Async), flag(demand.StructuralAccessor),
			flag(demand.ReferenceSpace), flag(demand.RuntimeIdentity),
		)
		_, _ = digest.Write(buffer)
	}
	return digest.Sum64()
}

// DurableSymbolID reports whether an identity survives its minting
// generation: durable IDs hash the declaration (ADR 0001), while
// generation-scoped counter IDs are meaningless outside the generation that
// issued them. The empty ID is trivially durable.
func DurableSymbolID(id SymbolID) bool {
	return id == "" || strings.HasPrefix(string(id), "symbol:h:")
}

func durableAsyncFunctions(facts []AsyncFunctionFact) bool {
	for _, fact := range facts {
		if !DurableSymbolID(fact.Symbol) || !DurableSymbolID(fact.Target) {
			return false
		}
	}
	return true
}

func symbolSetsEqual(left, right map[SymbolID]struct{}) bool {
	if len(left) != len(right) {
		return false
	}
	for symbol := range left {
		if _, ok := right[symbol]; !ok {
			return false
		}
	}
	return true
}

// entityAccumulator merges one file's aligned demand/entity/structural
// results with the same rules as the whole-batch builder. Same-location
// demands are adjacent in canonical order, so merging is a compare-with-last
// linear pass — no maps. Symbol lists may carry duplicates; the global
// assembly (builder queue, full-tier and union sets) deduplicates them.
type entityAccumulator struct {
	entities   []EntityFact
	enqueued   []SymbolID
	fullTier   []SymbolID
	structural []SymbolID
}

func newEntityAccumulator() *entityAccumulator {
	return &entityAccumulator{}
}

func (a *entityAccumulator) enqueue(symbol SymbolID) {
	if symbol == "" {
		return
	}
	a.enqueued = append(a.enqueued, symbol)
}

func (a *entityAccumulator) add(demand EntityDemand, entity EntityFact, structural SymbolID) {
	var target *EntityFact
	if last := len(a.entities) - 1; last >= 0 && a.entities[last].Location == entity.Location {
		target = &a.entities[last]
	} else {
		a.entities = append(a.entities, EntityFact{Location: entity.Location})
		target = &a.entities[len(a.entities)-1]
	}
	if entity.Symbol != "" {
		target.Symbol = entity.Symbol
		a.enqueue(entity.Symbol)
		if demand.References {
			a.fullTier = append(a.fullTier, entity.Symbol)
		}
	}
	if entity.TypeDescriptor != nil {
		target.TypeDescriptor = entity.TypeDescriptor
	}
	if entity.ResolvedCall != nil {
		target.ResolvedCall = entity.ResolvedCall
		a.enqueue(entity.ResolvedCall.Target)
		// Declaration, owner, and parameter identities are self-contained
		// metadata on the resolved-call fact. Only the call target is a symbol
		// closure lookup handle; hydrating standalone rows for every embedded
		// identity duplicates declarations and references across large call
		// sets.
	}
	if entity.Callability != "" {
		target.Callability = entity.Callability
	}
	if entity.ReferenceSpace != "" {
		target.ReferenceSpace = entity.ReferenceSpace
	}
	if entity.RuntimeIdentity != "" {
		target.RuntimeIdentity = entity.RuntimeIdentity
	}
	if structural != "" {
		a.structural = append(a.structural, structural)
	}
}

func (a *entityAccumulator) contribution(hash uint64) *fileClosureContribution {
	durable := true
	for _, symbol := range a.enqueued {
		if !DurableSymbolID(symbol) {
			durable = false
			break
		}
	}
	if durable {
		for _, symbol := range a.structural {
			if !DurableSymbolID(symbol) {
				durable = false
				break
			}
		}
	}
	descriptors := make([]symbolDescriptor, 0)
	dependencies := make(map[string]struct{})
	for index := range a.entities {
		entity := &a.entities[index]
		if entity.Symbol != "" && entity.TypeDescriptor != nil {
			descriptors = append(descriptors, symbolDescriptor{
				symbol:     entity.Symbol,
				descriptor: entity.TypeDescriptor,
			})
		}
		addDependency := func(location Location) {
			if location.Path != "" && filepath.Clean(location.Path) != filepath.Clean(entity.Location.Path) {
				dependencies[filepath.Clean(location.Path)] = struct{}{}
			}
		}
		if entity.TypeDescriptor != nil {
			for _, declaration := range entity.TypeDescriptor.AliasDeclarations {
				addDependency(declaration.Location)
			}
		}
		if call := entity.ResolvedCall; call != nil {
			if call.Declaration != nil {
				addDependency(call.Declaration.Location)
				for _, owner := range call.Declaration.Owners {
					addDependency(owner.Location)
				}
			}
			for _, mapping := range call.Arguments {
				if mapping.Parameter == nil {
					continue
				}
				if mapping.Parameter.Declaration != nil {
					addDependency(mapping.Parameter.Declaration.Location)
				}
				if mapping.Parameter.TypeDescriptor != nil {
					for _, declaration := range mapping.Parameter.TypeDescriptor.AliasDeclarations {
						addDependency(declaration.Location)
					}
				}
			}
		}
	}
	return &fileClosureContribution{
		demandHash:   hash,
		entities:     a.entities,
		descriptors:  descriptors,
		enqueued:     a.enqueued,
		fullTier:     a.fullTier,
		structural:   a.structural,
		dependencies: dependencies,
		durable:      durable,
	}
}

// materializeSemanticDemandRetained is the retained-aware counterpart of
// materializeSemanticDemand: files outside every accepted update's affected
// set whose demand lists hash identically reuse their previous
// contributions; only changed files run against the checker. A contribution
// is stored only when every identity it carries is durable (the source-fact
// memo's rule), so retained facts stay resolvable and wire output is
// byte-identical to a fresh whole-batch run. Descriptor suppression keeps
// whole-batch semantics via the structural-accessor union: recomputed files
// always run under the exact current union, and when the union differs from
// the previous generation's, the retained files whose descriptor demands
// touch the difference are refreshed under it. Caller holds p.mu.
func (p *DemandClosure) materializeSemanticDemandRetained(
	ctx context.Context,
	groups []demandGroup,
	generation uint64,
) (*FactTable, int, semanticDemandStages, ClosureRetention, error) {
	retention := ClosureRetention{}
	sources, err := p.backend.SourceFiles(ctx)
	if err != nil {
		return nil, 0, semanticDemandStages{}, retention, err
	}
	sort.Slice(sources, func(i, j int) bool { return sources[i].Path < sources[j].Path })
	table := p.recyclableTable
	p.recyclableTable = nil
	if table == nil {
		table = &FactTable{}
	}
	table.Schema = TypeFactsSchemaVersion
	table.Generation = generation
	table.ProjectID = "semantic-demand"
	table.Sources = sources
	table.Files = table.Files[:0]
	table.Entities = table.Entities[:0]
	// A recycled table may own chunks still shared by the immediately
	// preceding generation. Never reuse its flat symbol backing array.
	table.Symbols = nil
	table.symbols = nil
	table.transport = nil
	p.maybeResetInterner()
	builder := &closureBuilder{
		backend:                 p.backend,
		trace:                   p.trace,
		entities:                make(map[Location]*EntityFact),
		interner:                p.interner,
		symbolQueue:             p.queueScratch[:0],
		queueHandles:            p.queueHandleScratch[:0],
		symbolSeen:              newSymbolHandleSet(p.interner, p.seenScratch),
		fullTier:                newSymbolHandleSet(p.interner, p.fullScratch),
		changedSymbols:          newChangedSymbolSet(p.interner, p.changedScratch, p.changedIDScratch),
		factIndexScratch:        p.factIndexScratch,
		descriptors:             make(map[SymbolID]*TypeDescriptor),
		cleanPaths:              make(map[string]string),
		cachedSymbolFacts:       p.symbolFacts,
		cachedReferences:        p.symbolReferences,
		cachedSymbolOrder:       p.symbolOrder,
		symbolFactsBuffer:       p.symbolScratch,
		symbolOrderBuffer:       table.Symbols,
		removedSymbolCandidates: retainedSymbolCandidates(p.previousTable, p.transportChangedPaths),
	}
	// The generation's handle sets and queue live on closure-owned scratch;
	// hand the backing back for the next generation whichever way this one
	// ends.
	defer func() {
		p.seenScratch = builder.symbolSeen.members
		p.fullScratch = builder.fullTier.members
		p.changedScratch = builder.changedSymbols.set.members
		p.changedIDScratch = builder.changedSymbols.ids[:0]
		p.queueScratch = builder.symbolQueue[:0]
		p.queueHandleScratch = builder.queueHandles[:0]
		p.factIndexScratch = builder.factIndexScratch
	}()
	if p.previousTable != nil {
		builder.cachedCanonicalStore = p.previousTable.symbols
		if builder.cachedCanonicalStore == nil {
			builder.cachedCanonicalStore = newSymbolFactStore(p.previousTable.Symbols)
		}
	}
	// The async runs are rebuilt every generation, but into retained scratch:
	// counting first sizes the flat backing exactly, so it never reallocates
	// and each group's run is a stable capped window into it. The windows are
	// only ever read within the async section below — refresh lists copy the
	// demands out — so reusing the backing next generation is safe.
	asyncTotal := 0
	for index := range groups {
		for _, demand := range groups[index].demands {
			if demand.Async {
				asyncTotal++
			}
		}
	}
	asyncGroups := p.asyncGroupScratch[:0]
	if asyncTotal != 0 {
		flat := p.asyncDemandScratch
		if cap(flat) < asyncTotal {
			flat = make([]EntityDemand, 0, asyncTotal)
		} else {
			flat = flat[:0]
		}
		for index := range groups {
			start := len(flat)
			for _, demand := range groups[index].demands {
				if demand.Async {
					flat = append(flat, demand)
				}
			}
			if len(flat) > start {
				asyncGroups = append(asyncGroups, demandGroup{
					path:    groups[index].path,
					demands: flat[start:len(flat):len(flat)],
				})
			}
		}
		p.asyncDemandScratch = flat
	}
	stages := semanticDemandStages{}
	started := time.Now()
	refreshAllAsync := p.asyncFiles == nil || p.transportChangedPaths == nil
	refreshAsyncPaths := make(map[string]struct{})
	var refreshAsyncDemands []EntityDemand
	asyncByPath := make(map[string][]AsyncFunctionFact, len(asyncGroups))
	for _, group := range asyncGroups {
		_, changed := p.transportChangedPaths[group.path]
		cached, cachedOK := p.asyncFiles[group.path]
		if refreshAllAsync || changed || !cachedOK {
			refreshAsyncPaths[group.path] = struct{}{}
			refreshAsyncDemands = append(refreshAsyncDemands, group.demands...)
			continue
		}
		asyncByPath[group.path] = cached
		retention.RetainedAsyncFiles++
	}
	refreshedAsync, err := asyncFunctionsForDemands(ctx, p.backend, refreshAsyncDemands)
	if err != nil {
		return nil, 0, stages, retention, err
	}
	crossPathAsync := false
	for path := range refreshedAsync {
		if _, expected := refreshAsyncPaths[path]; !expected {
			crossPathAsync = true
			break
		}
	}
	if crossPathAsync && !refreshAllAsync {
		refreshAllAsync = true
		refreshAsyncPaths = make(map[string]struct{}, len(asyncGroups))
		refreshAsyncDemands = refreshAsyncDemands[:0]
		asyncByPath = make(map[string][]AsyncFunctionFact, len(asyncGroups))
		retention.RetainedAsyncFiles = 0
		for _, group := range asyncGroups {
			refreshAsyncPaths[group.path] = struct{}{}
			refreshAsyncDemands = append(refreshAsyncDemands, group.demands...)
		}
		refreshedAsync, err = asyncFunctionsForDemands(ctx, p.backend, refreshAsyncDemands)
		if err != nil {
			return nil, 0, stages, retention, err
		}
	}
	for path := range refreshAsyncPaths {
		asyncByPath[path] = nil
	}
	for path, facts := range refreshedAsync {
		asyncByPath[path] = facts
	}
	retention.RecomputedAsyncFiles = len(refreshAsyncPaths)
	nextAsyncFiles := make(map[string][]AsyncFunctionFact, len(asyncGroups))
	cacheableAsync := true
	for _, group := range asyncGroups {
		facts := asyncByPath[group.path]
		if !durableAsyncFunctions(facts) {
			cacheableAsync = false
			continue
		}
		nextAsyncFiles[group.path] = facts
	}
	if crossPathAsync {
		cacheableAsync = false
	}
	if cacheableAsync {
		p.asyncFiles = nextAsyncFiles
	} else {
		p.asyncFiles = nil
	}
	p.asyncGroupScratch = asyncGroups[:0]
	for _, source := range sources {
		if err := ctx.Err(); err != nil {
			return nil, 0, stages, retention, err
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
	if p.retained == nil {
		p.retained = make(map[string]*fileClosureContribution)
	}
	union := p.suppressionScratch
	p.suppressionScratch = nil
	if union == nil {
		union = make(map[SymbolID]struct{})
	} else {
		clear(union)
	}
	descriptorSeed := p.descriptorSeedScratch
	if descriptorSeed == nil {
		descriptorSeed = make(map[SymbolID]*TypeDescriptor)
		p.descriptorSeedScratch = descriptorSeed
	} else {
		clear(descriptorSeed)
	}
	var changed []int
	var changedDemands []EntityDemand
	for index := range groups {
		group := &groups[index]
		contribution := p.retained[group.path]
		_, pathChanged := p.transportChangedPaths[group.path]
		dependencyChanged := false
		for dependency := range contributionDependencies(contribution) {
			if _, changed := p.transportChangedPaths[dependency]; changed {
				dependencyChanged = true
				break
			}
		}
		// Update and demand-delta handling already name every path whose
		// demand run may differ. Hash unchanged runs only when no exact
		// changed-path set is available (initial/full materialization).
		if contribution == nil || p.transportChangedPaths == nil || pathChanged {
			// The accumulator merges same-location demands only when they are
			// adjacent, and the hash is order-sensitive, so a client that emits
			// a run in varying order would both mis-merge and defeat retention.
			// Canonicalize rather than reject: it costs nothing for a run that
			// already arrives sorted, and no caller can get it silently wrong.
			group.demands = canonicalDemandRun(group.demands)
			group.hash = demandListHash(group.demands, p.demandHashSeed())
		} else {
			group.hash = contribution.demandHash
		}
		if contribution != nil && !dependencyChanged && contribution.demandHash == group.hash {
			group.contribution = contribution
			for _, symbol := range contribution.structural {
				union[symbol] = struct{}{}
			}
			// Batch-wide first-wins descriptor dedup: descriptors the
			// retained files already carry are what a whole-batch run
			// would have cached before reaching the recomputed demands.
			for _, entry := range contribution.descriptors {
				if _, ok := descriptorSeed[entry.symbol]; !ok {
					descriptorSeed[entry.symbol] = entry.descriptor
				}
			}
			continue
		}
		changed = append(changed, index)
		changedDemands = append(changedDemands, group.demands...)
	}
	rebuildContributions := func(entities []EntityFact, structural []SymbolID, indices []int) {
		offset := 0
		for _, index := range indices {
			group := &groups[index]
			accumulator := newEntityAccumulator()
			for _, demand := range group.demands {
				accumulator.add(demand, entities[offset], structural[offset])
				offset++
			}
			group.contribution = accumulator.contribution(group.hash)
		}
	}
	if len(changedDemands) != 0 {
		entities, structural, err := p.backend.SemanticEntitiesScoped(ctx, changedDemands, union, descriptorSeed)
		if err != nil {
			return nil, 0, stages, retention, err
		}
		rebuildContributions(entities, structural, changed)
		for _, index := range changed {
			for _, symbol := range groups[index].contribution.structural {
				// Non-durable structural symbols re-mint each generation and
				// can only ever suppress entities in files that recompute
				// alongside them; comparing them across generations would
				// force a spurious whole-batch recompute.
				if DurableSymbolID(symbol) {
					union[symbol] = struct{}{}
				}
			}
		}
	}
	// Recomputed files always run under the exact current union (injected
	// retained structural symbols plus their own batch prefetch), so a
	// union change can only invalidate RETAINED contributions — and only
	// those whose descriptor demands touch a symbol whose suppression
	// status flipped.
	if p.lastSuppression != nil && !symbolSetsEqual(union, p.lastSuppression) {
		delta := make(map[SymbolID]struct{})
		for symbol := range union {
			if _, ok := p.lastSuppression[symbol]; !ok {
				delta[symbol] = struct{}{}
			}
		}
		for symbol := range p.lastSuppression {
			if _, ok := union[symbol]; !ok {
				delta[symbol] = struct{}{}
			}
		}
		var refresh []int
		var refreshDemands []EntityDemand
		wasChanged := make(map[int]bool, len(changed))
		for _, index := range changed {
			wasChanged[index] = true
		}
		for index := range groups {
			if wasChanged[index] {
				continue
			}
			group := &groups[index]
			var symbolAt map[Location]SymbolID
			for _, demand := range group.demands {
				if !demand.TypeDescriptor {
					continue
				}
				if symbolAt == nil {
					symbolAt = make(map[Location]SymbolID, len(group.contribution.entities))
					for entityIndex := range group.contribution.entities {
						entity := &group.contribution.entities[entityIndex]
						symbolAt[entity.Location] = entity.Symbol
					}
				}
				location := demand.Location
				location.Path = group.path
				if _, hit := delta[symbolAt[location]]; hit {
					refresh = append(refresh, index)
					refreshDemands = append(refreshDemands, group.demands...)
					break
				}
			}
		}
		if len(refresh) != 0 {
			retention.SuppressionRecompute = true
			entities, structural, err := p.backend.SemanticEntitiesScoped(ctx, refreshDemands, union, nil)
			if err != nil {
				return nil, 0, stages, retention, err
			}
			rebuildContributions(entities, structural, refresh)
			changed = append(changed, refresh...)
		}
	}
	// Every recomputed group's rows may differ from the preceding generation, and
	// the transport manifest is built from changed paths alone — so all of them
	// must be named here or the delta silently omits their rows. There are more
	// reasons to recompute than an edit: a changed demand run, a descriptor
	// refresh under a shifted suppression union, or a file holding non-durable
	// identities, which are re-minted every generation and so can never be
	// retained. Naming them is what lets such a file take the delta path instead
	// of forcing a whole-table pack.
	manifestChangedPaths := p.transportChangedPaths
	if len(changed) != 0 {
		manifestChangedPaths = maps.Clone(p.transportChangedPaths)
		if manifestChangedPaths == nil {
			manifestChangedPaths = make(map[string]struct{}, len(changed))
		}
		for _, index := range changed {
			manifestChangedPaths[groups[index].path] = struct{}{}
		}
	}

	nextRetained := make(map[string]*fileClosureContribution, len(groups))
	entityTotal := 0
	for index := range groups {
		group := &groups[index]
		// The source-fact memo's rule (ADR 0001): a contribution is stored
		// only when every identity it carries is durable. Files holding
		// generation-scoped counter IDs recompute every generation — all of
		// them together, in canonical order, so their minted counters match
		// a fresh whole-batch run.
		if group.contribution.durable {
			nextRetained[group.path] = group.contribution
		} else {
			retention.NonDurableFiles++
		}
		entityTotal += len(group.contribution.entities)
	}
	retention.RetainedFiles = len(groups) - len(changed)
	retention.RecomputedFiles = len(changed)
	// Files no longer demanded drop out here rather than lingering.
	p.retained = nextRetained
	// The outgoing lastSuppression becomes the next generation's union
	// scratch; it was built two generations ago, so it is never the map the
	// suppression-delta comparison above just read against.
	p.suppressionScratch = p.lastSuppression
	p.lastSuppression = union
	stages.demand = time.Since(started)
	started = time.Now()
	// Contributions are per-file and files are disjoint, so assembly is
	// concatenation plus one global sort — the same output order as the
	// whole-batch builder's location-keyed map.
	entities := table.Entities[:0]
	if cap(entities) < entityTotal {
		entities = make([]EntityFact, 0, entityTotal)
	}
	for index := range groups {
		group := &groups[index]
		entities = append(entities, group.contribution.entities...)
		for _, symbol := range group.contribution.enqueued {
			builder.enqueueSymbol(symbol)
		}
		for _, symbol := range group.contribution.fullTier {
			builder.fullTier.addID(symbol)
		}
	}
	stages.assembly = time.Since(started)
	started = time.Now()
	sort.Slice(entities, func(i, j int) bool {
		left, right := entities[i].Location, entities[j].Location
		if left.Path != right.Path {
			return left.Path < right.Path
		}
		if left.StartByte != right.StartByte {
			return left.StartByte < right.StartByte
		}
		return left.EndByte < right.EndByte
	})
	stages.sort = time.Since(started)
	started = time.Now()
	symbols, err := builder.closeSymbols(ctx)
	if err != nil {
		return nil, 0, stages, retention, err
	}
	p.symbolScratch = builder.symbolFactsBuffer
	symbolStore := builder.closedSymbolStore
	if symbolStore == nil {
		symbolStore = newSymbolFactStore(symbols)
	}
	stages.close = time.Since(started)
	retention.CachedSymbolFacts = builder.cachedSymbolHits
	retention.RecomputedSymbolFacts = builder.recomputedSymbolFacts
	retention.CachedReferenceFacts = builder.cachedReferenceHits
	retention.RecomputedReferences = builder.recomputedReferences
	retention.PatchedSymbolRows = builder.patchedSymbolRows
	retention.SharedSymbolChunks = builder.sharedSymbolChunks
	nextSymbolFacts := p.symbolFacts
	if nextSymbolFacts == nil {
		nextSymbolFacts = make(map[SymbolID]SymbolFact, len(symbols))
	}
	for id, fact := range nextSymbolFacts {
		if !builder.symbolSeen.containsID(id) {
			p.unindexSymbolFact(fact)
			delete(nextSymbolFacts, id)
		}
	}
	symbolStore.Range(func(fact SymbolFact) {
		// Only declaration-backed durable identities are safe across
		// generations. A declaration-less synthetic symbol may change its
		// alias meaning without giving Update a path by which to evict it.
		if !DurableSymbolID(fact.ID) || !DurableSymbolID(fact.AliasTarget) || len(fact.Declarations) == 0 {
			if previous, retained := nextSymbolFacts[fact.ID]; retained {
				p.unindexSymbolFact(previous)
				delete(nextSymbolFacts, fact.ID)
			}
			return
		}
		if builder.changedSymbols.containsID(fact.ID) {
			if previous, retained := nextSymbolFacts[fact.ID]; retained {
				p.unindexSymbolFact(previous)
			}
			retainedFact := SymbolFact{
				ID:           fact.ID,
				AliasTarget:  fact.AliasTarget,
				Declarations: fact.Declarations,
			}
			nextSymbolFacts[fact.ID] = retainedFact
			p.indexSymbolFact(retainedFact)
		}
	})
	p.symbolFacts = nextSymbolFacts
	p.symbolReferences = builder.closedReferences
	if cap(p.symbolOrder) < symbolStore.Len() {
		p.symbolOrder = make([]SymbolID, symbolStore.Len())
	} else {
		p.symbolOrder = p.symbolOrder[:symbolStore.Len()]
	}
	symbolIndex := 0
	symbolStore.Range(func(fact SymbolFact) {
		p.symbolOrder[symbolIndex] = fact.ID
		symbolIndex++
	})
	table.Symbols = symbols
	table.symbols = symbolStore
	table.Entities = entities
	table.transport = transportManifest(p.previousTable, table, builder, manifestChangedPaths)
	p.recyclableTable = p.previousTable
	p.previousTable = nil
	p.transportChangedPaths = nil
	// The table is transport-only: it exists to be converted to the wire shape
	// or diffed against its predecessor, and answers no per-location queries.
	stages.symbol = stages.assembly + stages.sort + stages.close
	return table, builder.fullTier.len(), stages, retention, nil
}

func contributionDependencies(contribution *fileClosureContribution) map[string]struct{} {
	if contribution == nil {
		return nil
	}
	return contribution.dependencies
}

// ClosureRetention reports how much of a generation's demand closure was
// carried over from retained per-file contributions.
type ClosureRetention struct {
	RetainedFiles         int  `json:"retainedFiles"`
	RecomputedFiles       int  `json:"recomputedFiles"`
	RetainedAsyncFiles    int  `json:"retainedAsyncFiles,omitempty"`
	RecomputedAsyncFiles  int  `json:"recomputedAsyncFiles,omitempty"`
	NonDurableFiles       int  `json:"nonDurableFiles,omitempty"`
	SuppressionRecompute  bool `json:"suppressionRecompute,omitempty"`
	CachedSymbolFacts     int  `json:"cachedSymbolFacts,omitempty"`
	RecomputedSymbolFacts int  `json:"recomputedSymbolFacts,omitempty"`
	CachedReferenceFacts  int  `json:"cachedReferenceFacts,omitempty"`
	RecomputedReferences  int  `json:"recomputedReferences,omitempty"`
	PatchedSymbolRows     int  `json:"patchedSymbolRows,omitempty"`
	SharedSymbolChunks    int  `json:"sharedSymbolChunks,omitempty"`
}
