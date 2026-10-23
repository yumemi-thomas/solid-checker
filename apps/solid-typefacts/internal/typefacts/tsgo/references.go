package tsgo

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// referenceIndex owns the generation-scoped merged index, its reusable
// per-file contributions, and the exact invalidation delta exposed to
// retained closure consumers.
type referenceIndex struct {
	// merged is nil until the first reference query of a generation.
	merged map[typefacts.SymbolID][]indexedReference
	spaces map[typefacts.SymbolID]uint8
	paths  []string
	// used records the canonical symbols whose merged rows escaped into a
	// retained fact. ReleaseAnalysisState prunes every other merged bucket;
	// per-file symbol evidence remains sufficient for exact invalidation.
	used map[typefacts.SymbolID]struct{}
	// complete is non-nil once ReleaseAnalysisState pruned merged buckets: it
	// names the only buckets still known whole. A bucket absent from it may be
	// missing or hold just the rows an incremental refresh added, so a query
	// for it rebuilds the merged index first.
	complete map[typefacts.SymbolID]struct{}
	// refreshPaths are affected files removed from an already-materialized
	// merged index. They are rescanned lazily at the established reference
	// closure point so symbol counter minting retains its order.
	refreshPaths map[string]struct{}
	// changedSymbols is the exact union removed from and added to refreshed
	// contributions. deltaExact distinguishes known-empty from unavailable.
	changedSymbols map[typefacts.SymbolID]struct{}
	deltaExact     bool
	// files carries durable per-file contributions across generations.
	files map[string]*fileReferences
}

// invalidate retains safe contributions and records exact symbol evidence for
// every discarded file. Small updates patch the merged index in place; broad
// updates rebuild it, but no longer discard the exact changed-symbol set.
func (r *referenceIndex) invalidate(
	program *compiler.Program,
	affected []string,
	retained func(string) bool,
) {
	r.spaces = nil
	composing := r.deltaExact && (r.merged == nil || len(r.refreshPaths) != 0)
	hadExactBase := r.merged != nil || composing
	// Large affected sets still favor a full merged-index rebuild. Exactness
	// and the physical update strategy are independent decisions.
	incremental := r.merged != nil && len(affected) <= 64 && !composing
	refreshPaths := make(map[string]struct{}, len(affected))
	changedSymbols := r.changedSymbols
	if !composing {
		changedSymbols = make(map[typefacts.SymbolID]struct{})
	}
	for path, entry := range r.files {
		// A non-durable entry holds generation-scoped counter IDs no later
		// generation can resolve; fail closed and re-scan.
		if retained(path) && entry.durable {
			continue
		}
		if hadExactBase {
			for _, group := range entry.refs {
				changedSymbols[group.id] = struct{}{}
			}
		}
		if incremental {
			pathIndex := sort.SearchStrings(r.paths, path)
			for _, group := range entry.refs {
				references := r.merged[group.id]
				kept := references[:0]
				for _, reference := range references {
					if int(reference.path) != pathIndex {
						kept = append(kept, reference)
					}
				}
				if len(kept) == 0 {
					delete(r.merged, group.id)
				} else {
					r.merged[group.id] = kept
				}
			}
			if sourceFile := program.GetSourceFile(path); sourceFile != nil && !sourceFile.IsDeclarationFile {
				refreshPaths[path] = struct{}{}
			}
		}
		delete(r.files, path)
	}
	if incremental {
		r.refreshPaths = refreshPaths
		r.changedSymbols = changedSymbols
		r.deltaExact = true
		return
	}
	r.merged = nil
	r.spaces = nil
	r.paths = nil
	r.refreshPaths = nil
	r.changedSymbols = changedSymbols
	r.deltaExact = hadExactBase
}

func (r *referenceIndex) reset() {
	r.merged = nil
	r.spaces = nil
	r.refreshPaths = nil
	r.changedSymbols = nil
	r.deltaExact = false
	r.files = nil
	r.paths = nil
	r.used = nil
	r.complete = nil
}

