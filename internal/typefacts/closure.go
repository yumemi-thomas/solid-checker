package typefacts

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"hash/maphash"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"sync"
	"time"
)

// ClosureBackend is the live project surface the demand closure materializes
// from. The tsgo adapter satisfies it.
type ClosureBackend interface {
	Project
	TypeDescriber
	CallDiscoverer
	BindingDiscoverer
	FunctionDiscoverer
	AsyncFunctionDiscoverer
}

// FileFacts bundles one file's bulk tables produced by a single traversal.
// Resolved, when populated, is index-aligned with Calls and carries each
// call's resolved target computed with the AST node in hand (nil where the
// call does not resolve), sparing consumers a position-keyed re-query.
type FileFacts struct {
	Calls          []SourceCall
	Bindings       []SourceBinding
	Functions      []SourceFunction
	AsyncFunctions []AsyncFunctionFact
	Resolved       []*Call
}

// FileFactsDiscoverer is an optional fused bulk capability: one AST pass
// yields all four per-file tables, replacing four independent walks. The
// closure prefers it when the backend offers it.
type FileFactsDiscoverer interface {
	SourceFileFacts(context.Context, string) (FileFacts, error)
}

// SemanticEntityDiscoverer is the production-shaped bulk capability. One
// checker lock and one node lookup per demand replace several position-keyed
// calls through the general Project interface.
type SemanticEntityDiscoverer interface {
	SemanticEntities(context.Context, []EntityDemand) ([]EntityFact, error)
}

// ScopedSemanticEntityDiscoverer additionally resolves a demand subset whose
// output must match a larger batch's semantics: the suppression set carries
// structural-accessor symbols from outside the subset, the descriptor seed
// carries type descriptors outside demands already computed (batch-wide
// first-wins dedup), and the second result reports the subset's
// structural-accessor symbol per demand. This is what makes per-file
// retention reproduce whole-batch output.
type ScopedSemanticEntityDiscoverer interface {
	SemanticEntitiesScoped(context.Context, []EntityDemand, map[SymbolID]struct{}, map[SymbolID]*TypeDescriptor) ([]EntityFact, []SymbolID, error)
}

// ReferenceBatchDiscoverer resolves reference lists under one backend lock.
// The map is keyed by the requested ID; an absent key has the same meaning as
// ErrNotFound from Project.References. This avoids tens of thousands of
// lock/canonicalization/copy round trips while preserving the Project API.
type ReferenceBatchDiscoverer interface {
	ReferencesBatch(context.Context, []SymbolID) (map[SymbolID][]Location, error)
}

// ReferenceChangeDiscoverer reports the exact canonical-symbol keys whose
// reference lists changed since the preceding generation. exact is false
// after a first build or broad invalidation, requiring callers to refresh
// every demanded list. Implementations must keep the delta stable for the
// generation so retries and cancelled analyses do not advance hidden state.
type ReferenceChangeDiscoverer interface {
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
	PrepareDuration  time.Duration    `json:"prepareDurationNs,omitempty"`
	Retention        ClosureRetention `json:"retention,omitzero"`
}

// ClosureMiss records one lookup the closed table could not answer. In
// fallback mode the miss is answered by the live backend and recorded; in
// strict mode the table's ErrNotFound reaches the caller.
type ClosureMiss struct {
	Kind     string   `json:"kind"`
	Location Location `json:"location,omitzero"`
	Symbol   SymbolID `json:"symbol,omitempty"`
}

const closureMissLimit = 10000

