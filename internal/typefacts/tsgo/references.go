package tsgo

import (
	"context"
	"fmt"
	"path/filepath"
	"sort"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
)

// referenceIndex owns the generation-scoped merged index, its reusable
// per-file contributions, and the exact invalidation delta exposed to
// retained closure consumers.
type referenceIndex struct {
	// merged is nil until the first reference query of a generation.
	merged map[typefacts.SymbolID][]typefacts.Location
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

// invalidate retains safe contributions after a single-file incremental
// update and prepares an exact lazy refresh when the affected set is small.
func (r *referenceIndex) invalidate(
	program *compiler.Program,
	affected []string,
	retained func(string) bool,
) {
	// A second update before pending fragments are rescanned cannot safely
	// compose an exact symbol delta. Large affected sets also favor a full
	// merged-index rebuild.
	incremental := r.merged != nil && len(affected) <= 64 && len(r.refreshPaths) == 0
	refreshPaths := make(map[string]struct{}, len(affected))
	changedSymbols := make(map[typefacts.SymbolID]struct{})
	for path, entry := range r.files {
		// A non-durable entry holds generation-scoped counter IDs no later
		// generation can resolve; fail closed and re-scan.
		if retained(path) && entry.durable {
			continue
		}
		if incremental {
			for id := range entry.refs {
				changedSymbols[id] = struct{}{}
				locations := r.merged[id]
				kept := locations[:0]
				for _, location := range locations {
					if filepath.Clean(location.Path) != path {
						kept = append(kept, location)
					}
				}
				if len(kept) == 0 {
					delete(r.merged, id)
				} else {
					r.merged[id] = kept
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
	r.refreshPaths = nil
	r.changedSymbols = nil
	r.deltaExact = false
}

func (r *referenceIndex) reset() {
	r.merged = nil
	r.refreshPaths = nil
	r.changedSymbols = nil
	r.deltaExact = false
	r.files = nil
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
	target, ok := p.symbolFor(id)
	if !ok {
		// A durable ID whose declaration no longer re-resolves fails closed
		// here, before any retained index entry could answer for it.
		return nil, fmt.Errorf("%w: symbol %s", typefacts.ErrNotFound, id)
	}
	canonical := p.idFor(p.canonicalSymbol(target))

	p.referenceIndex.ensure(p)
	return append([]typefacts.Location(nil), p.referenceIndex.merged[canonical]...), nil
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
	p.referenceIndex.ensure(p)
	result := make(map[typefacts.SymbolID][]typefacts.Location, len(ids))
	for _, id := range ids {
		target, ok := p.symbolFor(id)
		if !ok {
			continue
		}
		canonical := p.idFor(p.canonicalSymbol(target))
		result[id] = append([]typefacts.Location(nil), p.referenceIndex.merged[canonical]...)
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
		for id, locations := range entry.refs {
			touched[id] = struct{}{}
			if r.deltaExact {
				r.changedSymbols[id] = struct{}{}
			}
			r.merged[id] = append(r.merged[id], locations...)
		}
	}
	for id := range touched {
		locations := r.merged[id]
		sort.Slice(locations, func(i, j int) bool {
			if locations[i].Path != locations[j].Path {
				return locations[i].Path < locations[j].Path
			}
			if locations[i].StartByte != locations[j].StartByte {
				return locations[i].StartByte < locations[j].StartByte
			}
			return locations[i].EndByte < locations[j].EndByte
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
	refs    map[typefacts.SymbolID][]typefacts.Location
	durable bool
}

// build merges the per-file contributions into the current
// generation's reference index, scanning only files without a retained
// entry (Update already evicted the affected set and departed files).
// Merging in path order, with each per-file group already in byte order,
// preserves the References ordering contract (path, then start byte)
// without a global sort.
func (r *referenceIndex) build(p *project) map[typefacts.SymbolID][]typefacts.Location {
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
		if _, ok := r.files[path]; !ok {
			r.files[path] = r.scan(p, path, sourceFile)
		}
	}
	sort.Strings(paths)
	references := make(map[typefacts.SymbolID][]typefacts.Location)
	for _, path := range paths {
		for id, locations := range r.files[path].refs {
			references[id] = append(references[id], locations...)
		}
	}
	return references
}

// scan resolves every non-declaration identifier in one file.
func (r *referenceIndex) scan(p *project, path string, sourceFile *ast.SourceFile) *fileReferences {
	entry := &fileReferences{refs: make(map[typefacts.SymbolID][]typefacts.Location), durable: true}
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsIdentifier(node) && !ast.IsDeclarationNameOrImportPropertyName(node) {
			if symbol := p.checker.GetSymbolAtLocation(node); symbol != nil {
				id := p.idFor(p.canonicalSymbol(symbol))
				if !durableSymbolID(id) {
					entry.durable = false
				}
				entry.refs[id] = append(entry.refs[id], typefacts.Location{
					Path:      path,
					StartByte: scanner.SkipTrivia(sourceFile.Text(), node.Pos()),
					EndByte:   node.End(),
				})
			}
		}
		node.ForEachChild(visit)
		return false
	}
	for _, statement := range sourceFile.Statements.Nodes {
		visit(statement)
	}
	for id, locations := range entry.refs {
		sort.Slice(locations, func(i, j int) bool { return locations[i].StartByte < locations[j].StartByte })
		entry.refs[id] = locations
	}
	return entry
}
