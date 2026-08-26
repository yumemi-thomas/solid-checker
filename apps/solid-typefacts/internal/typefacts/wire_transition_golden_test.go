package typefacts

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

// transitionGoldenStep is one retained-table transition. The Rust client
// materializes BaseTransition, applies Transition against BaseToken, and
// compares the result with ExpectedTransition.
type transitionGoldenStep struct {
	Label              string `cbor:"label" json:"label"`
	BaseToken          string `cbor:"baseToken" json:"baseToken"`
	BaseTransition     []byte `cbor:"baseTransition" json:"baseTransition"`
	Transition         []byte `cbor:"transition" json:"transition"`
	ExpectedTransition []byte `cbor:"expectedTransition" json:"expectedTransition"`
}

type transitionGolden struct {
	Steps []transitionGoldenStep `cbor:"steps" json:"steps"`
}

const deltaGoldenProjectID = "/p/tsconfig.json"

// transitionGoldenSteps drives the production encoder over an edit, deletion,
// demand shrink, and per-path reference replacement. Full and delta frames use
// the same decoder and row writers.
func transitionGoldenSteps(t *testing.T) []transitionGoldenStep {
	t.Helper()
	encoder := &wireTransitionEncoder{tableSchema: TypeFactsTableSchemaVersionV3}
	packFull := func(table *FactTable) []byte {
		t.Helper()
		encoded, err := encoder.Encode(wireTransitionInput{
			ProjectID: deltaGoldenProjectID,
			Target:    table,
		})
		if err != nil {
			t.Fatal(err)
		}
		return encoded.Bytes
	}
	packDelta := func(base, target *FactTable, token string) []byte {
		t.Helper()
		encoded, err := encoder.Encode(wireTransitionInput{
			ProjectID:      deltaGoldenProjectID,
			BaseStateToken: token,
			Base:           base,
			Target:         target,
		})
		if err != nil {
			t.Fatal(err)
		}
		return encoded.Bytes
	}

	location := func(path string, start int) Location {
		return Location{Path: path, StartByte: start, EndByte: start + 1}
	}
	base := FactTable{
		Schema: 1, Generation: 1, ProjectID: deltaGoldenProjectID,
		Sources: []SourceFile{{Path: "a.ts", Source: []byte("a")}, {Path: "b.ts", Source: []byte("b")}},
		Entities: []EntityFact{
			{Location: location("a.ts", 1), Symbol: "a"},
			{Location: location("b.ts", 1), Symbol: "b"},
		},
		Symbols: []SymbolFact{{ID: "a"}, {ID: "b"}},
		Files:   []FileFact{{Path: "a.ts"}, {Path: "b.ts"}},
		stateID: 1,
	}
	edit := FactTable{
		Schema: 1, Generation: 2, ProjectID: deltaGoldenProjectID,
		Sources: []SourceFile{{Path: "a.ts", Source: []byte("a2")}, {Path: "b.ts", Source: []byte("b")}},
		Entities: []EntityFact{
			{Location: location("a.ts", 3), Symbol: "a2"},
			{Location: location("b.ts", 1), Symbol: "b"},
		},
		Symbols: []SymbolFact{{ID: "a2"}, {ID: "b"}},
		Files:   []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}, {Path: "b.ts"}},
		stateID: 2,
	}
	deleted := FactTable{
		Schema: 1, Generation: 3, ProjectID: deltaGoldenProjectID,
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{{Location: location("a.ts", 3), Symbol: "a2"}},
		Symbols:  []SymbolFact{{ID: "a2"}},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
		stateID:  3,
	}
	shrunk := FactTable{
		Schema: 1, Generation: 4, ProjectID: deltaGoldenProjectID,
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{},
		Symbols:  []SymbolFact{},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
		stateID:  4,
	}
	exact := &closureBuilder{
		referenceChangesExact: true,
		changedSymbols:        testChangedSet(newSymbolInterner(), "a", "a2", "b"),
	}
	edit.transport = transportManifest(&base, &edit, exact, map[string]struct{}{"a.ts": {}})
	deleted.transport = transportManifest(&edit, &deleted, exact, map[string]struct{}{"b.ts": {}})
	shrunk.transport = transportManifest(&deleted, &shrunk, exact, map[string]struct{}{"a.ts": {}})

	steps := make([]transitionGoldenStep, 0, 4)
	previous := base
	for index, next := range []struct {
		label string
		table FactTable
	}{{"edit", edit}, {"delete", deleted}, {"demand-shrink", shrunk}} {
		token := []string{"base-1", "base-2", "base-3"}[index]
		steps = append(steps, transitionGoldenStep{
			Label:              next.label,
			BaseToken:          token,
			BaseTransition:     packFull(&previous),
			Transition:         packDelta(&previous, &next.table, token),
			ExpectedTransition: packFull(&next.table),
		})
		previous = next.table
	}

	// The reference-file delta is the trickiest path in the Rust applier: it
	// splices one path's references inside an already path-sorted list.
	reference := func(path string, start int) Location {
		return Location{Path: path, StartByte: start, EndByte: start + 1}
	}
	sharedBase := FactTable{
		Schema: 1, Generation: 1, ProjectID: deltaGoldenProjectID,
		Symbols: []SymbolFact{{ID: "shared", References: []Location{
			reference("a.ts", 1), reference("b.ts", 1),
		}}},
		stateID: 5,
	}
	sharedEdit := FactTable{
		Schema: 1, Generation: 2, ProjectID: deltaGoldenProjectID,
		Symbols: []SymbolFact{{ID: "shared", References: []Location{
			reference("a.ts", 3), reference("a.ts", 5), reference("b.ts", 1),
		}}},
		stateID: 6,
	}
	sharedEdit.transport = transportManifest(&sharedBase, &sharedEdit, &closureBuilder{
		referenceChangesExact: true,
		changedSymbols:        testChangedSet(newSymbolInterner(), "shared"),
	}, map[string]struct{}{"a.ts": {}})
	steps = append(steps, transitionGoldenStep{
		Label:              "symbol-reference-file",
		BaseToken:          "base-reference",
		BaseTransition:     packFull(&sharedBase),
		Transition:         packDelta(&sharedBase, &sharedEdit, "base-reference"),
		ExpectedTransition: packFull(&sharedEdit),
	})
	return steps
}

func transitionGoldenPath(t *testing.T) string {
	t.Helper()
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	return filepath.Join(filepath.Dir(filename), "..", "..", "..", "..", "benchmarks", "typefacts", "phase1", "typefacts-v5-transition-golden.cbor")
}

// The Rust client applies these same transitions in crates/typefacts/src.
// Keeping the fixture in the repo makes both languages answer for the same
// input. Set
// TYPEFACTS_UPDATE_GOLDEN=1 to regenerate the fixture after a deliberate,
// coordinated format change.
func TestV5TransitionGoldenMatchesProducerOutput(t *testing.T) {
	encoded, err := wirecbor.Marshal(transitionGolden{Steps: transitionGoldenSteps(t)})
	if err != nil {
		t.Fatal(err)
	}
	if os.Getenv("TYPEFACTS_UPDATE_GOLDEN") != "" {
		if err := os.WriteFile(transitionGoldenPath(t), encoded, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	golden, err := os.ReadFile(transitionGoldenPath(t))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("transition golden is stale: producer emits %d bytes, fixture has %d", len(encoded), len(golden))
	}
}