// ClosureProject serves the Project interface from a demand-closed FactTable
// materialized once per generation. The transitive fact expansion runs against
// the live backend during materialization; afterwards every query is a table
// lookup. An accepted Update discards the table; the next query rebuilds it
// for the new generation. The complete universe is never enumerated.
type ClosureProject struct {
	mu          sync.Mutex
	backend     ClosureBackend
	fused       FileFactsDiscoverer
	fallback    bool
	table       *FactTable
	demandTable *FactTable
	// previousDemandTable and transportChangedPaths describe the immediate
	// predecessor of the next semantic-demand table. The private transport
	// manifest is valid only for that base generation; protocol callers with
	// an older acknowledged snapshot automatically take the full diff path.
	previousDemandTable   *FactTable
	spareDemandTable      *FactTable
	transportChangedPaths map[string]struct{}
	demands               []EntityDemand
	closedSyms            map[SymbolID]struct{}
	fullTier              map[SymbolID]struct{}
	refDemand             map[SymbolID]struct{}
	// scanSeeds caches each file's byte-scan seed locations. Scans are pure
	// functions of one file's bytes, so entries survive generations and are
	// dropped only when that file itself changes.
	scanSeeds     map[string][]scanSeed
	compilerSeeds []Location
	stats         ClosureStats
	misses        []ClosureMiss
	generation    uint64
	closed        bool
	// retained carries per-file demand-closure contributions across
	// generations (ADR 0001): an accepted update drops exactly the affected
	// set, and a file whose demands are unchanged reuses its entity facts
	// instead of re-resolving them against the checker. lastSuppression is
	// the previous generation's structural-accessor union; when the union
	// changes, every file is recomputed so descriptor suppression keeps
	// whole-batch semantics.
	retained        map[string]*fileClosureContribution
	lastSuppression map[SymbolID]struct{}
	// symbolFacts memoizes the generation-independent half of durable
	// symbol closure: alias targets and declarations. Update removes facts
	// declared in affected files. symbolReferences separately retains lists
	// whose presence is known (including known-empty lists); because an edit
	// can add a reference to an otherwise unchanged symbol, they are reused
	// only when the backend supplies an exact changed-symbol delta.
	symbolFacts      map[SymbolID]SymbolFact
	symbolReferences map[SymbolID][]Location
	// symbolOrder is the preceding materialized table's canonical ID order.
	// Retained closure uses it as an ordering index so an ordinary edit only
	// sorts genuinely new symbols instead of re-sorting the complete table.
	symbolOrder   []SymbolID
	symbolScratch []SymbolFact
	demandSeed    maphash.Seed
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

// NewClosureProject wraps a live backend. With fallback enabled, lookups the
// closed table cannot answer are recorded and delegated to the backend, so
// results stay correct while closure coverage is being measured. With fallback
// disabled the table is authoritative and a gap surfaces as ErrNotFound.
func NewClosureProject(backend Project, fallback bool) (*ClosureProject, error) {
	full, ok := backend.(ClosureBackend)
	if !ok {
		return nil, errors.New("demand closure requires the bulk discovery and type-description capabilities")
	}
	fused, _ := backend.(FileFactsDiscoverer)
	return &ClosureProject{backend: full, fused: fused, fallback: fallback, scanSeeds: make(map[string][]scanSeed), generation: 1}, nil
}

// OpenClosure adapts an OpenProjectFunc so the engine sees closure-backed
// projects. observe, when non-nil, receives every constructed ClosureProject.
func OpenClosure(open OpenProjectFunc, fallback bool, observe func(*ClosureProject)) OpenProjectFunc {
	return func(ctx context.Context, configPath string) (Project, error) {
		inner, err := open(ctx, configPath)
		if err != nil {
			return nil, err
		}
		project, err := NewClosureProject(inner, fallback)
		if err != nil {
			_ = inner.Close()
			return nil, err
		}
		if observe != nil {
			observe(project)
		}
		return project, nil
	}
}

// demandHashSeed lazily initializes the process-local seed for demand-run
// hashing; retained state never crosses processes.
func (p *ClosureProject) demandHashSeed() maphash.Seed {
	if p.demandSeed == (maphash.Seed{}) {
		p.demandSeed = maphash.MakeSeed()
	}
	return p.demandSeed
}

// Stats returns the most recent generation's materialization cost.
func (p *ClosureProject) Stats() ClosureStats {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.stats
}

// Misses returns the recorded lookups the closed table could not answer.
func (p *ClosureProject) Misses() []ClosureMiss {
	p.mu.Lock()
	defer p.mu.Unlock()
	return append([]ClosureMiss(nil), p.misses...)
}

// Table materializes the current generation if needed and returns its
// table. Experiment support (reuse kill-test); the table is shared, not
// copied.
func (p *ClosureProject) Table(ctx context.Context) (*FactTable, error) {
	return p.ensureTable(ctx)
}

// DropTable discards the current table without an Update, so experiment
// code can drive the backend's generations directly. Experiment support.
func (p *ClosureProject) DropTable() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.table = nil
	p.demandTable = nil
	p.previousDemandTable = nil
	p.spareDemandTable = nil
	p.transportChangedPaths = nil
	p.demands = nil
	p.closedSyms = nil
	p.fullTier = nil
	p.refDemand = nil
	p.retained = nil
	p.lastSuppression = nil
	p.symbolFacts = nil
	p.symbolReferences = nil
	p.symbolScratch = nil
}

// EncodeTable returns the current generation's table in the deterministic
// wire shape via the supplied encoder, materializing it first if needed. The
// encoder indirection keeps the codec dependency out of the live path.
func (p *ClosureProject) EncodeTable(ctx context.Context, encode func(any) ([]byte, error)) ([]byte, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if err := p.ensureTableLocked(ctx); err != nil {
		return nil, err
	}
	return encode(*p.table)
}

func (p *ClosureProject) Update(ctx context.Context, changes []FileChange) (AffectedSet, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	affected, err := p.backend.Update(ctx, changes)
	if err != nil {
		return affected, err
	}
	for _, change := range changes {
		delete(p.scanSeeds, filepath.Clean(change.Path))
	}
	if len(affected.Files) != 0 {
		invalidPaths := make(map[string]struct{}, len(affected.Files)+len(changes))
		for _, path := range affected.Files {
			invalidPaths[filepath.Clean(path)] = struct{}{}
		}
		for _, change := range changes {
			invalidPaths[filepath.Clean(change.Path)] = struct{}{}
		}
		for id, fact := range p.symbolFacts {
			for _, declaration := range fact.Declarations {
				if _, invalid := invalidPaths[filepath.Clean(declaration.Location.Path)]; invalid {
					delete(p.symbolFacts, id)
					delete(p.symbolReferences, id)
					break
				}
			}
		}
		p.table = nil
		p.previousDemandTable = p.demandTable
		p.demandTable = nil
		p.transportChangedPaths = invalidPaths
		p.closedSyms = nil
		p.fullTier = nil
		// refDemand keys are generation-scoped; keeping them across
		// generations is unbounded growth, not information.
		p.refDemand = nil
		// Retained contributions survive except the affected set; a
		// departed file must be evicted now, not when next queried.
		for _, path := range affected.Files {
			delete(p.retained, filepath.Clean(path))
		}
		for _, change := range changes {
			delete(p.retained, filepath.Clean(change.Path))
		}
		p.generation++
	}
	return affected, nil
}

