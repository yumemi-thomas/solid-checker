package typefacts

import (
	"bytes"
	"cmp"
	"reflect"
	"slices"
	"sort"
)

// DiffFactTablesV3 produces a path/symbol keyed replacement delta. Both input
// tables are immutable to the operation.
func DiffFactTablesV3(previous, next FactTableV2) FactTableDeltaV3 {
	delta := FactTableDeltaV3{Generation: next.Generation}
	diffByKey(previous.Sources, next.Sources, func(value SourceDigestV2) string { return value.Path }, &delta.Sources, &delta.RemovedSourcePaths)
	diffByKey(previous.Files, next.Files, func(value FileFactV2) string { return value.Path }, &delta.Files, &delta.RemovedFilePaths)
	diffByKey(previous.Symbols, next.Symbols, func(value SymbolFactV2) string { return value.ID }, &delta.Symbols, &delta.RemovedSymbolIDs)

	previousEntities := entitiesByPath(previous.Entities)
	nextEntities := entitiesByPath(next.Entities)
	for path, entities := range nextEntities {
		if !reflect.DeepEqual(previousEntities[path], entities) {
			delta.EntityFiles = append(delta.EntityFiles, EntityFileV3{Path: path, Entities: entities})
		}
	}
	for path := range previousEntities {
		if _, ok := nextEntities[path]; !ok {
			delta.RemovedEntityPaths = append(delta.RemovedEntityPaths, path)
		}
	}
	sort.Slice(delta.EntityFiles, func(i, j int) bool { return delta.EntityFiles[i].Path < delta.EntityFiles[j].Path })
	sort.Strings(delta.RemovedEntityPaths)
	return delta
}

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
		func(left, right FileFact) bool { return reflect.DeepEqual(left, right) },
		fileFactV2,
		&delta.Files,
		&delta.RemovedFilePaths,
	)
	diffCanonicalRows(
		previous.symbolFactsSlice(),
		next.symbolFactsSlice(),
		func(value SymbolFact) string { return string(value.ID) },
		func(left, right SymbolFact) bool { return reflect.DeepEqual(left, right) },
		symbolFactV2,
		&delta.Symbols,
		&delta.RemovedSymbolIDs,
	)
	diffCanonicalEntityFiles(previous.Entities, next.Entities, &delta)
	return delta
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
		func(left, right FileFact) bool { return reflect.DeepEqual(left, right) },
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
		case !reflect.DeepEqual(oldEntities, newEntities):
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
		case rightOK && (left.AliasTarget != right.AliasTarget || !reflect.DeepEqual(left.Declarations, right.Declarations)):
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
		if reflect.DeepEqual(previousReferences, nextReferences) {
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
			if !reflect.DeepEqual(previous[left:leftEnd], next[right:rightEnd]) {
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

func diffByKey[T any](previous, next []T, key func(T) string, changed *[]T, removed *[]string) {
	old := make(map[string]T, len(previous))
	for _, value := range previous {
		old[key(value)] = value
	}
	present := make(map[string]struct{}, len(next))
	for _, value := range next {
		id := key(value)
		present[id] = struct{}{}
		if prior, ok := old[id]; !ok || !reflect.DeepEqual(prior, value) {
			*changed = append(*changed, value)
		}
	}
	for _, value := range previous {
		id := key(value)
		if _, ok := present[id]; !ok {
			*removed = append(*removed, id)
		}
	}
	sort.Strings(*removed)
}

func entitiesByPath(entities []EntityFactV2) map[string][]EntityFactV2 {
	result := make(map[string][]EntityFactV2)
	for _, entity := range entities {
		result[entity.Location.Path] = append(result[entity.Location.Path], entity)
	}
	return result
}

// ApplyFactTableDeltaV3 is the reference delta applicator used by
// differential tests and non-Rust protocol clients.
func ApplyFactTableDeltaV3(previous FactTableV2, delta FactTableDeltaV3) FactTableV2 {
	result := previous
	result.Generation = delta.Generation
	result.Sources = applyByKey(previous.Sources, delta.Sources, delta.RemovedSourcePaths, func(value SourceDigestV2) string { return value.Path })
	result.Files = applyByKey(previous.Files, delta.Files, delta.RemovedFilePaths, func(value FileFactV2) string { return value.Path })
	result.Symbols = applyByKey(previous.Symbols, delta.Symbols, delta.RemovedSymbolIDs, func(value SymbolFactV2) string { return value.ID })
	for _, replacement := range delta.SymbolReferenceFiles {
		index := slices.IndexFunc(result.Symbols, func(symbol SymbolFactV2) bool {
			return symbol.ID == replacement.ID
		})
		if index < 0 {
			continue
		}
		symbol := &result.Symbols[index]
		symbol.References = slices.DeleteFunc(symbol.References, func(reference LocationV2) bool {
			return reference.Path == replacement.Path
		})
		symbol.References = append(symbol.References, replacement.References...)
		slices.SortFunc(symbol.References, func(left, right LocationV2) int {
			return cmp.Or(
				cmp.Compare(left.Path, right.Path),
				cmp.Compare(left.StartByte, right.StartByte),
				cmp.Compare(left.EndByte, right.EndByte),
			)
		})
	}

	replaced := make(map[string]struct{}, len(delta.EntityFiles)+len(delta.RemovedEntityPaths))
	for _, file := range delta.EntityFiles {
		replaced[file.Path] = struct{}{}
	}
	for _, path := range delta.RemovedEntityPaths {
		replaced[path] = struct{}{}
	}
	result.Entities = result.Entities[:0]
	for _, entity := range previous.Entities {
		if _, ok := replaced[entity.Location.Path]; !ok {
			result.Entities = append(result.Entities, entity)
		}
	}
	for _, file := range delta.EntityFiles {
		result.Entities = append(result.Entities, file.Entities...)
	}
	slices.SortFunc(result.Entities, func(left, right EntityFactV2) int {
		return cmp.Or(
			cmp.Compare(left.Location.Path, right.Location.Path),
			cmp.Compare(left.Location.StartByte, right.Location.StartByte),
			cmp.Compare(left.Location.EndByte, right.Location.EndByte),
		)
	})
	return result
}

func applyByKey[T any](previous, changed []T, removed []string, key func(T) string) []T {
	replaced := make(map[string]struct{}, len(changed)+len(removed))
	for _, value := range changed {
		replaced[key(value)] = struct{}{}
	}
	for _, id := range removed {
		replaced[id] = struct{}{}
	}
	result := make([]T, 0, len(previous)+len(changed))
	for _, value := range previous {
		if _, ok := replaced[key(value)]; !ok {
			result = append(result, value)
		}
	}
	result = append(result, changed...)
	slices.SortFunc(result, func(left, right T) int { return cmp.Compare(key(left), key(right)) })
	return result
}
