package typefacts_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

// These benchmarks price the transport stages the at-scale lifecycle
// benchmarks cannot separate: the update path alone, the two table-diff
// strategies, and the cold whole-table pack. Each isolates one stage so a
// change to that stage is visible without being drowned in checker work.

// BenchmarkUpdateOnlyAtScale prices one accepted leaf update with no analyze
// behind it: the incremental program update, the affected-set walk (which
// today rebuilds the reverse-dependency graph from every import edge), and
// retained-state eviction. The leaf-edit benchmark pays this too, but bundled
// with a full analyze; this one moves when the update path itself does.
func BenchmarkUpdateOnlyAtScale(b *testing.B) {
	ctx := context.Background()
	root := generateCorpus(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		b.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = closure.Close() })

	demands := realisticDemands(b, backend.(demandSource), ctx)
	if _, err := closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
		b.Fatal(err)
	}

	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}

	edit := 0
	b.ReportAllocs()
	for b.Loop() {
		edit++
		source := make([]byte, 0, len(original)+32)
		source = append(source, original...)
		source = append(source, fmt.Sprintf("\n// edit %d\n", edit)...)
		if _, err := closure.Update(ctx, []typefacts.FileChange{{Path: editPath, Version: uint64(edit), Source: source}}); err != nil {
			b.Fatal(err)
		}
	}
}

// diffBenchTables materializes two tables exactly one accepted leaf edit
// apart, the way Session holds them: the previous generation's table is safe
// for precisely one generation before the closure recycles its storage, and
// no further materialization happens here.
//
// With forceFallback the changed-path list handed to the second
// materialization is padded past the transport manifest's 64-path limit with
// paths that exist in no generation. Naming a path that never held a demand
// run is a harmless over-approximation, but pushing the count over the limit
// suppresses the manifest, which is exactly the state a >64-file change
// (branch switch, format-on-save across a package) puts the producer in.
func diffBenchTables(b *testing.B, forceFallback bool) (previous, next *typefacts.FactTable) {
	b.Helper()
	ctx := context.Background()
	root := generateCorpus(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		b.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = closure.Close() })

	demands := realisticDemands(b, backend.(demandSource), ctx)
	previous, err = closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands))
	if err != nil {
		b.Fatal(err)
	}

	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}
	edited := append(append(make([]byte, 0, len(original)+16), original...), "\n// edit\n"...)
	if _, err := closure.Update(ctx, []typefacts.FileChange{{Path: editPath, Version: 1, Source: edited}}); err != nil {
		b.Fatal(err)
	}

	changed := []string{editPath}
	if forceFallback {
		for phantom := 0; phantom < 70; phantom++ {
			changed = append(changed, filepath.Join(root, fmt.Sprintf("phantom%02d.ts", phantom)))
		}
	}
	next, err = closure.DemandTableForGroups(ctx, 2, groupedDemands(demands), changed)
	if err != nil {
		b.Fatal(err)
	}
	return previous, next
}

// BenchmarkManifestTableDiff prices the ordinary delta construction: the
// transport manifest names the rows that may differ, so the diff touches
// candidates instead of tables.
func BenchmarkManifestTableDiff(b *testing.B) {
	previous, next := diffBenchTables(b, false)
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		delta := typefacts.DiffFactTablesV3FromInternal(*previous, *next, 2)
		if delta.Empty() {
			b.Fatal("leaf edit must produce a non-empty delta")
		}
	}
}

// BenchmarkFallbackTableDiff prices the same one-edit delta computed without
// the manifest's help: every source, file, symbol, and entity row of both
// tables is visited and compared. The gap between this and
// BenchmarkManifestTableDiff is what a client pays extra per analyze once a
// change exceeds the manifest's path limit.
func BenchmarkFallbackTableDiff(b *testing.B) {
	previous, next := diffBenchTables(b, true)
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		delta := typefacts.DiffFactTablesV3FromInternal(*previous, *next, 2)
		if delta.Empty() {
			b.Fatal("leaf edit must produce a non-empty delta")
		}
	}
}

// BenchmarkColdTablePack prices the full-mode response body: converting the
// internal table to its wire form and packing it, which is what a client's
// first analyze and every retained-state desync pays on top of the analysis
// itself. Today that route allocates a complete intermediate FactTableV2 just
// to feed the packed encoder. packed-B/op records the frame size the client
// must then decode.
func BenchmarkColdTablePack(b *testing.B) {
	ctx := context.Background()
	root := generateCorpus(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		b.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = closure.Close() })

	demands := realisticDemands(b, backend.(demandSource), ctx)
	table, err := closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands))
	if err != nil {
		b.Fatal(err)
	}

	var packedBytes int
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		packed, err := typefacts.PackedFactTableV3From(typefacts.FactTableV2From(*table, projectID, 1))
		if err != nil {
			b.Fatal(err)
		}
		packedBytes = len(packed)
	}
	b.ReportMetric(float64(packedBytes), "packed-B/op")
}