func (p *ClosureProject) Close() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.closed = true
	p.table = nil
	p.demandTable = nil
	p.previousDemandTable = nil
	p.spareDemandTable = nil
	p.transportChangedPaths = nil
	p.demands = nil
	p.closedSyms = nil
	p.symbolFacts = nil
	p.symbolReferences = nil
	p.symbolOrder = nil
	p.symbolScratch = nil
	return p.backend.Close()
}

// SourceFiles answers from the retained backend without materializing the
// demand closure: the table's source list is populated verbatim from the
// backend, and a table forced into existence here is discarded as soon as the
// first analyze request arrives with real compiler seeds.
func (p *ClosureProject) SourceFiles(ctx context.Context) ([]SourceFile, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, errors.New("closure project is closed")
	}
	return p.backend.SourceFiles(ctx)
}

func (p *ClosureProject) SymbolAt(ctx context.Context, location Location) (SymbolID, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return "", err
	}
	symbol, err := table.SymbolAt(ctx, location)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "symbol-at", Location: location})
		return p.backend.SymbolAt(ctx, location)
	}
	return symbol, err
}

func (p *ClosureProject) TypeAt(ctx context.Context, location Location) (TypeID, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return "", err
	}
	id, err := table.TypeAt(ctx, location)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "type-at", Location: location})
		return p.backend.TypeAt(ctx, location)
	}
	return id, err
}

func (p *ClosureProject) DescribeTypeAt(ctx context.Context, location Location) (TypeDescriptor, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return TypeDescriptor{}, err
	}
	descriptor, err := table.DescribeTypeAt(ctx, location)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "describe-type-at", Location: location})
		return p.backend.DescribeTypeAt(ctx, location)
	}
	return descriptor, err
}

func (p *ClosureProject) ResolvedCall(ctx context.Context, location Location) (Call, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return Call{}, err
	}
	call, err := table.ResolvedCall(ctx, location)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "resolved-call", Location: location})
		return p.backend.ResolvedCall(ctx, location)
	}
	return call, err
}

func (p *ClosureProject) ResolveAlias(ctx context.Context, id SymbolID) (SymbolID, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return "", err
	}
	target, err := table.ResolveAlias(ctx, id)
	if p.missedSymbol(err, id) {
		p.recordMiss(ClosureMiss{Kind: "resolve-alias", Symbol: id})
		return p.backend.ResolveAlias(ctx, id)
	}
	return target, err
}

func (p *ClosureProject) Declarations(ctx context.Context, id SymbolID) ([]Declaration, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	declarations, err := table.Declarations(ctx, id)
	if p.missedSymbol(err, id) {
		p.recordMiss(ClosureMiss{Kind: "declarations", Symbol: id})
		return p.backend.Declarations(ctx, id)
	}
	return declarations, err
}

func (p *ClosureProject) References(ctx context.Context, id SymbolID) ([]Location, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	p.mu.Lock()
	if p.refDemand == nil {
		p.refDemand = make(map[SymbolID]struct{})
	}
	p.refDemand[id] = struct{}{}
	p.mu.Unlock()
	// Chase alias edges to the canonical symbol, where reference lists are
	// stored (canonical reference storage). The chain is finite; the bound
	// only guards against a malformed table.
	current := id
	for range 64 {
		target, aliasErr := table.ResolveAlias(ctx, current)
		if aliasErr != nil {
			break
		}
		current = target
	}
	references, err := table.References(ctx, current)
	if p.missedSymbol(err, current) || (err != nil && current != id && p.missedSymbol(err, id)) {
		p.recordMiss(ClosureMiss{Kind: "references", Symbol: id})
		return p.backend.References(ctx, id)
	}
	return references, err
}

// ReferencesDemanded reports how many distinct symbols have had their
// reference lists queried since the project opened. The gap between this and
// the closed symbol count is the headroom for a tighter reference rule.
func (p *ClosureProject) ReferencesDemanded() int {
	p.mu.Lock()
	defer p.mu.Unlock()
	return len(p.refDemand)
}

// ReferenceDemand returns the demanded-references symbol set itself — the
// oracle any tier rule is measured against.
func (p *ClosureProject) ReferenceDemand() map[SymbolID]struct{} {
	p.mu.Lock()
	defer p.mu.Unlock()
	demand := make(map[SymbolID]struct{}, len(p.refDemand))
	for id := range p.refDemand {
		demand[id] = struct{}{}
	}
	return demand
}

