package typefacts

import (
	"fmt"
	"testing"
)

func TestTransportManifestKeepsBroadExactPathEvidence(t *testing.T) {
	previous := &FactTable{
		Schema:     TypeFactsSchemaVersion,
		Generation: 1,
		stateID:    1,
	}
	next := &FactTable{
		Schema:     TypeFactsSchemaVersion,
		Generation: 2,
		stateID:    2,
	}
	paths := make(map[string]struct{}, 65)
	for index := range 65 {
		paths[fmt.Sprintf("file-%02d.ts", index)] = struct{}{}
	}

	manifest := transportManifest(
		previous,
		next,
		&closureBuilder{
			referenceChangesExact: true,
			changedSymbols:        newChangedSymbolSet(newSymbolInterner(), nil, nil),
		},
		paths,
	)

	if manifest == nil || !manifest.exact {
		t.Fatal("broad exact path evidence was discarded")
	}
	if len(manifest.sourcePaths) != len(paths) {
		t.Fatalf("manifest paths = %d, want %d", len(manifest.sourcePaths), len(paths))
	}
}

func TestExactSymbolPlanDerivesReferenceOperationsFromCanonicalRuns(t *testing.T) {
	reference := func(path string, start int) Location {
		return Location{Path: path, StartByte: start, EndByte: start + 1}
	}
	base := &FactTable{Symbols: []SymbolFact{{
		ID: "shared",
		References: []Location{
			reference("a.ts", 1),
			reference("b.ts", 1),
			reference("c.ts", 1),
		},
	}}}
	target := &FactTable{Symbols: []SymbolFact{{
		ID: "shared",
		References: []Location{
			reference("a.ts", 1),
			reference("b.ts", 2),
			reference("d.ts", 1),
		},
	}}}
	manifest := &factTableTransportChanges{
		symbolIDs: map[SymbolID]struct{}{"shared": {}},
		exact:     true,
	}
	encoder := &wireTransitionEncoder{}

	encoder.planExactSymbolDelta(base, target, manifest)

	if len(encoder.symbolOps) != 3 {
		t.Fatalf("reference operations = %d, want 3", len(encoder.symbolOps))
	}
	for index, path := range []string{"b.ts", "c.ts", "d.ts"} {
		operation := encoder.symbolOps[index]
		if operation.tag != wireTransitionReplaceReferencePath || operation.referencePath != path {
			t.Fatalf("operation %d = %+v, want reference path %q", index, operation, path)
		}
	}
	if len(encoder.symbolOps[1].references) != 0 {
		t.Fatal("removed c.ts run did not encode an empty replacement")
	}
}
