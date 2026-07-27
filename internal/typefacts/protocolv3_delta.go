package typefacts

import (
	"bytes"
	"slices"
	"sort"
)

// DiffFactTablesV3FromInternal compares two canonical transport-only tables
// directly and converts only changed rows to their wire representation. This
// avoids allocating a complete FactTableV2 merely to emit a small v3 delta.
func DiffFactTablesV3FromInternal(previous, next FactTable, generation uint64) FactTableDeltaV3 {
	if manifest := next.transport; manifest != nil && manifest.exact && manifest.baseGeneration == previous.Generation {
		return diffFactTablesV3FromManifest(previous, next, generation, manifest)
	}
	delta := FactTableDeltaV3{Generation: generation}
	diffCanonicalRows(
		previous.Sources,
		next.Sources,
		func(value SourceFile) string { return value.Path },
		func(left, right SourceFile) bool { return bytes.Equal(left.Source, right.Source) },
		sourceDigestV2,
		&delta.Sources,
		&delta.RemovedSourcePaths,
	)
	diffCanonicalRows(
		previous.Files,
		next.Files,
		func(value FileFact) string { return value.Path },
		fileFactEqual,
		fileFactV2,
		&delta.Files,
		&delta.RemovedFilePaths,
	)
	diffCanonicalSymbolStores(previous, next, &delta)
	diffCanonicalEntityFiles(previous.Entities, next.Entities, &delta)
	return delta
}

// symbolFactCursor walks a table's symbol rows chunk by chunk in canonical ID
// order, exposing chunk boundaries so the diff can skip shared chunks whole.
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

// diffCanonicalSymbolStores merge-walks two symbol stores without flattening
// either. A chunk the successor shares with its predecessor is the same slice
// by construction (symbolFactStore.Patch), so when both cursors stand at the
// start of one it is skipped whole — for an ordinary edit that reduces the
// fallback diff's symbol pass to the few chunks that were actually rebuilt.
func diffCanonicalSymbolStores(previous, next FactTable, delta *FactTableDeltaV3) {
	left := newSymbolFactCursor(previous)
	right := newSymbolFactCursor(next)
	for left.valid() && right.valid() {
		if left.row == 0 && right.row == 0 {
			leftChunk, rightChunk := left.chunks[left.chunk], right.chunks[right.chunk]
			if len(leftChunk) != 0 && len(leftChunk) == len(rightChunk) && &leftChunk[0] == &rightChunk[0] {
				left.chunk++
				right.chunk++
				continue
			}
		}
		leftFact, rightFact := left.fact(), right.fact()
		switch {
		case leftFact.ID < rightFact.ID:
			delta.RemovedSymbolIDs = append(delta.RemovedSymbolIDs, string(leftFact.ID))
			left.next()
		case rightFact.ID < leftFact.ID:
			delta.Symbols = append(delta.Symbols, symbolFactV2(rightFact))
			right.next()
		default:
			if !symbolFactEqual(leftFact, rightFact) {
				delta.Symbols = append(delta.Symbols, symbolFactV2(rightFact))
			}
			left.next()
			right.next()
		}
	}
	for ; left.valid(); left.next() {
		delta.RemovedSymbolIDs = append(delta.RemovedSymbolIDs, string(left.fact().ID))
	}
	for ; right.valid(); right.next() {
		delta.Symbols = append(delta.Symbols, symbolFactV2(right.fact()))
	}
}

