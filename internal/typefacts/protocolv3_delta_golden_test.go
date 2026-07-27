package typefacts

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/wirecbor"
)

// deltaGoldenStep is one retained-table transition: apply Delta to the table
// packed in Base and the result must equal the table packed in Expected.
type deltaGoldenStep struct {
	Label    string           `cbor:"label" json:"label"`
	Base     []byte           `cbor:"base" json:"base"`
	Delta    FactTableDeltaV3 `cbor:"delta" json:"delta"`
	Expected []byte           `cbor:"expected" json:"expected"`
}

type deltaGolden struct {
	Steps []deltaGoldenStep `cbor:"steps" json:"steps"`
}

const deltaGoldenProjectID = "/p/tsconfig.json"

// deltaGoldenSteps drives the production differ over the same transitions the
// Go round-trip tests use — an edit, a file deletion, a demand shrink, and a
// per-path reference replacement — and records each (base, delta, expected)
// triple in wire form.
func deltaGoldenSteps(t *testing.T) []deltaGoldenStep {
	t.Helper()
	pack := func(table FactTable, generation uint64) []byte {
		t.Helper()
		packed, err := PackedFactTableV3From(FactTableV2From(table, deltaGoldenProjectID, generation))
		if err != nil {
			t.Fatal(err)
		}
		return packed
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
	}
	deleted := FactTable{
		Schema: 1, Generation: 3, ProjectID: deltaGoldenProjectID,
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{{Location: location("a.ts", 3), Symbol: "a2"}},
		Symbols:  []SymbolFact{{ID: "a2"}},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
	}
	shrunk := FactTable{
		Schema: 1, Generation: 4, ProjectID: deltaGoldenProjectID,
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{},
		Symbols:  []SymbolFact{},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
	}
	exact := &closureBuilder{
		referenceChangesExact: true,
		changedSymbols:        testChangedSet(newSymbolInterner(), "a", "a2", "b"),
	}
	edit.transport = transportManifest(&base, &edit, exact, map[string]struct{}{"a.ts": {}})
	deleted.transport = transportManifest(&edit, &deleted, exact, map[string]struct{}{"b.ts": {}})
	shrunk.transport = transportManifest(&deleted, &shrunk, exact, map[string]struct{}{"a.ts": {}})

	steps := make([]deltaGoldenStep, 0, 4)
	previous := base
	for _, next := range []struct {
		label string
		table FactTable
	}{{"edit", edit}, {"delete", deleted}, {"demand-shrink", shrunk}} {
		steps = append(steps, deltaGoldenStep{
			Label:    next.label,
			Base:     pack(previous, previous.Generation),
			Delta:    DiffFactTablesV3FromInternal(previous, next.table, next.table.Generation),
			Expected: pack(next.table, next.table.Generation),
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
	}
	sharedEdit := FactTable{
		Schema: 1, Generation: 2, ProjectID: deltaGoldenProjectID,
		Symbols: []SymbolFact{{ID: "shared", References: []Location{
			reference("a.ts", 3), reference("a.ts", 5), reference("b.ts", 1),
		}}},
	}
	sharedEdit.transport = transportManifest(&sharedBase, &sharedEdit, &closureBuilder{
		referenceChangesExact: true,
		changedSymbols:        testChangedSet(newSymbolInterner(), "shared"),
	}, map[string]struct{}{"a.ts": {}})
	steps = append(steps, deltaGoldenStep{
		Label:    "symbol-reference-file",
		Base:     pack(sharedBase, sharedBase.Generation),
		Delta:    DiffFactTablesV3FromInternal(sharedBase, sharedEdit, sharedEdit.Generation),
		Expected: pack(sharedEdit, sharedEdit.Generation),
	})
	return steps
}

func deltaGoldenPath(t *testing.T) string {
	t.Helper()
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	return filepath.Join(filepath.Dir(filename), "..", "..", "benchmarks", "phase1", "typefacts-v3-delta-golden.cbor")
}

// The Rust client applies these same deltas in the apply_table_delta tests in
// crates/typefacts/src/session.rs. Keeping the fixture in the repo is what
// makes the two appliers answer for the same input.
func TestDeltaGoldenMatchesProducerOutput(t *testing.T) {
	golden, err := os.ReadFile(deltaGoldenPath(t))
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := wirecbor.Marshal(deltaGolden{Steps: deltaGoldenSteps(t)})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("delta golden is stale: producer emits %d bytes, fixture has %d", len(encoded), len(golden))
	}
}
