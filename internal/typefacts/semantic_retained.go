package typefacts

import (
	"context"
	"encoding/binary"
	"fmt"
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
	contribution *retainedContribution
}

func validateCanonicalSourceFiles(files []SourceFile) error {
	for index := range files {
		path := files[index].Path
		if path == "" || filepath.Clean(path) != path {
			return fmt.Errorf("source file path %q is not clean and non-empty", path)
		}
		if index != 0 && files[index-1].Path >= path {
			return fmt.Errorf("source files are not strictly path-ordered at %q", path)
		}
	}
	return nil
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
	if err := validateCanonicalSourceFiles(sources); err != nil {
		return nil, 0, semanticDemandStages{}, retention, err
	}
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
	var cachedCanonicalStore *symbolFactStore
	if p.previousTable != nil {
		cachedCanonicalStore = p.previousTable.symbols
		if cachedCanonicalStore == nil {
			cachedCanonicalStore = newSymbolFactStore(p.previousTable.Symbols)
		}
	}
	p.maybeResetInterner(cachedCanonicalStore.Len())
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
		cachedReferences:        p.symbolReferences,
		cachedCanonicalStore:    cachedCanonicalStore,
		invalidatedSymbols:      p.invalidatedSymbols,
		symbolFactsBuffer:       p.symbolScratch,
		symbolOrderBuffer:       table.Symbols,
		removedSymbolCandidates: retainedSymbolCandidates(p.previousTable, p.transportChangedPaths),
	}
	// The generation's handle sets and queue live on closure-owned scratch;
	// hand the backing back for the next generation whichever way this one
	// ends.
	releaseLinearScratch := false
	defer func() {
		p.seenScratch = builder.symbolSeen.members
		p.fullScratch = builder.fullTier.members
		p.changedScratch = builder.changedSymbols.set.members
		if releaseLinearScratch {
			p.changedIDScratch = nil
			p.queueScratch = nil
			p.queueHandleScratch = nil
			p.factIndexScratch = nil
		} else {
			p.changedIDScratch = builder.changedSymbols.ids[:0]
			p.queueScratch = builder.symbolQueue[:0]
			p.queueHandleScratch = builder.queueHandles[:0]
			p.factIndexScratch = builder.factIndexScratch
		}
	}()
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
		path := source.Path
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
	union := p.suppressionScratch
	p.suppressionScratch = nil
	if union == nil {
		union = make(map[SymbolID]struct{})
	} else {
		clear(union)
	}
	suppressionCommitted := false
	defer func() {
		if !suppressionCommitted {
			p.suppressionScratch = union
		}
	}()
	descriptorSeed := p.descriptorSeedScratch
	if descriptorSeed == nil {
		descriptorSeed = make(map[SymbolID]*TypeDescriptor)
		p.descriptorSeedScratch = descriptorSeed
	} else {
		clear(descriptorSeed)
	}
	var changed []int
	var changedRuns []SemanticDemandRun
	for index := range groups {
		group := &groups[index]
		contribution := p.retained.get(group.path)
		_, pathChanged := p.transportChangedPaths[group.path]
		// Update and demand-delta handling already name every path whose
		// demand run may differ. Hash unchanged runs only when no exact
		// changed-path set is available (initial/full materialization).
		if contribution == nil || p.transportChangedPaths == nil || pathChanged {
			group.hash = demandListHash(group.demands, p.demandHashSeed())
		} else {
			group.hash = contribution.demandHash
		}
		if contribution != nil && contribution.demandHash == group.hash {
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
		changedRuns = append(changedRuns, SemanticDemandRun{
			Path:    group.path,
			Demands: group.demands,
		})
	}
	rebuildContributions := func(results []SemanticDemandRunResult, indices []int) error {
		if len(results) != len(indices) {
			return fmt.Errorf("semantic demand run results = %d, want %d", len(results), len(indices))
		}
		for resultIndex, index := range indices {
			group := &groups[index]
			contribution, err := prepareRetainedContribution(
				group.path,
				group.hash,
				group.demands,
				results[resultIndex],
			)
			if err != nil {
				return err
			}
			group.contribution = contribution
		}
		return nil
	}
	if len(changedRuns) != 0 {
		results, err := p.backend.SemanticDemandRuns(ctx, changedRuns, SemanticScope{
			Suppression:    union,
			DescriptorSeed: descriptorSeed,
		})
		if err != nil {
			return nil, 0, stages, retention, err
		}
		if err := rebuildContributions(results, changed); err != nil {
			return nil, 0, stages, retention, err
		}
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
		refreshSet := make(map[int]struct{})
		for symbol := range delta {
			p.retained.rangeDescriptorUsers(symbol, func(path string) {
				index := sort.Search(len(groups), func(index int) bool {
					return groups[index].path >= path
				})
				if index == len(groups) ||
					groups[index].path != path ||
					groups[index].contribution != p.retained.get(path) {
					return
				}
				refreshSet[index] = struct{}{}
			})
		}
		refresh := make([]int, 0, len(refreshSet))
		for index := range refreshSet {
			refresh = append(refresh, index)
		}
		sort.Ints(refresh)
		refreshRuns := make([]SemanticDemandRun, 0, len(refresh))
		for _, index := range refresh {
			refreshRuns = append(refreshRuns, SemanticDemandRun{
				Path:    groups[index].path,
				Demands: groups[index].demands,
			})
		}
		if len(refresh) != 0 {
			retention.SuppressionRecompute = true
			results, err := p.backend.SemanticDemandRuns(ctx, refreshRuns, SemanticScope{
				Suppression: union,
			})
			if err != nil {
				return nil, 0, stages, retention, err
			}
			if err := rebuildContributions(results, refresh); err != nil {
				return nil, 0, stages, retention, err
			}
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

	entityTotal := 0
	for index := range groups {
		group := &groups[index]
		// The source-fact memo's rule (ADR 0001): a contribution is stored
		// only when every identity it carries is durable. Files holding
		// generation-scoped counter IDs recompute every generation — all of
		// them together, in canonical order, so their minted counters match
		// a fresh whole-batch run.
		if !group.contribution.durable {
			retention.NonDurableFiles++
		}
		entityTotal += len(group.contribution.entities)
	}
	retention.RetainedFiles = len(groups) - len(changed)
	retention.RecomputedFiles = len(changed)
	stages.demand = time.Since(started)
	started = time.Now()
	// Groups are path-sorted and unique, and each immutable contribution was
	// prepared from one canonical demand run. Concatenation therefore already
	// is the fact table's canonical location order.
	entities := table.Entities[:0]
	if cap(entities) < entityTotal {
		entities = make([]EntityFact, 0, entityTotal)
	}
	for index := range groups {
		group := &groups[index]
		start := len(entities)
		entities = append(entities, group.contribution.entities...)
		// The canonical table is the retained entity backing store. Repoint
		// each contribution at its capped path window so the per-file
		// preparation arrays become collectible instead of duplicating the
		// complete entity table for the rest of the session.
		group.contribution.entities = entities[start:len(entities):len(entities)]
		for _, symbol := range group.contribution.enqueued {
			builder.enqueueSymbol(symbol)
		}
		for _, symbol := range group.contribution.fullTier {
			builder.fullTier.addID(symbol)
		}
	}
	rootSnapshot := copyHandleMembership(builder.symbolSeen.members, p.rootSnapshotScratch)
	fullRootSnapshot := copyHandleMembership(builder.fullTier.members, p.fullRootSnapshotScratch)
	p.rootSnapshotScratch = nil
	p.fullRootSnapshotScratch = nil
	snapshotsCommitted := false
	defer func() {
		if !snapshotsCommitted {
			p.rootSnapshotScratch = rootSnapshot
			p.fullRootSnapshotScratch = fullRootSnapshot
		}
	}()
	stages.assembly = time.Since(started)
	started = time.Now()
	var symbols []SymbolFact
	stableSeeds := equalHandleMembership(rootSnapshot, p.lastRoots) &&
		equalHandleMembership(fullRootSnapshot, p.lastFullRoots)
	if stableSeeds {
		patched, err := builder.patchStableSymbolUniverse(
			ctx,
			p.invalidatedSymbols,
			p.symbolMemoComplete,
			p.lastFullTier,
		)
		if err != nil {
			return nil, 0, stages, retention, err
		}
		if !patched {
			symbols, err = builder.closeSymbols(ctx)
			if err != nil {
				return nil, 0, stages, retention, err
			}
		}
	} else {
		var err error
		symbols, err = builder.closeSymbols(ctx)
		if err != nil {
			return nil, 0, stages, retention, err
		}
	}
	fullTierSnapshot := copyHandleMembership(builder.fullTier.members, p.fullTierSnapshotScratch)
	p.fullTierSnapshotScratch = nil
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
	retention.StableSymbolClosure = builder.stableUniversePatch
	// symbolsByPath is the only auxiliary index the immutable canonical store
	// needs. Synchronize it from exact deltas when possible; cold/fallback
	// closure scans the canonical rows once without retaining a duplicate map.
	if builder.closedSymbolStore != nil && p.symbolsByPath != nil {
		for _, id := range builder.removedSymbolIDs {
			if fact, present := cachedCanonicalStore.Get(id); present {
				p.unindexSymbolFact(fact)
			}
		}
		for _, id := range builder.changedSymbols.ids {
			if previous, present := cachedCanonicalStore.Get(id); present {
				p.unindexSymbolFact(previous)
			}
			if fact, present := symbolStore.Get(id); present &&
				DurableSymbolID(fact.ID) && DurableSymbolID(fact.AliasTarget) && len(fact.Declarations) != 0 {
				p.indexSymbolFact(fact)
			}
		}
	} else {
		p.symbolsByPath = nil
		p.symbolMemoComplete = true
		symbolStore.Range(func(fact SymbolFact) {
			if !DurableSymbolID(fact.ID) || !DurableSymbolID(fact.AliasTarget) || len(fact.Declarations) == 0 {
				p.symbolMemoComplete = false
				return
			}
			p.indexSymbolFact(fact)
		})
	}
	p.symbolReferences = builder.closedReferences
	clear(p.invalidatedSymbols)
	table.Symbols = symbols
	table.symbols = symbolStore
	table.Entities = entities
	p.nextTableStateID++
	if p.nextTableStateID == 0 {
		// Zero is reserved for hand-built/non-retained tables, whose
		// manifests always take the canonical fallback diff.
		p.nextTableStateID++
	}
	table.stateID = p.nextTableStateID
	table.transport = transportManifest(p.previousTable, table, builder, manifestChangedPaths)
	if p.retainedPathScratch == nil {
		p.retainedPathScratch = make(map[string]struct{}, len(groups))
	}
	p.retained.commit(groups, p.retainedPathScratch)
	// Retained contributions and their suppression context commit together
	// only after every fallible preparation stage has succeeded.
	previousSuppression := p.lastSuppression
	p.lastSuppression = union
	p.suppressionScratch = previousSuppression
	suppressionCommitted = true
	p.rootSnapshotScratch = p.lastRoots
	p.lastRoots = rootSnapshot
	p.fullRootSnapshotScratch = p.lastFullRoots
	p.lastFullRoots = fullRootSnapshot
	p.fullTierSnapshotScratch = p.lastFullTier
	p.lastFullTier = fullTierSnapshot
	snapshotsCommitted = true
	p.recyclableTable = p.previousTable
	p.previousTable = nil
	p.transportChangedPaths = nil
	// The descriptor seed is a generation-local lookup derived entirely from
	// retained contributions. Its cold-sized buckets otherwise duplicate that
	// durable state for the whole session; rebuild the small changed-generation
	// view instead of retaining the cold map.
	p.descriptorSeedScratch = nil
	// The canonical chunk store now owns every closed SymbolFact. A spare flat
	// full-closure buffer is useful only for fallback rebuilds and duplicates
	// that store after the normal cold path; incremental patches allocate only
	// their changed subset when this buffer is absent.
	p.symbolScratch = nil
	// Async filtering and closure queues are linear working sets, not retained
	// semantic state. Keep the dense membership snapshots that prove the next
	// incremental fixed point, but release these duplicate cold-sized rows.
	p.asyncDemandScratch = nil
	p.asyncGroupScratch = nil
	releaseLinearScratch = true
	p.backend.ReleaseAnalysisState()
	// The table is transport-only: it exists to be converted to the wire shape
	// or diffed against its predecessor, and answers no per-location queries.
	stages.symbol = stages.assembly + stages.sort + stages.close
	return table, builder.fullTier.len(), stages, retention, nil
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
	StableSymbolClosure   bool `json:"stableSymbolClosure,omitempty"`
}
