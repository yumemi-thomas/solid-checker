package typefacts

import (
	"cmp"
	"reflect"
	"slices"
	"sort"
	"testing"
)

// A Go model of the delta applicator that the Rust client actually runs
// (apply_table_delta in crates/typefacts/src/session.rs). It exists so the
// producer's differ can be checked by a diff-then-apply round trip without a
// live client; it ships in no binary.
//
// The model deliberately enforces the same invariants the Rust applier
// enforces, and fails the test when one is broken, so the two cannot drift
// into disagreeing about the same delta. The authoritative check that Rust
// applies these deltas identically is the cross-language fixture in
// protocolv3_delta_golden_test.go.

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

func applyFactTableDeltaV3(t *testing.T, previous FactTableV2, delta FactTableDeltaV3) FactTableV2 {
	t.Helper()
	result := previous
	result.Generation = delta.Generation
	result.Sources = applyByKey(previous.Sources, delta.Sources, delta.RemovedSourcePaths, func(value SourceDigestV2) string { return value.Path })
	result.Files = applyByKey(previous.Files, delta.Files, delta.RemovedFilePaths, func(value FileFactV2) string { return value.Path })
	result.Symbols = applyByKey(previous.Symbols, delta.Symbols, delta.RemovedSymbolIDs, func(value SymbolFactV2) string { return value.ID })
	for _, replacement := range delta.SymbolReferenceFiles {
		for _, reference := range replacement.References {
			// Rust rejects the whole frame here; so must the model.
			if reference.Path != replacement.Path {
				t.Fatalf("reference delta for %q carries a reference in %q", replacement.Path, reference.Path)
			}
		}
		index := slices.IndexFunc(result.Symbols, func(symbol SymbolFactV2) bool {
			return symbol.ID == replacement.ID
		})
		if index < 0 {
			t.Fatalf("reference delta names symbol %q, which the retained table does not hold", replacement.ID)
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
