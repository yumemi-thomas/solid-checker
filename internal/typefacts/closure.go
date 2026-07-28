package typefacts

import (
	"context"
	"errors"
	"hash/maphash"
	"path/filepath"
	"sync"
	"time"
)

// ClosureBackend is the live project surface the demand closure materializes
// from. Every method here is called by the closure: there are no optional
// capabilities and no fallbacks, because there is exactly one implementation
// (the tsgo adapter) and it satisfies all of them. A capability probe with one
// adapter encodes a second adapter that has never existed.
//
// Every path returned by a backend method is clean. SourceFiles returns
// strictly path-ordered rows. SemanticDemandRuns
// accepts runs whose paths and every nested location are already clean and
// returns clean, strictly ordered, unique dependency paths that exclude their
// owning run. These are adapter-seam invariants: the closure checks returned
// evidence and rejects malformed rows, but neither side normalizes paths again.
//
// SemanticDemandRuns resolves a demand subset whose output must match a larger
// batch's semantics. The scope carries structural-accessor symbols and type
// descriptors from outside the subset; aligned per-file results carry explicit
// retention evidence without flattening and repartitioning file ownership.
//
// ReferencesBatch resolves reference lists under one backend lock, keyed by the
// requested ID; an absent key means the same as an empty list.
// ChangedReferences reports the exact canonical-symbol keys whose reference
// lists moved since the preceding generation, and reports exact=false after a
// first build or a broad invalidation — a live answer, not a missing
// capability. It must stay stable for the generation so retries and cancelled
// analyses do not advance hidden state.
type ClosureBackend interface {
	SourceFiles(context.Context) ([]SourceFile, error)
	Update(context.Context, []FileChange) (AffectedSet, error)
	Close() error

	ResolveAlias(context.Context, SymbolID) (SymbolID, error)
	Declarations(context.Context, SymbolID) ([]Declaration, error)

	SemanticDemandRuns(context.Context, []SemanticDemandRun, SemanticScope) ([]SemanticDemandRunResult, error)
	AsyncFunctionsAt(context.Context, []Location) ([]AsyncFunctionFact, error)
	ReferencesBatch(context.Context, []SymbolID) (map[SymbolID][]Location, error)
	ChangedReferences(context.Context) (ids []SymbolID, exact bool, err error)
}

// ClosureStats reports the cost of one generation's closed fact table.
type ClosureStats struct {
	BuildSequence    uint64           `json:"-"`
	Generation       uint64           `json:"generation"`
	Files            int              `json:"files"`
	Entities         int              `json:"entities"`
	Symbols          int              `json:"symbols"`
	FullTierSymbols  int              `json:"fullTierSymbols"`
	BuildDuration    time.Duration    `json:"buildDurationNs"`
	AsyncDuration    time.Duration    `json:"asyncDurationNs,omitempty"`
	DemandDuration   time.Duration    `json:"demandDurationNs,omitempty"`
	SymbolDuration   time.Duration    `json:"symbolDurationNs,omitempty"`
	AssemblyDuration time.Duration    `json:"assemblyDurationNs,omitempty"`
	SortDuration     time.Duration    `json:"sortDurationNs,omitempty"`
	CloseDuration    time.Duration    `json:"closeDurationNs,omitempty"`
	Retention        ClosureRetention `json:"retention,omitzero"`
}