func (p *project) References(ctx context.Context, id typefacts.SymbolID) ([]typefacts.Location, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
	}
	target, ok := p.symbolFor(id)
	if !ok {
		// A durable ID whose declaration no longer re-resolves fails closed
		// here, before any retained index entry could answer for it.
		return nil, fmt.Errorf("%w: symbol %s", typefacts.ErrNotFound, id)
	}
	canonical := p.idFor(p.canonicalSymbol(target))

	p.referenceIndex.ensure(p)
	return p.referenceIndex.query(p, canonical), nil
}

// ReferencesBatch is the closure-oriented counterpart of References. TS-Go's
// reference index is already retained as per-file fragments; resolving a
// batch here merges those fragments once and amortizes the project lock,
// durable-ID lookup, alias canonicalization, and slice allocation.
func (p *project) ReferencesBatch(ctx context.Context, ids []typefacts.SymbolID) (map[typefacts.SymbolID][]typefacts.Location, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, ErrClosed
	}
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, err
	}
	p.referenceIndex.ensure(p)
	result := make(map[typefacts.SymbolID][]typefacts.Location, len(ids))
	for _, id := range ids {
		target, ok := p.symbolFor(id)
		if !ok {
			continue
		}
		canonical := p.idFor(p.canonicalSymbol(target))
		result[id] = p.referenceIndex.query(p, canonical)
	}
	return result, nil
}

