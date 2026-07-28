package typefacts

import (
	"slices"
	"sort"
)

type wireTransitionCollectionOp uint64

const (
	wireTransitionUnchanged wireTransitionCollectionOp = 0
	wireTransitionReplace   wireTransitionCollectionOp = 1
	wireTransitionRemove    wireTransitionCollectionOp = 2
)

type wireTransitionPathOp struct {
	path string

	sourceOp wireTransitionCollectionOp
	source   SourceDigest

	entityOp wireTransitionCollectionOp
	entities []EntityFact

	fileOp wireTransitionCollectionOp
	file   FileFact
}

type wireTransitionSymbolOpTag uint64

const (
	wireTransitionReplaceSymbol        wireTransitionSymbolOpTag = 0
	wireTransitionRemoveSymbol         wireTransitionSymbolOpTag = 1
	wireTransitionReplaceReferencePath wireTransitionSymbolOpTag = 2
)

type wireTransitionSymbolOp struct {
	id   SymbolID
	tag  wireTransitionSymbolOpTag
	fact SymbolFact

	referencePath string
	references    []Location
}

// symbolFactCursor walks immutable symbol chunks in canonical ID order and
// exposes chunk boundaries so fallback diffs can skip shared chunks whole.
type symbolFactCursor struct {
	chunks [][]SymbolFact
	chunk  int
	row    int
}

func newSymbolFactCursor(table FactTable) symbolFactCursor {
	if table.symbols != nil {
		return symbolFactCursor{chunks: table.symbols.chunks}
	}
	if len(table.Symbols) == 0 {
		return symbolFactCursor{}
	}
	return symbolFactCursor{chunks: [][]SymbolFact{table.Symbols}}
}

func (c *symbolFactCursor) valid() bool { return c.chunk < len(c.chunks) }

func (c *symbolFactCursor) fact() SymbolFact { return c.chunks[c.chunk][c.row] }

func (c *symbolFactCursor) next() {
	c.row++
	if c.row == len(c.chunks[c.chunk]) {
		c.chunk++
		c.row = 0
	}
}

func (e *wireTransitionEncoder) planFull(target *FactTable) {
	cursor := newWireTransitionTablePathCursor(target)
	for cursor.valid() {
		e.paths = append(e.paths, cursor.path())
		cursor.next()
	}
}

func (e *wireTransitionEncoder) planDelta(base, target *FactTable) {
	manifest := target.transport
	if manifest != nil &&
		manifest.exact &&
		base.stateID != 0 &&
		manifest.baseGeneration == base.Generation &&
		manifest.baseStateID == base.stateID {
		e.planExactPathDelta(base, target, manifest)
		e.planExactSymbolDelta(base, target, manifest)
		return
	}
	e.planFallbackPathDelta(base, target)
	e.planFallbackSymbolDelta(base, target)
}

func (e *wireTransitionEncoder) planExactPathDelta(
	base, target *FactTable,
	manifest *factTableTransportChanges,
) {
	for path := range manifest.sourcePaths {
		e.paths = append(e.paths, path)
	}
	for path := range manifest.entityPaths {
		e.paths = append(e.paths, path)
	}
	for path := range manifest.filePaths {
		e.paths = append(e.paths, path)
	}
	e.paths = sortUniqueWireTransitionPaths(e.paths)
	for _, path := range e.paths {
		_, sourceCandidate := manifest.sourcePaths[path]
		_, entityCandidate := manifest.entityPaths[path]
		_, fileCandidate := manifest.filePaths[path]
		if operation, changed := wireTransitionPathDifference(
			base,
			target,
			path,
			sourceCandidate,
			entityCandidate,
			fileCandidate,
		); changed {
			e.pathOps = append(e.pathOps, operation)
		}
	}
}

func (e *wireTransitionEncoder) planFallbackPathDelta(base, target *FactTable) {
	previous := newWireTransitionTablePathCursor(base)
	next := newWireTransitionTablePathCursor(target)
	for previous.valid() && next.valid() {
		previousRows, nextRows := previous.rows(), next.rows()
		switch {
		case previousRows.path < nextRows.path:
			if operation, changed := wireTransitionPathRowsDifference(
				previousRows,
				wireTransitionPathRows{path: previousRows.path},
				true,
				true,
				true,
			); changed {
				e.pathOps = append(e.pathOps, operation)
			}
			previous.next()
		case nextRows.path < previousRows.path:
			if operation, changed := wireTransitionPathRowsDifference(
				wireTransitionPathRows{path: nextRows.path},
				nextRows,
				true,
				true,
				true,
			); changed {
				e.pathOps = append(e.pathOps, operation)
			}
			next.next()
		default:
			if operation, changed := wireTransitionPathRowsDifference(
				previousRows,
				nextRows,
				true,
				true,
				true,
			); changed {
				e.pathOps = append(e.pathOps, operation)
			}
			previous.next()
			next.next()
		}
	}
	for previous.valid() {
		rows := previous.rows()
		if operation, changed := wireTransitionPathRowsDifference(
			rows,
			wireTransitionPathRows{path: rows.path},
			true,
			true,
			true,
		); changed {
			e.pathOps = append(e.pathOps, operation)
		}
		previous.next()
	}
	for next.valid() {
		rows := next.rows()
		if operation, changed := wireTransitionPathRowsDifference(
			wireTransitionPathRows{path: rows.path},
			rows,
			true,
			true,
			true,
		); changed {
			e.pathOps = append(e.pathOps, operation)
		}
		next.next()
	}
}