func (p *ClosureProject) SourceCalls(ctx context.Context, path string) ([]SourceCall, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	calls, err := table.SourceCalls(ctx, path)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "source-calls", Location: Location{Path: path}})
		return p.backend.SourceCalls(ctx, path)
	}
	return calls, err
}

func (p *ClosureProject) SourceBindings(ctx context.Context, path string) ([]SourceBinding, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	bindings, err := table.SourceBindings(ctx, path)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "source-bindings", Location: Location{Path: path}})
		return p.backend.SourceBindings(ctx, path)
	}
	return bindings, err
}

func (p *ClosureProject) SourceFunctions(ctx context.Context, path string) ([]SourceFunction, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	functions, err := table.SourceFunctions(ctx, path)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "source-functions", Location: Location{Path: path}})
		return p.backend.SourceFunctions(ctx, path)
	}
	return functions, err
}

func (p *ClosureProject) SourceAsyncFunctions(ctx context.Context, path string) ([]AsyncFunctionFact, error) {
	table, err := p.ensureTable(ctx)
	if err != nil {
		return nil, err
	}
	asyncFunctions, err := table.SourceAsyncFunctions(ctx, path)
	if p.missed(err) {
		p.recordMiss(ClosureMiss{Kind: "source-async-functions", Location: Location{Path: path}})
		return p.backend.SourceAsyncFunctions(ctx, path)
	}
	return asyncFunctions, err
}

func (p *ClosureProject) missed(err error) bool {
	return p.fallback && err != nil && errors.Is(err, ErrNotFound)
}

// missedSymbol treats a symbol that IS in the closed table as answered: the
// table's ErrNotFound then means "not an alias" or "no declarations", exactly
// as the live backend would report, so falling back would only re-derive the
// same answer while polluting the miss log.
func (p *ClosureProject) missedSymbol(err error, id SymbolID) bool {
	if !p.missed(err) {
		return false
	}
	p.mu.Lock()
	_, closed := p.closedSyms[id]
	p.mu.Unlock()
	return !closed
}

func (p *ClosureProject) recordMiss(miss ClosureMiss) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if len(p.misses) < closureMissLimit {
		p.misses = append(p.misses, miss)
	}
}

func (p *ClosureProject) ensureTable(ctx context.Context) (*FactTable, error) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if err := p.ensureTableLocked(ctx); err != nil {
		return nil, err
	}
	return p.table, nil
}

func (p *ClosureProject) ensureTableLocked(ctx context.Context) error {
	if p.closed {
		return errors.New("closure project is closed")
	}
	if p.table != nil {
		return nil
	}
	started := time.Now()
	table, closedSyms, fullTier, err := materializeClosure(ctx, p.backend, p.fused, p.scanSeeds, p.compilerSeeds, p.generation)
	if err != nil {
		return err
	}
	p.table = table
	p.closedSyms = closedSyms
	p.fullTier = fullTier
	p.stats = ClosureStats{
		Generation:      p.generation,
		Files:           len(table.Files),
		Entities:        len(table.Entities),
		Symbols:         len(table.Symbols),
		FullTierSymbols: len(fullTier),
		BuildDuration:   time.Since(started),
	}
	return nil
}

// FullTier reports the current generation's tier assignment: symbols whose
// reference lists the expansion ruleset would include under TypeFacts v2.
func (p *ClosureProject) FullTier() map[SymbolID]struct{} {
	p.mu.Lock()
	defer p.mu.Unlock()
	tier := make(map[SymbolID]struct{}, len(p.fullTier))
	for id := range p.fullTier {
		tier[id] = struct{}{}
	}
	return tier
}

// ReferenceDemandOutsideFullTier returns demanded-references symbols the tier
// rule would have classified out — the tier rule's violation set, which must
// be empty before TypeFacts v2 freezes.
func (p *ClosureProject) ReferenceDemandOutsideFullTier() []SymbolID {
	p.mu.Lock()
	defer p.mu.Unlock()
	violations := make([]SymbolID, 0)
	for id := range p.refDemand {
		if _, ok := p.fullTier[id]; !ok {
			violations = append(violations, id)
		}
	}
	sort.Slice(violations, func(i, j int) bool { return violations[i] < violations[j] })
	return violations
}