// DemandClosure produces one generation's fact table from a demand set, taking
// the transitive expansion over reachable symbols to a fixed point against the
// live backend. The project's complete fact universe is never enumerated.
//
// An accepted Update advances the generation and discards the table; the next
// analysis rebuilds it, reusing every per-file contribution whose file stayed
// outside the affected set and whose demand run is unchanged (ADR 0001).
//
// The table it returns is transport-only: it exists to be converted to the wire
// shape or diffed against its predecessor, and answers no per-location queries.
type DemandClosure struct {
	mu      sync.Mutex
	backend ClosureBackend
	trace   Trace
	table   *FactTable
	// previousTable and transportChangedPaths describe the immediate
	// predecessor of the next table. The private transport manifest is valid
	// only for that base generation; protocol callers with an older
	// acknowledged snapshot automatically take the full diff path.
	// recyclableTable is the generation before that, whose slice storage the
	// next materialization may overwrite in place — so nothing may retain it.
	previousTable         *FactTable
	recyclableTable       *FactTable
	transportChangedPaths map[string]struct{}
	// interner assigns every symbol this session reaches a dense stable
	// handle; the scratch slices below back the per-generation handle sets so
	// a generation allocates no symbol-keyed maps at all.
	interner           *symbolInterner
	seenScratch        []bool
	fullScratch        []bool
	changedScratch     []bool
	changedIDScratch   []SymbolID
	queueScratch       []SymbolID
	queueHandleScratch []int32
	factIndexScratch   []int32
	// lastRoots and lastFullRoots retain the raw, pre-alias seed sets of the
	// preceding successful generation. lastFullTier is the expanded reference
	// tier. Stable seeds plus unchanged alias edges prove the same fixed point
	// without walking the complete symbol universe again.
	lastRoots               []bool
	lastFullRoots           []bool
	lastFullTier            []bool
	rootSnapshotScratch     []bool
	fullRootSnapshotScratch []bool
	fullTierSnapshotScratch []bool
	stats                   ClosureStats
	generation              uint64
	nextTableStateID        uint64
	closed                  bool
	// retained owns immutable per-file Retained contributions and the reverse
	// indexes used to invalidate exact dependents and structural-descriptor
	// users. retainedPathScratch is the transaction's reusable desired set.
	retained            retainedContributionStore
	retainedPathScratch map[string]struct{}
	// lastSuppression is the preceding generation's structural-accessor union.
	// A changed union refreshes only paths named by retained.descriptorUsers.
	lastSuppression map[SymbolID]struct{}
	// suppressionScratch and descriptorSeedScratch back the per-generation
	// suppression union and descriptor seed. Both are derived in full every
	// generation — the first-wins descriptor order makes true incremental
	// maintenance fragile — but deriving into cleared, retained maps costs no
	// allocation once the session reaches steady state. The union ping-pongs
	// with lastSuppression, which must stay a distinct map for the
	// suppression-delta comparison.
	suppressionScratch    map[SymbolID]struct{}
	descriptorSeedScratch map[SymbolID]*TypeDescriptor
	// symbolFacts memoizes the generation-independent half of durable
	// symbol closure: alias targets and declarations. Update removes facts
	// declared in affected files. symbolReferences separately retains lists
	// whose presence is known (including known-empty lists); because an edit
	// can add a reference to an otherwise unchanged symbol, they are reused
	// only when the backend supplies an exact changed-symbol delta.
	symbolFacts      map[SymbolID]SymbolFact
	symbolReferences map[SymbolID][]Location
	// symbolsByPath indexes symbolFacts by cleaned declaring path, so Update
	// evicts an affected set by looking up its paths instead of scanning every
	// retained fact. Every write to symbolFacts goes through indexSymbolFact /
	// unindexSymbolFact to keep the two exactly in sync.
	symbolsByPath map[string]map[SymbolID]struct{}
	symbolScratch []SymbolFact
	// invalidatedSymbols names every previously reached durable fact evicted
	// by accepted source updates. symbolMemoComplete proves that this evidence
	// covered the entire preceding canonical store.
	invalidatedSymbols map[SymbolID]struct{}
	symbolMemoComplete bool
	// asyncFiles retains complete durable async-function contributions by
	// demand path. Exact changed-path manifests let ordinary edits query only
	// changed files; cross-file selections fall back to a full async batch.
	// The two scratch slices back each generation's async demand runs: a flat
	// buffer the runs window into, and the group list itself.
	asyncFiles         map[string][]AsyncFunctionFact
	asyncDemandScratch []EntityDemand
	asyncGroupScratch  []demandGroup
	demandSeed         maphash.Seed
}

// NewDemandClosure wraps a live backend, which must satisfy every capability
// the closure calls. trace may be nil, which disables tracing entirely.
func NewDemandClosure(backend Project, trace Trace) (*DemandClosure, error) {
	full, ok := backend.(ClosureBackend)
	if !ok {
		return nil, errors.New("demand closure requires the demand-run semantic, async, and reference-batch capabilities")
	}
	return &DemandClosure{backend: full, trace: trace, generation: 1}, nil
}

// indexSymbolFact records a retained fact under each clean declaring path.
// Callers must pair it with every insert into symbolFacts.
func (p *DemandClosure) indexSymbolFact(fact SymbolFact) {
	if p.symbolsByPath == nil {
		p.symbolsByPath = make(map[string]map[SymbolID]struct{})
	}
	for _, declaration := range fact.Declarations {
		path := declaration.Location.Path
		ids := p.symbolsByPath[path]
		if ids == nil {
			ids = make(map[SymbolID]struct{})
			p.symbolsByPath[path] = ids
		}
		ids[fact.ID] = struct{}{}
	}
}