// ChangedReferences exposes the retained reference index's generation-stable
// invalidation set. It never consumes the delta: cancelled analyses and
// retries in the same generation observe the same answer.
func (p *project) ChangedReferences(ctx context.Context) ([]typefacts.SymbolID, bool, error) {
	if err := ctx.Err(); err != nil {
		return nil, false, err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return nil, false, ErrClosed
	}
	if err := p.ensureCheckerLocked(ctx); err != nil {
		return nil, false, err
	}
	p.referenceIndex.ensure(p)
	if !p.referenceIndex.deltaExact {
		return nil, false, nil
	}
	ids := make([]typefacts.SymbolID, 0, len(p.referenceIndex.changedSymbols))
	for id := range p.referenceIndex.changedSymbols {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	return ids, true, nil
}

func (r *referenceIndex) ensure(p *project) {
	if r.merged == nil {
		r.merged = r.build(p)
		r.refreshPaths = nil
		r.complete = nil
		return
	}
	if len(r.refreshPaths) == 0 {
		return
	}
	paths := make([]string, 0, len(r.refreshPaths))
	for path := range r.refreshPaths {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	touched := make(map[typefacts.SymbolID]struct{})
	for _, path := range paths {
		sourceFile := p.program.GetSourceFile(path)
		if sourceFile == nil || sourceFile.IsDeclarationFile {
			continue
		}
		entry := r.scan(p, path, sourceFile)
		r.files[path] = entry
		pathIndex := sort.SearchStrings(r.paths, path)
		if pathIndex == len(r.paths) || r.paths[pathIndex] != path {
			// Incremental refresh is only selected when program membership is
			// unchanged, so every refreshed path belongs to the established
			// compact path table.
			continue
		}
		for groupIndex := range entry.refs {
			group := &entry.refs[groupIndex]
			touched[group.id] = struct{}{}
			if r.deltaExact {
				r.changedSymbols[group.id] = struct{}{}
			}
			references := r.merged[group.id]
			for _, span := range group.spans {
				references = append(references, indexedReference{
					path:  uint32(pathIndex),
					start: span.start,
					end:   span.end,
				})
			}
			r.merged[group.id] = references
			group.spans = nil
		}
		entry.merged = true
	}
	for id := range touched {
		references := r.merged[id]
		sort.Slice(references, func(i, j int) bool {
			if references[i].path != references[j].path {
				return references[i].path < references[j].path
			}
			if references[i].start != references[j].start {
				return references[i].start < references[j].start
			}
			return references[i].end < references[j].end
		})
	}
	r.refreshPaths = nil
}

// fileReferences is one file's contribution to the reference index: every
// resolvable non-declaration identifier in the file, grouped by the durable
// SymbolID of its alias-canonicalized symbol, each group in ascending byte
// order. durable reports whether every grouped ID is durable; an entry with
// generation-scoped counter IDs cannot outlive its generation.
type fileReferences struct {
	refs    []fileReferenceGroup
	spaces  []fileReferenceSpace
	durable bool
	// merged reports that the spans were moved into a merged index and
	// cleared. Such an entry still carries exact symbol evidence but cannot
	// rebuild a dropped index; build rescans it.
	merged bool
}

// Per-file contributions deliberately omit the file path: referenceIndex.files
// already owns it once. AST offsets are compiler positions and fit in 32 bits,
// cutting each retained reference from a Location's string header plus two ints
// to one compact eight-byte span.
type fileReferenceSpan struct {
	start int32
	end   int32
}

type indexedReference struct {
	path  uint32
	start int32
	end   int32
}

type fileReferenceGroup struct {
	id    typefacts.SymbolID
	spans []fileReferenceSpan
}

type fileReferenceSpace struct {
	id   typefacts.SymbolID
	bits uint8
}

// build merges the per-file contributions into the current
// generation's reference index, scanning only files without a retained
// entry (Update already evicted the affected set and departed files).
// Merging in path order, with each per-file group already in byte order,
// preserves the References ordering contract (path, then start byte)
// without a global sort.
func (r *referenceIndex) build(p *project) map[typefacts.SymbolID][]indexedReference {
	if r.files == nil {
		r.files = make(map[string]*fileReferences)
	}
	sourceFiles := p.program.SourceFiles()
	paths := make([]string, 0, len(sourceFiles))
	for _, sourceFile := range sourceFiles {
		if sourceFile.IsDeclarationFile {
			continue
		}
		path := filepath.Clean(sourceFile.FileName())
		paths = append(paths, path)
		if prior, ok := r.files[path]; !ok {
			entry := r.scan(p, path, sourceFile)
			r.files[path] = entry
			if r.deltaExact {
				for _, group := range entry.refs {
					r.changedSymbols[group.id] = struct{}{}
				}
			}
		} else if prior.merged {
			// Retained across a broad invalidation: its text and durable
			// identities are unchanged, so this is not a changed symbol.
			r.files[path] = r.scan(p, path, sourceFile)
		}
	}
	sort.Strings(paths)
	r.paths = paths
	references := make(map[typefacts.SymbolID][]indexedReference)
	spaces := make(map[typefacts.SymbolID]uint8)
	for pathIndex, path := range paths {
		for groupIndex := range r.files[path].refs {
			group := &r.files[path].refs[groupIndex]
			indexed := references[group.id]
			for _, span := range group.spans {
				indexed = append(indexed, indexedReference{
					path:  uint32(pathIndex),
					start: span.start,
					end:   span.end,
				})
			}
			references[group.id] = indexed
			group.spans = nil
		}
		r.files[path].merged = true
		for _, space := range r.files[path].spaces {
			spaces[space.id] |= space.bits
		}
	}
	r.spaces = spaces
	return references
}

func (r *referenceIndex) locations(id typefacts.SymbolID) []typefacts.Location {
	references := r.merged[id]
	if len(references) == 0 {
		return nil
	}
	locations := make([]typefacts.Location, len(references))
	for index, reference := range references {
		locations[index] = typefacts.Location{
			Path:      r.paths[reference.path],
			StartByte: int(reference.start),
			EndByte:   int(reference.end),
		}
	}
	return locations
}

// query completes the bucket for id, marks it used, and returns its locations.
// The merged index must already be ensured.
func (r *referenceIndex) query(p *project, id typefacts.SymbolID) []typefacts.Location {
	if r.complete != nil {
		if _, whole := r.complete[id]; !whole {
			// Pruned by ReleaseAnalysisState; retained contributions rescan
			// into a complete index.
			r.merged = r.build(p)
			r.refreshPaths = nil
			r.complete = nil
		}
	}
	r.markUsed(id)
	return r.locations(id)
}

func (r *referenceIndex) markUsed(id typefacts.SymbolID) {
	if r.used == nil {
		r.used = make(map[typefacts.SymbolID]struct{})
	}
	r.used[id] = struct{}{}
}

// releaseAnalysisState drops the whole index. Merging moves each file's spans
// into the merged buckets, so a pruned bucket cannot be restored from the
// per-file entries; keeping the entries would answer a later query with an
// empty list. Nothing survives an update (ADR 0364), so the only later reader
// is the same generation, and it rebuilds the index from the program.
func (r *referenceIndex) releaseAnalysisState() {
	r.merged = nil
	r.complete = nil
	r.spaces = nil
	r.refreshPaths = nil
	r.files = nil
	r.paths = nil
	r.used = nil
}

// scan resolves every non-declaration identifier in one file.
func (r *referenceIndex) scan(p *project, path string, sourceFile *ast.SourceFile) *fileReferences {
	references := make(map[typefacts.SymbolID][]fileReferenceSpan)
	spaces := make(map[typefacts.SymbolID]uint8)
	var unordered map[typefacts.SymbolID]struct{}
	durable := true
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsIdentifier(node) {
			var symbol *ast.Symbol
			if !ast.IsDeclarationNameOrImportPropertyName(node) {
				symbol = p.checker.GetSymbolAtLocation(node)
			} else if parent := node.Parent; parent != nil && ast.IsShorthandPropertyAssignment(parent) && parent.Name() == node {
				// `{ value }` names the property it declares and reads the
				// value binding at once. The name is a declaration name, so
				// without this the read of `value` is in no reference list,
				// and a census of its references misses the object holding it.
				symbol = p.checker.GetShorthandAssignmentValueSymbol(parent)
			}
			if symbol != nil {
				referenceID := p.idFor(symbol)
				id := p.idFor(p.canonicalSymbol(symbol))
				if !durableSymbolID(id) || !durableSymbolID(referenceID) {
					durable = false
				}
				spans := references[id]
				start := int32(scanner.SkipTrivia(sourceFile.Text(), node.Pos()))
				// Parsed children normally arrive in source order. Preserve the
				// ordering contract without sorting every bucket, but detect and
				// repair any non-lexical compiler traversal defensively.
				if len(spans) != 0 && spans[len(spans)-1].start > start {
					if unordered == nil {
						unordered = make(map[typefacts.SymbolID]struct{})
					}
					unordered[id] = struct{}{}
				}
				references[id] = append(spans, fileReferenceSpan{
					start: start,
					end:   int32(node.End()),
				})
				if isTypeSpaceReference(node) {
					spaces[referenceID] |= 2
				} else {
					spaces[referenceID] |= 1
				}
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	entry := &fileReferences{
		refs:    make([]fileReferenceGroup, 0, len(references)),
		spaces:  make([]fileReferenceSpace, 0, len(spaces)),
		durable: durable,
	}
	for id, spans := range references {
		if _, needsSort := unordered[id]; needsSort {
			sort.Slice(spans, func(i, j int) bool { return spans[i].start < spans[j].start })
		}
		entry.refs = append(entry.refs, fileReferenceGroup{id: id, spans: spans})
	}
	sort.Slice(entry.refs, func(i, j int) bool { return entry.refs[i].id < entry.refs[j].id })
	for id, bits := range spaces {
		entry.spaces = append(entry.spaces, fileReferenceSpace{id: id, bits: bits})
	}
	sort.Slice(entry.spaces, func(i, j int) bool { return entry.spaces[i].id < entry.spaces[j].id })
	return entry
}

// IsPartOfTypeNode handles the right side of a QualifiedName, but an imported
// namespace is commonly its leftmost identifier. Classify the complete
// qualified name so its enclosing TypeReference (or TypeQuery) determines the
// reference space for every identifier in the chain.
func isTypeSpaceReference(node *ast.Node) bool {
	for node.Parent != nil && ast.IsQualifiedName(node.Parent) {
		node = node.Parent
	}
	return ast.IsPartOfTypeNode(node)
}

func referenceSpaceFromBits(bits uint8) typefacts.ReferenceSpace {
	switch bits {
	case 1:
		return typefacts.ReferenceSpaceValue
	case 2:
		return typefacts.ReferenceSpaceType
	case 3:
		return typefacts.ReferenceSpaceBoth
	default:
		return typefacts.ReferenceSpaceNeither
	}
}

func (r *referenceIndex) spaceFor(p *project, id typefacts.SymbolID) typefacts.ReferenceSpace {
	r.ensure(p)
	if r.spaces == nil {
		spaces := make(map[typefacts.SymbolID]uint8)
		for _, entry := range r.files {
			for _, space := range entry.spaces {
				spaces[space.id] |= space.bits
			}
		}
		r.spaces = spaces
	}
	if bits := r.spaces[id]; bits != 0 {
		return referenceSpaceFromBits(bits)
	}
	return typefacts.ReferenceSpaceNeither
}
