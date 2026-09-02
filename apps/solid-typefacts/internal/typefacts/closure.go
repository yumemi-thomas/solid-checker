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
	ReleaseAnalysisState()
}

// symbolEvidenceBackend is the production TSGo oracle seam used by the active
// protocol. One
// call holds the checker once for the whole batch. The fallback in
// resolveSymbolEvidence keeps compiler-independent test adapters small.
type symbolEvidenceBackend interface {
	SymbolEvidence(context.Context, []SymbolQueryV6) ([]SymbolFact, error)
}

// ErrModuleGraphUnavailable is returned when the backend cannot report the
// program's resolved module graph. There is deliberately no approximation: an
// inventory that omits files the analysis read, presented as the complete list,
// is the exact defect an attested closure exists to remove, so a backend that
// cannot answer says so instead.
var ErrModuleGraphUnavailable = errors.New("type facts backend reports no resolved module graph")

// ErrInvocationFactsUnavailable is returned when a backend cannot provide the
// exact selected-signature and census capability. Falling back to weaker entity
// facts would turn an absent proof into a false closed transcript.
var ErrInvocationFactsUnavailable = errors.New("type facts backend reports no invocation transcripts")

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
	// The canonical symbol store itself memoizes alias targets and
	// declarations; keeping a second SymbolID -> SymbolFact map duplicated
	// every row. symbolReferences separately retains lists whose presence is
	// known (including known-empty lists).
	symbolReferences map[SymbolID][]Location
	// symbolsByPath indexes durable canonical facts by cleaned declaring path, so Update
	// evicts an affected set by looking up its paths instead of scanning every
	// retained fact. Every canonical-store change goes through
	// indexSymbolFact / unindexSymbolFact to keep the index in sync.
	symbolsByPath map[string][]SymbolID
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
	return newDemandClosure(backend, trace)
}

func newDemandClosure(backend Project, trace Trace) (*DemandClosure, error) {
	full, ok := backend.(ClosureBackend)
	if !ok {
		return nil, errors.New("demand closure requires the demand-run semantic, async, and reference-batch capabilities")
	}
	return &DemandClosure{
		backend: full, trace: trace, generation: 1,
	}, nil
}

// releaseTransportRows is called only after the packed transition owns the
// expanded entity/file rows. The closure keeps only compact retained
// contributions needed to produce the next transition.
func (p *DemandClosure) releaseTransportRows() {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.table == nil {
		return
	}
	for _, contribution := range p.retained.byPath {
		contribution.releaseTransportRows()
	}
	p.table.Entities = nil
	clear(p.table.entityRuns)
	p.table.entityRuns = p.table.entityRuns[:0]
	p.table.Files = nil
	p.table.sourceDigests = nil
}

func (p *DemandClosure) forceFullMaterialization() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.resetAnalysisStateLocked()
}

// indexSymbolFact records a retained fact under each clean declaring path.
// Callers pair it with every durable insert into the canonical symbol store.
func (p *DemandClosure) indexSymbolFact(fact SymbolFact) {
	if p.symbolsByPath == nil {
		p.symbolsByPath = make(map[string][]SymbolID)
	}
	for _, declaration := range fact.Declarations {
		path := declaration.Location.Path
		ids := p.symbolsByPath[path]
		present := false
		for _, existing := range ids {
			if existing == fact.ID {
				present = true
				break
			}
		}
		if !present {
			p.symbolsByPath[path] = append(ids, fact.ID)
		}
	}
}