func diffFactTablesV3FromManifest(previous, next FactTable, generation uint64, manifest *factTableTransportChanges) FactTableDeltaV3 {
	delta := FactTableDeltaV3{Generation: generation}
	diffCanonicalCandidates(
		previous.Sources,
		next.Sources,
		sortedStringKeys(manifest.sourcePaths),
		func(value SourceFile) string { return value.Path },
		func(left, right SourceFile) bool { return bytes.Equal(left.Source, right.Source) },
		sourceDigestV2,
		&delta.Sources,
		&delta.RemovedSourcePaths,
	)
	diffCanonicalCandidates(
		previous.Files,
		next.Files,
		sortedStringKeys(manifest.filePaths),
		func(value FileFact) string { return value.Path },
		fileFactEqual,
		fileFactV2,
		&delta.Files,
		&delta.RemovedFilePaths,
	)
	symbolKeys := make([]string, 0, len(manifest.symbolIDs))
	for id := range manifest.symbolIDs {
		symbolKeys = append(symbolKeys, string(id))
	}
	sort.Strings(symbolKeys)
	diffCanonicalSymbolCandidates(
		previous,
		next,
		symbolKeys,
		sortedStringKeys(manifest.sourcePaths),
		&delta.Symbols,
		&delta.SymbolReferenceFiles,
		&delta.RemovedSymbolIDs,
	)
	for _, path := range sortedStringKeys(manifest.entityPaths) {
		oldEntities := canonicalEntityPath(previous.Entities, path)
		newEntities := canonicalEntityPath(next.Entities, path)
		switch {
		case len(newEntities) == 0 && len(oldEntities) != 0:
			delta.RemovedEntityPaths = append(delta.RemovedEntityPaths, path)
		case !entityFactsEqual(oldEntities, newEntities):
			delta.EntityFiles = append(delta.EntityFiles, EntityFileV3{
				Path: path, Entities: convertEntityFactsV2(newEntities),
			})
		}
	}
	return delta
}

func diffCanonicalSymbolCandidates(
	previous, next FactTable,
	keys []string,
	referencePaths []string,
	changed *[]SymbolFactV2,
	referenceFiles *[]SymbolReferenceFileV3,
	removed *[]string,
) {
	for _, candidate := range keys {
		id := SymbolID(candidate)
		left, leftOK := previous.canonicalSymbol(id)
		right, rightOK := next.canonicalSymbol(id)
		switch {
		case leftOK && !rightOK:
			*removed = append(*removed, candidate)
		case rightOK && !leftOK:
			*changed = append(*changed, symbolFactV2(right))
		case rightOK && (left.AliasTarget != right.AliasTarget || !slices.Equal(left.Declarations, right.Declarations)):
			*changed = append(*changed, symbolFactV2(right))
		case rightOK:
			diffSymbolReferenceFiles(candidate, left.References, right.References, referencePaths, referenceFiles)
		}
	}
}

func diffSymbolReferenceFiles(
	id string,
	previous, next []Location,
	paths []string,
	changed *[]SymbolReferenceFileV3,
) {
	for _, path := range paths {
		previousReferences := canonicalReferencesForPath(previous, path)
		nextReferences := canonicalReferencesForPath(next, path)
		if locationsEqual(previousReferences, nextReferences) {
			continue
		}
		references := make([]LocationV2, 0, len(nextReferences))
		for _, reference := range nextReferences {
			references = append(references, locationV2(reference))
		}
		*changed = append(*changed, SymbolReferenceFileV3{
			ID:         id,
			Path:       path,
			References: references,
		})
	}
}

func canonicalReferencesForPath(references []Location, path string) []Location {
	start := sort.Search(len(references), func(index int) bool {
		return references[index].Path >= path
	})
	end := start
	for end < len(references) && references[end].Path == path {
		end++
	}
	return references[start:end]
}

func sortedStringKeys(values map[string]struct{}) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func diffCanonicalCandidates[Raw, Wire any](
	previous, next []Raw,
	keys []string,
	key func(Raw) string,
	equal func(Raw, Raw) bool,
	convert func(Raw) Wire,
	changed *[]Wire,
	removed *[]string,
) {
	for _, candidate := range keys {
		left, leftOK := canonicalRow(previous, candidate, key)
		right, rightOK := canonicalRow(next, candidate, key)
		switch {
		case leftOK && !rightOK:
			*removed = append(*removed, candidate)
		case rightOK && (!leftOK || !equal(left, right)):
			*changed = append(*changed, convert(right))
		}
	}
}