// materializeClosure runs the transitive demand expansion against the live
// backend and returns the closed, prepared table. Seeds are the bulk per-file
// tables plus the byte-scan classes reactiveir derives query locations from;
// expansion follows alias, declaration, and reference edges to a fixed point
// over the symbols those seeds reach. Facts outside the closure are never
// computed — in particular, no full-universe enumeration happens here.
func materializeClosure(ctx context.Context, backend ClosureBackend, fused FileFactsDiscoverer, scanCache map[string][]scanSeed, compilerSeeds []Location, generation uint64) (*FactTable, map[SymbolID]struct{}, map[SymbolID]struct{}, error) {
	sources, err := backend.SourceFiles(ctx)
	if err != nil {
		return nil, nil, nil, err
	}
	sort.Slice(sources, func(i, j int) bool { return sources[i].Path < sources[j].Path })
	builder := &closureBuilder{
		backend:       backend,
		fused:         fused,
		scanCache:     scanCache,
		externalSeeds: len(compilerSeeds) != 0,
		entities:      make(map[Location]*EntityFact),
		symbolSeen:    make(map[SymbolID]struct{}),
		fullTier:      make(map[SymbolID]struct{}),
		descriptors:   make(map[SymbolID]*TypeDescriptor),
		cleanPaths:    make(map[string]string),
	}
	table := &FactTable{
		Schema:     TypeFactsSchemaVersion,
		Generation: generation,
		ProjectID:  "demand-closure",
		Sources:    sources,
	}
	for _, file := range sources {
		if err := ctx.Err(); err != nil {
			return nil, nil, nil, err
		}
		fileFact, err := builder.seedFile(ctx, file)
		if err != nil {
			return nil, nil, nil, err
		}
		table.Files = append(table.Files, fileFact)
	}
	for _, seed := range compilerSeeds {
		if err := builder.seedCompilerSpan(ctx, seed, sources); err != nil {
			return nil, nil, nil, err
		}
	}
	symbols, err := builder.closeSymbols(ctx)
	if err != nil {
		return nil, nil, nil, err
	}
	table.Symbols = symbols
	table.Entities = builder.sortedEntities()
	sort.Slice(table.Files, func(i, j int) bool { return table.Files[i].Path < table.Files[j].Path })
	table.Prepare()
	return table, builder.symbolSeen, builder.fullTier, nil
}

type closureBuilder struct {
	backend       ClosureBackend
	fused         FileFactsDiscoverer
	scanCache     map[string][]scanSeed
	externalSeeds bool
	cleanPaths    map[string]string
	entities      map[Location]*EntityFact
	symbolQueue   []SymbolID
	symbolSeen    map[SymbolID]struct{}
	// fullTier marks symbols reached through reference-demanding seed
	// classes (binding names, alias assignments, import specifiers,
	// function names and parameters, callback/JSX identifiers, exports).
	// Symbols reached only as classification targets stay out. Full tier
	// propagates along alias-target edges because reactiveir canonicalizes
	// before fanning out on references. This is TypeFacts v2's tier rule;
	// v1-shaped tables still carry references for every symbol, so today
	// the assignment is observational — the measure-demand-closure harness
	// verifies demanded references stay inside the full tier.
	fullTier               map[SymbolID]struct{}
	descriptors            map[SymbolID]*TypeDescriptor
	referencesOnlyFullTier bool
	cachedSymbolFacts      map[SymbolID]SymbolFact
	cachedReferences       map[SymbolID][]Location
	cachedSymbolOrder      []SymbolID
	symbolFactsBuffer      []SymbolFact
	symbolOrderBuffer      []SymbolFact
	closedReferences       map[SymbolID][]Location
	cachedSymbolHits       int
	recomputedSymbolFacts  int
	cachedReferenceHits    int
	recomputedReferences   int
	changedSymbolIDs       map[SymbolID]struct{}
	referenceChangesExact  bool
}

func (b *closureBuilder) seedCompilerSpan(ctx context.Context, span Location, sources []SourceFile) error {
	path := filepath.Clean(span.Path)
	for _, source := range sources {
		if filepath.Clean(source.Path) != path {
			continue
		}
		for _, location := range compilerSpanIdentifiers(path, source.Source, span) {
			if err := b.addSymbolEntity(ctx, location); err != nil {
				return err
			}
		}
		return nil
	}
	return fmt.Errorf("compiler span source is missing: %s", path)
}

// seedFile enumerates one file's bulk tables and the entities reactiveir will
// query within it.
func (b *closureBuilder) seedFile(ctx context.Context, file SourceFile) (FileFact, error) {
	path := b.cleanPath(file.Path)
	facts, err := b.fileFacts(ctx, path)
	if err != nil {
		return FileFact{}, err
	}
	calls, bindings, functions, asyncFunctions := facts.Calls, facts.Bindings, facts.Functions, facts.AsyncFunctions
	for _, binding := range bindings {
		for _, name := range binding.Names {
			if name.EndByte > name.StartByte {
				if err := b.addSymbolEntity(ctx, name); err != nil {
					return FileFact{}, err
				}
			}
		}
		b.enqueueSymbol(binding.Initializer.Target)
	}
	haveResolved := len(facts.Resolved) == len(calls)
	for index, call := range calls {
		var resolved *Call
		if haveResolved {
			resolved = facts.Resolved[index]
		}
		if err := b.seedCall(ctx, file.Source, call, resolved, haveResolved); err != nil {
			return FileFact{}, err
		}
	}
	for _, function := range functions {
		if err := b.addSymbolEntity(ctx, function.Name); err != nil {
			return FileFact{}, err
		}
		for _, parameter := range function.Parameters {
			if err := b.seedParameter(ctx, file.Source, parameter); err != nil {
				return FileFact{}, err
			}
		}
	}
	for _, async := range asyncFunctions {
		b.enqueueSymbol(async.Symbol)
		b.enqueueSymbol(async.Target)
	}
	if !b.externalSeeds {
		seeds, cached := b.scanCache[path]
		if !cached {
			seeds = scanSeedLocations(path, file.Source)
			b.scanCache[path] = seeds
		}
		for _, seed := range seeds {
			if err := b.addTieredSymbolEntity(ctx, seed.location, seed.full); err != nil {
				return FileFact{}, err
			}
		}
	}
	return FileFact{
		Path:           path,
		Calls:          calls,
		Bindings:       bindings,
		Functions:      functions,
		AsyncFunctions: asyncFunctions,
	}, nil
}