func wireTransitionPathDifference(
	base, target *FactTable,
	path string,
	sourceCandidate, entityCandidate, fileCandidate bool,
) (wireTransitionPathOp, bool) {
	previous := wireTransitionPathRows{path: path}
	if base != nil {
		previous = wireTransitionPathRowsAt(base, path)
	}
	next := wireTransitionPathRowsAt(target, path)
	return wireTransitionPathRowsDifference(
		previous,
		next,
		sourceCandidate,
		entityCandidate,
		fileCandidate,
	)
}

type wireTransitionPathRows struct {
	path string

	source    SourceDigest
	hasSource bool

	entities []EntityFact

	file    FileFact
	hasFile bool
}

func wireTransitionPathRowsAt(table *FactTable, path string) wireTransitionPathRows {
	rows := wireTransitionPathRows{path: path}
	rows.source, rows.hasSource = wireTransitionSource(table.wireSourceDigests(), path)
	rows.entities = canonicalEntityPath(table.Entities, path)
	rows.file, rows.hasFile = canonicalFileFact(table.Files, path)
	return rows
}

func wireTransitionPathRowsDifference(
	previous, next wireTransitionPathRows,
	sourceCandidate, entityCandidate, fileCandidate bool,
) (wireTransitionPathOp, bool) {
	operation := wireTransitionPathOp{path: next.path}
	if sourceCandidate {
		switch {
		case previous.hasSource && !next.hasSource:
			operation.sourceOp = wireTransitionRemove
		case next.hasSource &&
			(!previous.hasSource || previous.source.SHA256 != next.source.SHA256):
			operation.sourceOp = wireTransitionReplace
			operation.source = next.source
		}
	}
	if entityCandidate {
		switch {
		case len(previous.entities) != 0 && len(next.entities) == 0:
			operation.entityOp = wireTransitionRemove
		case len(next.entities) != 0 && !entityFactsEqual(previous.entities, next.entities):
			operation.entityOp = wireTransitionReplace
			operation.entities = next.entities
		}
	}
	if fileCandidate {
		switch {
		case previous.hasFile && !next.hasFile:
			operation.fileOp = wireTransitionRemove
		case next.hasFile && (!previous.hasFile || !fileFactEqual(previous.file, next.file)):
			operation.fileOp = wireTransitionReplace
			operation.file = next.file
		}
	}
	changed := operation.sourceOp != wireTransitionUnchanged ||
		operation.entityOp != wireTransitionUnchanged ||
		operation.fileOp != wireTransitionUnchanged
	return operation, changed
}

func (e *wireTransitionEncoder) planExactSymbolDelta(
	base, target *FactTable,
	manifest *factTableTransportChanges,
) {
	for id := range manifest.symbolIDs {
		e.symbolIDs = append(e.symbolIDs, id)
	}
	slices.Sort(e.symbolIDs)

	for _, id := range e.symbolIDs {
		previous, previousOK := base.canonicalSymbol(id)
		next, nextOK := target.canonicalSymbol(id)
		switch {
		case previousOK && !nextOK:
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id: id, tag: wireTransitionRemoveSymbol,
			})
		case nextOK && !previousOK:
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id: id, tag: wireTransitionReplaceSymbol, fact: next,
			})
		case nextOK &&
			(previous.AliasTarget != next.AliasTarget ||
				!slices.Equal(previous.Declarations, next.Declarations)):
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id: id, tag: wireTransitionReplaceSymbol, fact: next,
			})
		case nextOK:
			e.planExactReferenceDelta(id, previous.References, next.References)
		}
	}
}

// planExactReferenceDelta merge-walks the two canonical path runs for one
// candidate symbol. This derives the exact operations directly instead of
// multiplying every candidate symbol by every changed source path.
func (e *wireTransitionEncoder) planExactReferenceDelta(
	id SymbolID,
	previous, next []Location,
) {
	for len(previous) != 0 || len(next) != 0 {
		var path string
		switch {
		case len(previous) == 0:
			path = next[0].Path
		case len(next) == 0:
			path = previous[0].Path
		case previous[0].Path < next[0].Path:
			path = previous[0].Path
		default:
			path = next[0].Path
		}
		previousEnd := referencePathEnd(previous, path)
		nextEnd := referencePathEnd(next, path)
		previousRun := previous[:previousEnd]
		nextRun := next[:nextEnd]
		if !locationsEqual(previousRun, nextRun) {
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id:            id,
				tag:           wireTransitionReplaceReferencePath,
				referencePath: path,
				references:    nextRun,
			})
		}
		previous = previous[previousEnd:]
		next = next[nextEnd:]
	}
}

func referencePathEnd(references []Location, path string) int {
	end := 0
	for end < len(references) && references[end].Path == path {
		end++
	}
	return end
}

