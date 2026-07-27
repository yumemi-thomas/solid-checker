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
// SemanticEntitiesScoped resolves a demand subset whose output must match a
// larger batch's semantics: the suppression set carries structural-accessor
// symbols from outside the subset, the descriptor seed carries type descriptors
// already computed for demands outside it (batch-wide first-wins dedup), and
// the second result reports the subset's structural-accessor symbol per demand.
// That is what lets per-file retention reproduce whole-batch output.
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

	SemanticEntitiesScoped(context.Context, []EntityDemand, map[SymbolID]struct{}, map[SymbolID]*TypeDescriptor) ([]EntityFact, []SymbolID, error)
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
	stats              ClosureStats
	generation         uint64
	closed             bool
	// retained carries per-file demand-closure contributions across
	// generations (ADR 0001): an accepted update drops exactly the affected
	// set, and a file whose demands are unchanged reuses its entity facts
	// instead of re-resolving them against the checker. lastSuppression is
	// the previous generation's structural-accessor union; when the union
	// changes, every file is recomputed so descriptor suppression keeps
	// whole-batch semantics.
	retained        map[string]*fileClosureContribution
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
	// symbolOrder is the preceding materialized table's canonical ID order.
	// Retained closure uses it as an ordering index so an ordinary edit only
	// sorts genuinely new symbols instead of re-sorting the complete table.
	symbolOrder   []SymbolID
	symbolScratch []SymbolFact
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

// fileClosureContribution is one file's share of the semantic demand
// closure, valid while the file stays outside every accepted update's
// affected set and its demand list hashes identically.
type fileClosureContribution struct {
	demandHash  uint64
	entities    []EntityFact
	descriptors []symbolDescriptor
	enqueued    []SymbolID
	fullTier    []SymbolID
	structural  []SymbolID
	durable     bool
}

type symbolDescriptor struct {
	symbol     SymbolID
	descriptor *TypeDescriptor
}

// NewDemandClosure wraps a live backend, which must satisfy every capability
// the closure calls. trace may be nil, which disables tracing entirely.
func NewDemandClosure(backend Project, trace Trace) (*DemandClosure, error) {
	full, ok := backend.(ClosureBackend)
	if !ok {
		return nil, errors.New("demand closure requires the scoped semantic, async, and reference-batch capabilities")
	}
	return &DemandClosure{backend: full, trace: trace, generation: 1}, nil
}

// indexSymbolFact records a retained fact under each of its cleaned declaring
// paths. Callers must pair it with every insert into symbolFacts.
func (p *DemandClosure) indexSymbolFact(fact SymbolFact) {
	if p.symbolsByPath == nil {
		p.symbolsByPath = make(map[string]map[SymbolID]struct{})
	}
	for _, declaration := range fact.Declarations {
		path := filepath.Clean(declaration.Location.Path)
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
		path := filepath.Clean(declaration.Location.Path)
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
		invalidPaths[filepath.Clean(path)] = struct{}{}
	}
	for _, change := range changes {
		invalidPaths[filepath.Clean(change.Path)] = struct{}{}
	}
	for path := range invalidPaths {
		for id := range p.symbolsByPath[path] {
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
	// Retained contributions survive except the affected set; a
	// departed file must be evicted now, not when next queried.
	for _, path := range affected.Files {
		delete(p.retained, filepath.Clean(path))
	}
	for _, change := range changes {
		delete(p.retained, filepath.Clean(change.Path))
	}
	// Every accepted protocol update advances exactly one generation, even
	// when the backend reports no affected source files.
	p.generation++
	return affected, nil
}

func (p *DemandClosure) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.closed = true
	p.table = nil
	p.previousTable = nil
	p.recyclableTable = nil
	p.transportChangedPaths = nil
	p.symbolFacts = nil
	p.symbolReferences = nil
	p.symbolsByPath = nil
	p.symbolOrder = nil
	p.symbolScratch = nil
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
	return p.backend.Close()
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