// fileFacts prefers the fused single-pass capability and falls back to the
// four standalone walks.
func (b *closureBuilder) fileFacts(ctx context.Context, path string) (FileFacts, error) {
	if b.fused != nil {
		facts, err := b.fused.SourceFileFacts(ctx, path)
		if err != nil {
			return FileFacts{}, fmt.Errorf("closure file facts for %s: %w", path, err)
		}
		return facts, nil
	}
	facts := FileFacts{}
	var err error
	if facts.Calls, err = b.backend.SourceCalls(ctx, path); err != nil {
		return FileFacts{}, fmt.Errorf("closure calls for %s: %w", path, err)
	}
	if facts.Bindings, err = b.backend.SourceBindings(ctx, path); err != nil {
		return FileFacts{}, fmt.Errorf("closure bindings for %s: %w", path, err)
	}
	if facts.Functions, err = b.backend.SourceFunctions(ctx, path); err != nil {
		return FileFacts{}, fmt.Errorf("closure functions for %s: %w", path, err)
	}
	if facts.AsyncFunctions, err = b.backend.SourceAsyncFunctions(ctx, path); err != nil {
		return FileFacts{}, fmt.Errorf("closure async functions for %s: %w", path, err)
	}
	return facts, nil
}

func (b *closureBuilder) cleanPath(path string) string {
	if cleaned, ok := b.cleanPaths[path]; ok {
		return cleaned
	}
	cleaned := filepath.Clean(path)
	b.cleanPaths[path] = cleaned
	return cleaned
}

// seedCall materializes the entities one call expression can be queried at:
// the resolved call on the whole-call and callee ranges, the callee symbol,
// the callee type descriptor at its dot-trimmed sub-range (mirroring
// reactiveir's describeTarget), and identifier arguments' symbols. resolved
// carries the fused enumerator's node-native resolution when haveResolved is
// set; otherwise the backend is queried by position.
func (b *closureBuilder) seedCall(ctx context.Context, source []byte, call SourceCall, resolved *Call, haveResolved bool) error {
	if !haveResolved {
		backendResolved, err := b.backend.ResolvedCall(ctx, call.Callee)
		switch {
		case err == nil:
			resolved = &backendResolved
		case !errors.Is(err, ErrNotFound):
			return err
		}
	}
	if resolved != nil {
		callEntity := b.entity(call.Location)
		callEntity.ResolvedCall = resolved
		calleeEntity := b.entity(call.Callee)
		calleeEntity.ResolvedCall = resolved
		b.enqueueSymbol(resolved.Target)
	}
	if err := b.addClassificationEntity(ctx, call.Callee); err != nil {
		return err
	}
	// Member callees are also queried at the property sub-range after the
	// last dot (isSolidCalleeAt), which resolves a different symbol than the
	// whole callee expression.
	if property := calleeProperty(source, call.Callee); property != call.Callee {
		if err := b.addClassificationEntity(ctx, property); err != nil {
			return err
		}
	}
	b.enqueueSymbol(call.Target)
	if err := b.seedDescriptor(ctx, source, call); err != nil {
		return err
	}
	for _, argument := range call.Arguments {
		start, end := closureTrimByteSpan(source, argument.StartByte, argument.EndByte)
		if start >= end || !closureIdentifierPattern.Match(source[start:end]) {
			continue
		}
		location := Location{Path: argument.Path, StartByte: start, EndByte: end}
		if err := b.addClassificationEntity(ctx, location); err != nil {
			return err
		}
		descriptor, err := b.backend.DescribeTypeAt(ctx, location)
		if err != nil {
			if !errors.Is(err, ErrNotFound) {
				return err
			}
			continue
		}
		b.entity(location).TypeDescriptor = &descriptor
	}
	return nil
}

// calleeProperty returns the sub-range after the last dot of a member callee,
// or the callee itself when it is a plain identifier.
func calleeProperty(source []byte, callee Location) Location {
	if callee.EndByte <= callee.StartByte || callee.EndByte > len(source) {
		return callee
	}
	if dot := bytes.LastIndexByte(source[callee.StartByte:callee.EndByte], '.'); dot >= 0 {
		return Location{Path: callee.Path, StartByte: callee.StartByte + dot + 1, EndByte: callee.EndByte}
	}
	return callee
}