// unindexSymbolFact removes a retained fact from the path index. Callers must
// pair it with every delete from symbolFacts, passing the fact being removed.
func (p *DemandClosure) unindexSymbolFact(fact SymbolFact) {
	for _, declaration := range fact.Declarations {
		path := declaration.Location.Path
		ids := p.symbolsByPath[path]
		delete(ids, fact.ID)
		if len(ids) == 0 {
			delete(p.symbolsByPath, path)
		}
	}
}

// demandHashSeed lazily initializes the process-local seed for demand-run
// hashing; retained state never crosses processes.
func (p *DemandClosure) demandHashSeed() maphash.Seed {
	if p.demandSeed == (maphash.Seed{}) {
		p.demandSeed = maphash.MakeSeed()
	}
	return p.demandSeed
}

// Stats returns the most recent generation's materialization cost.
func (p *DemandClosure) Stats() ClosureStats {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.stats
}

// Update applies editor overlays and advances exactly one generation, even when
// the backend reports no affected source files. Retained contributions survive
// except for the affected set and the changed files themselves, which are
// evicted here rather than when next demanded.
func (p *DemandClosure) Update(ctx context.Context, changes []FileChange) (AffectedSet, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	affected, err := p.backend.Update(ctx, changes)
	if err != nil {
		return affected, err
	}
	invalidPaths := make(map[string]struct{}, len(affected.Files)+len(changes))
	for _, path := range affected.Files {
		invalidPaths[path] = struct{}{}
	}
	for _, change := range changes {
		invalidPaths[filepath.Clean(change.Path)] = struct{}{}
	}
	for path := range invalidPaths {
		for id := range p.symbolsByPath[path] {
			if p.invalidatedSymbols == nil {
				p.invalidatedSymbols = make(map[SymbolID]struct{})
			}
			p.invalidatedSymbols[id] = struct{}{}
			if fact, retained := p.symbolFacts[id]; retained {
				p.unindexSymbolFact(fact)
				delete(p.symbolFacts, id)
			}
			delete(p.symbolReferences, id)
		}
	}
	p.previousTable = p.table
	p.table = nil
	p.transportChangedPaths = invalidPaths
	// Accepted source state invalidates the exact Retained contributions and
	// their direct dependency users immediately. Analyze-time preparation is
	// transactional, but an accepted update itself cannot be rolled back.
	p.retained.invalidate(invalidPaths)
	// Every accepted protocol update advances exactly one generation, even
	// when the backend reports no affected source files.
	p.generation++
	return affected, nil
}

func (p *DemandClosure) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.closed = true
	p.resetAnalysisStateLocked()
	return p.backend.Close()
}

func (p *DemandClosure) resetAnalysisStateLocked() {
	p.table = nil
	p.previousTable = nil
	p.recyclableTable = nil
	p.transportChangedPaths = nil
	p.symbolFacts = nil
	p.symbolReferences = nil
	p.symbolsByPath = nil
	p.symbolScratch = nil
	p.retained.reset()
	p.retainedPathScratch = nil
	p.lastSuppression = nil
	p.suppressionScratch = nil
	p.descriptorSeedScratch = nil
	p.asyncFiles = nil
	p.asyncDemandScratch = nil
	p.asyncGroupScratch = nil
	p.interner = nil
	p.seenScratch = nil
	p.fullScratch = nil
	p.changedScratch = nil
	p.changedIDScratch = nil
	p.queueScratch = nil
	p.queueHandleScratch = nil
	p.factIndexScratch = nil
	p.lastRoots = nil
	p.lastFullRoots = nil
	p.lastFullTier = nil
	p.rootSnapshotScratch = nil
	p.fullRootSnapshotScratch = nil
	p.fullTierSnapshotScratch = nil
	p.invalidatedSymbols = nil
	p.symbolMemoComplete = false
}

// abandonAnalysis drops every derived analysis cache after a materialized
// table failed to publish through its owning Session. Reconstructing on the
// next analyze is intentionally conservative and rare: retaining any part of
// the rejected demand snapshot could make it observable through a later,
// otherwise unrelated demand delta.
func (p *DemandClosure) abandonAnalysis() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.resetAnalysisStateLocked()
}

// SourceFiles answers from the retained backend without materializing the
// demand closure: the table's source list is populated verbatim from the
// backend, and a table forced into existence here is discarded as soon as the
// first analyze request arrives with real compiler seeds.
func (p *DemandClosure) SourceFiles(ctx context.Context) ([]SourceFile, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, errors.New("closure project is closed")
	}
	return p.backend.SourceFiles(ctx)
}
