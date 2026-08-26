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

// BenchmarkUpdateShapeChangeAtScale prices the update whose edit changes the
// module's exported shape: the semantic cutoff cannot apply, so this is the
// path that must walk the module graph for the affected set. The comment-only
// edit in BenchmarkUpdateOnlyAtScale never reaches that walk.
func BenchmarkUpdateShapeChangeAtScale(b *testing.B) {
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
		source := make([]byte, 0, len(original)+48)
		source = append(source, original...)
		source = append(source, fmt.Sprintf("\nexport const shapeEdit%d = %d;\n", edit, edit)...)
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
// extraPaths pads the exact changed-path evidence with paths that exist in no
// generation. Naming one is a harmless over-approximation and isolates the
// planning cost of a broad manifest from actual checker recomputation.
func diffBenchTables(b *testing.B, extraPaths int) (previous, next *typefacts.FactTable, projectID string) {
	b.Helper()
	ctx := context.Background()
	root := generateCorpus(b)
	projectID = filepath.Clean(filepath.Join(root, "tsconfig.json"))
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
	for phantom := 0; phantom < extraPaths; phantom++ {
		changed = append(changed, filepath.Join(root, fmt.Sprintf("phantom%02d.ts", phantom)))
	}
	next, err = closure.DemandTableForGroups(ctx, 2, groupedDemands(demands), changed)
	if err != nil {
		b.Fatal(err)
	}
	return previous, next, projectID
}

// BenchmarkManifestDeltaWireTableTransition prices the ordinary warm response:
// the exact manifest limits planning to candidates, then the shared encoder
// writes the delta transition directly from internal rows.
func BenchmarkManifestDeltaWireTableTransition(b *testing.B) {
	previous, next, projectID := diffBenchTables(b, 0)
	encoder := &typefacts.WireTransitionEncoderForTest{}
	var transitionBytes int
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		transition, err := encoder.Delta(projectID, "1", previous, next)
		if err != nil {
			b.Fatal(err)
		}
		transitionBytes = len(transition)
	}
	b.ReportMetric(float64(transitionBytes), "transition-B/op")
}

// BenchmarkBroadManifestDeltaWireTableTransition prices the former 64-path
// cliff: one real edit plus 70 exact, empty path candidates.
func BenchmarkBroadManifestDeltaWireTableTransition(b *testing.B) {
	previous, next, projectID := diffBenchTables(b, 70)
	encoder := &typefacts.WireTransitionEncoderForTest{}
	var transitionBytes int
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		transition, err := encoder.Delta(projectID, "1", previous, next)
		if err != nil {
			b.Fatal(err)
		}
		transitionBytes = len(transition)
	}
	b.ReportMetric(float64(transitionBytes), "transition-B/op")
}

// BenchmarkFallbackDeltaWireTableTransition retains the evidence-free
// correctness fallback as an explicit comparison.
func BenchmarkFallbackDeltaWireTableTransition(b *testing.B) {
	previous, next, projectID := diffBenchTables(b, 0)
	typefacts.DropTransportEvidenceForTest(next)
	encoder := &typefacts.WireTransitionEncoderForTest{}
	var transitionBytes int
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		transition, err := encoder.Delta(projectID, "1", previous, next)
		if err != nil {
			b.Fatal(err)
		}
		transitionBytes = len(transition)
	}
	b.ReportMetric(float64(transitionBytes), "transition-B/op")
}

// BenchmarkFullWireTableTransition prices the cold response with the same
// session-owned encoder used in production. transition-B/op records the frame
// size the client decodes.
func BenchmarkFullWireTableTransition(b *testing.B) {
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

	encoder := &typefacts.WireTransitionEncoderForTest{}
	var transitionBytes int
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		transition, err := encoder.Full(projectID, table)
		if err != nil {
			b.Fatal(err)
		}
		transitionBytes = len(transition)
	}
	b.ReportMetric(float64(transitionBytes), "transition-B/op")
}