func (b *closureBuilder) seedDescriptor(ctx context.Context, source []byte, call SourceCall) error {
	callee := call.Callee
	if callee.EndByte <= callee.StartByte || callee.EndByte > len(source) {
		return nil
	}
	location := calleeProperty(source, callee)
	var descriptor *TypeDescriptor
	if call.Target != "" {
		if cached, ok := b.descriptors[call.Target]; ok {
			descriptor = cached
		} else {
			described, err := b.backend.DescribeTypeAt(ctx, location)
			if err != nil {
				if !errors.Is(err, ErrNotFound) && ctx.Err() != nil {
					return err
				}
				b.descriptors[call.Target] = nil
			} else {
				descriptor = &described
				b.descriptors[call.Target] = descriptor
			}
		}
	} else {
		described, err := b.backend.DescribeTypeAt(ctx, location)
		if err != nil {
			if !errors.Is(err, ErrNotFound) && ctx.Err() != nil {
				return err
			}
		} else {
			descriptor = &described
		}
	}
	if descriptor != nil {
		entity := b.entity(call.Callee)
		entity.TypeDescriptor = descriptor
	}
	return nil
}

// seedParameter records the parameter's leading declared identifier, which is
// where reactiveir aims its parameter symbol queries. For destructured
// parameters it seeds every identifier token inside the binding pattern,
// since prop-destructure analysis queries those inner names.
func (b *closureBuilder) seedParameter(ctx context.Context, source []byte, parameter Location) error {
	if parameter.EndByte <= parameter.StartByte || parameter.EndByte > len(source) {
		return nil
	}
	start, end, ok := closureDeclarationName(source, parameter.StartByte, parameter.EndByte)
	if ok {
		return b.addSymbolEntity(ctx, Location{Path: parameter.Path, StartByte: start, EndByte: end})
	}
	patternStart := parameter.StartByte
	for patternStart < parameter.EndByte && (source[patternStart] == ' ' || source[patternStart] == '\t' || source[patternStart] == '\r' || source[patternStart] == '\n') {
		patternStart++
	}
	if patternStart >= parameter.EndByte {
		return nil
	}
	var closer byte
	switch source[patternStart] {
	case '{':
		closer = '}'
	case '[':
		closer = ']'
	default:
		return nil
	}
	patternEnd := closureMatchingBrace(source, patternStart, source[patternStart], closer)
	if patternEnd < 0 || patternEnd > parameter.EndByte {
		patternEnd = parameter.EndByte
	}
	for _, match := range closureIdentifierTokenPattern.FindAllIndex(source[patternStart:patternEnd], -1) {
		location := Location{Path: parameter.Path, StartByte: patternStart + match[0], EndByte: patternStart + match[1]}
		if err := b.addSymbolEntity(ctx, location); err != nil {
			return err
		}
	}
	return nil
}

// scanSeed is one byte-scan seed location and its tier.
type scanSeed struct {
	location Location
	full     bool
}

// scanSeedLocations mirrors the reactiveir byte scans whose matches feed
// SymbolAt: export declarations, export lists, named imports, alias
// assignments, callback identifiers, and JSX tags. It is a pure function of
// one file's bytes, so results are cached across generations until the file
// itself changes.
func scanSeedLocations(path string, source []byte) []scanSeed {
	seeds := make([]scanSeed, 0)
	fullSeed := func(start, end int) {
		seeds = append(seeds, scanSeed{location: Location{Path: path, StartByte: start, EndByte: end}, full: true})
	}
	classificationSeed := func(start, end int) {
		seeds = append(seeds, scanSeed{location: Location{Path: path, StartByte: start, EndByte: end}})
	}
	for _, match := range closureExportConstPattern.FindAllSubmatchIndex(source, -1) {
		end := closureStatementEnd(source, match[1])
		if end < 0 {
			continue
		}
		for _, span := range closureSplitArguments(source, match[1], end) {
			if start, finish, ok := closureDeclarationName(source, span.start, span.end); ok {
				fullSeed(start, finish)
			}
		}
	}
	for _, match := range closureExportClassPattern.FindAllSubmatchIndex(source, -1) {
		fullSeed(match[2], match[3])
	}
	for _, match := range closureExportListPattern.FindAllSubmatchIndex(source, -1) {
		for _, span := range closureSplitArguments(source, match[2], match[3]) {
			if specifier := closureImportSpecifierPattern.FindSubmatchIndex(source[span.start:span.end]); specifier != nil {
				fullSeed(span.start+specifier[2], span.start+specifier[3])
			}
		}
	}
	for _, match := range closureNamedImportPattern.FindAllSubmatchIndex(source, -1) {
		for _, span := range closureSplitArguments(source, match[2], match[3]) {
			specifier := closureImportSpecifierPattern.FindSubmatchIndex(source[span.start:span.end])
			if specifier == nil {
				continue
			}
			localStart, localEnd := span.start+specifier[2], span.start+specifier[3]
			if specifier[4] >= 0 {
				localStart, localEnd = span.start+specifier[4], span.start+specifier[5]
			}
			fullSeed(localStart, localEnd)
		}
	}
	for _, match := range closureAliasAssignmentPattern.FindAllSubmatchIndex(source, -1) {
		fullSeed(match[2], match[3])
		fullSeed(match[4], match[5])
	}
	for _, match := range closureConstBindingPattern.FindAllSubmatchIndex(source, -1) {
		for _, token := range closureIdentifierTokenPattern.FindAllIndex(source[match[2]:match[3]], -1) {
			fullSeed(match[2]+token[0], match[2]+token[1])
		}
	}
	// Callback identifiers and JSX tags resolve symbols for canonical
	// lookups (ModuleCalls, component identification) but never fan out on
	// references — classification tier, verified against refDemand.
	for _, match := range closureBracedIdentifierPattern.FindAllSubmatchIndex(source, -1) {
		classificationSeed(match[2], match[3])
	}
	for _, match := range closureJSXTagPattern.FindAllSubmatchIndex(source, -1) {
		classificationSeed(match[2], match[3])
	}
	return seeds
}