func (e *wireTransitionEncoder) planFallbackSymbolDelta(base, target *FactTable) {
	left := newSymbolFactCursor(*base)
	right := newSymbolFactCursor(*target)
	for left.valid() && right.valid() {
		if left.row == 0 && right.row == 0 {
			leftChunk, rightChunk := left.chunks[left.chunk], right.chunks[right.chunk]
			if len(leftChunk) != 0 &&
				len(leftChunk) == len(rightChunk) &&
				&leftChunk[0] == &rightChunk[0] {
				left.chunk++
				right.chunk++
				continue
			}
		}
		previous, next := left.fact(), right.fact()
		switch {
		case previous.ID < next.ID:
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id: previous.ID, tag: wireTransitionRemoveSymbol,
			})
			left.next()
		case next.ID < previous.ID:
			e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
				id: next.ID, tag: wireTransitionReplaceSymbol, fact: next,
			})
			right.next()
		default:
			if !symbolFactEqual(previous, next) {
				e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
					id: next.ID, tag: wireTransitionReplaceSymbol, fact: next,
				})
			}
			left.next()
			right.next()
		}
	}
	for ; left.valid(); left.next() {
		fact := left.fact()
		e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
			id: fact.ID, tag: wireTransitionRemoveSymbol,
		})
	}
	for ; right.valid(); right.next() {
		fact := right.fact()
		e.symbolOps = append(e.symbolOps, wireTransitionSymbolOp{
			id: fact.ID, tag: wireTransitionReplaceSymbol, fact: fact,
		})
	}
}

// wireTransitionTablePathCursor merge-walks one table's three independently
// canonical path streams without flattening or sorting them.
type wireTransitionTablePathCursor struct {
	table *FactTable

	source int
	entity int
	file   int
}

func newWireTransitionTablePathCursor(table *FactTable) wireTransitionTablePathCursor {
	return wireTransitionTablePathCursor{table: table}
}

func (c *wireTransitionTablePathCursor) valid() bool {
	sources := c.table.wireSourceDigests()
	return c.table != nil &&
		(c.source < len(sources) ||
			c.entity < len(c.table.Entities) ||
			c.file < len(c.table.Files))
}

func (c *wireTransitionTablePathCursor) path() string {
	var path string
	set := false
	consider := func(candidate string) {
		if !set || candidate < path {
			path = candidate
			set = true
		}
	}
	sources := c.table.wireSourceDigests()
	if c.source < len(sources) {
		consider(sources[c.source].Path)
	}
	if c.entity < len(c.table.Entities) {
		consider(c.table.Entities[c.entity].Location.Path)
	}
	if c.file < len(c.table.Files) {
		consider(c.table.Files[c.file].Path)
	}
	return path
}

func (c *wireTransitionTablePathCursor) rows() wireTransitionPathRows {
	path := c.path()
	rows := wireTransitionPathRows{path: path}
	sources := c.table.wireSourceDigests()
	if c.source < len(sources) && sources[c.source].Path == path {
		rows.source = sources[c.source]
		rows.hasSource = true
	}
	if c.entity < len(c.table.Entities) &&
		c.table.Entities[c.entity].Location.Path == path {
		end := entityPathEnd(c.table.Entities, c.entity)
		rows.entities = c.table.Entities[c.entity:end]
	}
	if c.file < len(c.table.Files) && c.table.Files[c.file].Path == path {
		rows.file = c.table.Files[c.file]
		rows.hasFile = true
	}
	return rows
}

func (c *wireTransitionTablePathCursor) next() {
	path := c.path()
	sources := c.table.wireSourceDigests()
	if c.source < len(sources) && sources[c.source].Path == path {
		c.source++
	}
	if c.entity < len(c.table.Entities) &&
		c.table.Entities[c.entity].Location.Path == path {
		c.entity = entityPathEnd(c.table.Entities, c.entity)
	}
	if c.file < len(c.table.Files) && c.table.Files[c.file].Path == path {
		c.file++
	}
}

func sortUniqueWireTransitionPaths(paths []string) []string {
	sort.Strings(paths)
	write := 0
	for _, path := range paths {
		if write != 0 && paths[write-1] == path {
			continue
		}
		paths[write] = path
		write++
	}
	clear(paths[write:])
	return paths[:write]
}

func wireTransitionSource(sources []SourceDigest, path string) (SourceDigest, bool) {
	index := sort.Search(len(sources), func(index int) bool {
		return sources[index].Path >= path
	})
	if index == len(sources) || sources[index].Path != path {
		return SourceDigest{}, false
	}
	return sources[index], true
}

func canonicalEntityPath(entities []EntityFact, path string) []EntityFact {
	start := sort.Search(len(entities), func(index int) bool {
		return entities[index].Location.Path >= path
	})
	if start == len(entities) || entities[start].Location.Path != path {
		return nil
	}
	return entities[start:entityPathEnd(entities, start)]
}

func entityPathEnd(entities []EntityFact, start int) int {
	path := entities[start].Location.Path
	end := start + 1
	for end < len(entities) && entities[end].Location.Path == path {
		end++
	}
	return end
}