// unindexSymbolFact removes a retained fact from the path index. Callers must
// pair it with every canonical deletion, passing the fact being removed.
func (p *DemandClosure) unindexSymbolFact(fact SymbolFact) {
	for _, declaration := range fact.Declarations {
		path := declaration.Location.Path
		ids := p.symbolsByPath[path]
		for index, existing := range ids {
			if existing != fact.ID {
				continue
			}
			copy(ids[index:], ids[index+1:])
			ids[len(ids)-1] = ""
			ids = ids[:len(ids)-1]
			break
		}
		if len(ids) == 0 {
			delete(p.symbolsByPath, path)
		} else {
			p.symbolsByPath[path] = ids
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
		for len(p.symbolsByPath[path]) != 0 {
			ids := p.symbolsByPath[path]
			id := ids[len(ids)-1]
			if p.invalidatedSymbols == nil {
				p.invalidatedSymbols = make(map[SymbolID]struct{})
			}
			p.invalidatedSymbols[id] = struct{}{}
			if fact, retained := p.table.canonicalSymbol(id); retained {
				p.unindexSymbolFact(fact)
			} else {
				p.symbolsByPath[path] = ids[:len(ids)-1]
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

// ModuleGraph answers from the retained backend without materializing the
// demand closure, exactly as SourceFiles does. The module graph is a property
// of the accepted program, not of any demand set, so asking for it neither
// builds a fact table nor disturbs one that exists.
func (p *DemandClosure) ModuleGraph(
	ctx context.Context,
	demand ModuleInventoryDemand,
) (ModuleInventory, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return ModuleInventory{}, errors.New("closure project is closed")
	}
	provider, ok := p.backend.(ModuleGraphProvider)
	if !ok {
		return ModuleInventory{}, ErrModuleGraphUnavailable
	}
	return provider.ModuleGraph(ctx, demand)
}

// InvocationTranscripts answers from the retained compiler program without
// materializing or mutating the retained entity demand table.
func (p *DemandClosure) InvocationTranscripts(
	ctx context.Context,
	demands []InvocationDemand,
) (InvocationAnswer, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return InvocationAnswer{}, errors.New("closure project is closed")
	}
	provider, ok := p.backend.(InvocationAnalyzer)
	if !ok {
		return InvocationAnswer{}, ErrInvocationFactsUnavailable
	}
	return provider.InvocationTranscripts(ctx, demands)
}

// ExportValueTranscripts answers exact expression values from the retained
// compiler program without mutating the editor demand table.
func (p *DemandClosure) ExportValueTranscripts(
	ctx context.Context,
	demands []ExportValueDemand,
) (ExportValueAnswer, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return ExportValueAnswer{}, errors.New("closure project is closed")
	}
	provider, ok := p.backend.(ExportValueAnalyzer)
	if !ok {
		return ExportValueAnswer{}, ErrInvocationFactsUnavailable
	}
	return provider.ExportValueTranscripts(ctx, demands)
}

// resolveSymbolEvidence answers one Rust-owned closure worklist batch. It does
// not retain the returned rows: ownership crosses the process seam immediately.
func (p *DemandClosure) resolveSymbolEvidence(
	ctx context.Context,
	queries []SymbolQueryV6,
) ([]SymbolFact, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, errors.New("closure project is closed")
	}
	if batched, ok := p.backend.(symbolEvidenceBackend); ok {
		return batched.SymbolEvidence(ctx, queries)
	}
	facts := make([]SymbolFact, len(queries))
	referenceIDs := make([]SymbolID, 0, len(queries))
	for index, query := range queries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		facts[index].ID = query.ID
		if !query.ReferencesOnly {
			if target, err := p.backend.ResolveAlias(ctx, query.ID); err == nil {
				facts[index].AliasTarget = target
			} else if !errors.Is(err, ErrNotFound) {
				return nil, err
			}
			if declarations, err := p.backend.Declarations(ctx, query.ID); err == nil {
				facts[index].Declarations = declarations
			} else if !errors.Is(err, ErrNotFound) {
				return nil, err
			}
		}
		if query.References {
			referenceIDs = append(referenceIDs, query.ID)
		}
	}
	references, err := p.backend.ReferencesBatch(ctx, referenceIDs)
	if err != nil {
		return nil, err
	}
	for index, query := range queries {
		if query.References {
			facts[index].References = references[query.ID]
		}
	}
	return facts, nil
}

func (p *DemandClosure) releaseBackendAnalysisState() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.backend.ReleaseAnalysisState()
}

func (p *DemandClosure) changedReferences(ctx context.Context) ([]SymbolID, bool, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.backend.ChangedReferences(ctx)
}