// addSymbolEntity resolves the symbol at a reference-demanding seed location
// and places it in the full tier. A location without a resolvable symbol is
// skipped, matching the tolerant reactiveir callers.
func (b *closureBuilder) addSymbolEntity(ctx context.Context, location Location) error {
	return b.addTieredSymbolEntity(ctx, location, true)
}

// addClassificationEntity resolves a symbol demanded only for classification
// (callees, call arguments); it joins the full tier only if another seed
// class also reaches it.
func (b *closureBuilder) addClassificationEntity(ctx context.Context, location Location) error {
	return b.addTieredSymbolEntity(ctx, location, false)
}

func (b *closureBuilder) addTieredSymbolEntity(ctx context.Context, location Location, full bool) error {
	if location.EndByte <= location.StartByte {
		return nil
	}
	if existing, ok := b.entities[location]; ok && existing.Symbol != "" {
		if full {
			b.fullTier[existing.Symbol] = struct{}{}
		}
		return nil
	}
	symbol, err := b.backend.SymbolAt(ctx, location)
	if err != nil {
		if errors.Is(err, ErrNotFound) {
			return nil
		}
		return err
	}
	entity := b.entity(location)
	entity.Symbol = symbol
	b.enqueueSymbol(symbol)
	if full {
		b.fullTier[symbol] = struct{}{}
	}
	return nil
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
	factByID := make(map[SymbolID]*SymbolFact, len(facts))
	for index := range facts {
		fact := &facts[index]
		factByID[fact.ID] = fact
	}
	if batched, ok := b.backend.(ReferenceBatchDiscoverer); ok {
		ids := make([]SymbolID, 0, len(facts))
		for index := range facts {
			fact := &facts[index]
			if fact.AliasTarget != "" {
				continue
			}
			if b.referencesOnlyFullTier {
				if _, demanded := b.fullTier[fact.ID]; !demanded {
					continue
				}
			}
			ids = append(ids, fact.ID)
		}

		refresh := ids
		if changes, supportsChanges := b.backend.(ReferenceChangeDiscoverer); supportsChanges && b.cachedReferences != nil {
			changedIDs, exact, err := changes.ChangedReferences(ctx)
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

		references, err := batched.ReferencesBatch(ctx, refresh)
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
				if b.referencesOnlyFullTier {
					if _, demanded := b.fullTier[id]; !demanded {
						delete(b.closedReferences, id)
					}
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
	} else {
		b.closedReferences = make(map[SymbolID][]Location)
		for index := range facts {
			fact := &facts[index]
			if fact.AliasTarget != "" {
				continue
			}
			if b.referencesOnlyFullTier {
				if _, demanded := b.fullTier[fact.ID]; !demanded {
					continue
				}
			}
			// Canonical reference storage: lists live on non-alias symbols.
			// The demand path also omits lists for classification-only symbols.
			references, err := b.backend.References(ctx, fact.ID)
			switch {
			case err == nil:
				fact.References = references
				b.closedReferences[fact.ID] = references
				b.recomputedReferences++
			case !errors.Is(err, ErrNotFound):
				return nil, err
			default:
				b.closedReferences[fact.ID] = nil
				b.recomputedReferences++
			}
		}
	}
	referencesDuration := time.Since(started)
	started = time.Now()
	var spare []SymbolFact
	facts, spare = orderSymbolFacts(facts, factByID, b.cachedSymbolOrder, b.symbolOrderBuffer[:0])
	b.symbolFactsBuffer = spare[:0]
	sortDuration := time.Since(started)
	if os.Getenv("SOLID_TYPEFACTS_TIMINGS") != "" {
		fmt.Fprintf(os.Stderr,
			"{\"typefactsCloseSymbols\":{\"factsNs\":%d,\"fullTierNs\":%d,\"referencesNs\":%d,\"sortNs\":%d,\"initialSymbols\":%d,\"symbols\":%d,\"fullTier\":%d,\"references\":%d}}\n",
			factsDuration, fullTierDuration, referencesDuration, sortDuration,
			initialSymbolCount, len(facts), len(b.fullTier), len(b.closedReferences))
	}
	return facts, nil
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

func (b *closureBuilder) sortedEntities() []EntityFact {
	entities := make([]EntityFact, 0, len(b.entities))
	for _, entity := range b.entities {
		entities = append(entities, *entity)
	}
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
	return entities
}