func canonicalRow[T any](values []T, candidate string, key func(T) string) (T, bool) {
	index := sort.Search(len(values), func(index int) bool { return key(values[index]) >= candidate })
	if index == len(values) || key(values[index]) != candidate {
		var zero T
		return zero, false
	}
	return values[index], true
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

func diffCanonicalRows[Raw, Wire any](
	previous, next []Raw,
	key func(Raw) string,
	equal func(Raw, Raw) bool,
	convert func(Raw) Wire,
	changed *[]Wire,
	removed *[]string,
) {
	left, right := 0, 0
	for left < len(previous) && right < len(next) {
		leftKey, rightKey := key(previous[left]), key(next[right])
		switch {
		case leftKey < rightKey:
			*removed = append(*removed, leftKey)
			left++
		case rightKey < leftKey:
			*changed = append(*changed, convert(next[right]))
			right++
		default:
			if !equal(previous[left], next[right]) {
				*changed = append(*changed, convert(next[right]))
			}
			left++
			right++
		}
	}
	for ; left < len(previous); left++ {
		*removed = append(*removed, key(previous[left]))
	}
	for ; right < len(next); right++ {
		*changed = append(*changed, convert(next[right]))
	}
}

func diffCanonicalEntityFiles(previous, next []EntityFact, delta *FactTableDeltaV3) {
	left, right := 0, 0
	for left < len(previous) || right < len(next) {
		var leftPath, rightPath string
		if left < len(previous) {
			leftPath = previous[left].Location.Path
		}
		if right < len(next) {
			rightPath = next[right].Location.Path
		}
		switch {
		case right >= len(next) || left < len(previous) && leftPath < rightPath:
			delta.RemovedEntityPaths = append(delta.RemovedEntityPaths, leftPath)
			left = entityPathEnd(previous, left)
		case left >= len(previous) || rightPath < leftPath:
			end := entityPathEnd(next, right)
			delta.EntityFiles = append(delta.EntityFiles, EntityFileV3{
				Path:     rightPath,
				Entities: convertEntityFactsV2(next[right:end]),
			})
			right = end
		default:
			leftEnd, rightEnd := entityPathEnd(previous, left), entityPathEnd(next, right)
			if !entityFactsEqual(previous[left:leftEnd], next[right:rightEnd]) {
				delta.EntityFiles = append(delta.EntityFiles, EntityFileV3{
					Path:     rightPath,
					Entities: convertEntityFactsV2(next[right:rightEnd]),
				})
			}
			left, right = leftEnd, rightEnd
		}
	}
}

func entityPathEnd(entities []EntityFact, start int) int {
	path := entities[start].Location.Path
	end := start + 1
	for end < len(entities) && entities[end].Location.Path == path {
		end++
	}
	return end
}

func convertEntityFactsV2(entities []EntityFact) []EntityFactV2 {
	result := make([]EntityFactV2, 0, len(entities))
	for _, entity := range entities {
		result = append(result, entityFactV2(entity))
	}
	return result
}

// Empty reports whether applying the delta changes any collection. Generation
// advancement is deliberately excluded: an empty delta can still advance a
// client from one generation to the next.
func (delta FactTableDeltaV3) Empty() bool {
	return len(delta.Sources) == 0 &&
		len(delta.RemovedSourcePaths) == 0 &&
		len(delta.EntityFiles) == 0 &&
		len(delta.RemovedEntityPaths) == 0 &&
		len(delta.Symbols) == 0 &&
		len(delta.RemovedSymbolIDs) == 0 &&
		len(delta.SymbolReferenceFiles) == 0 &&
		len(delta.Files) == 0 &&
		len(delta.RemovedFilePaths) == 0
}
