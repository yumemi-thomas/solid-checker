package typefacts_test

import (
	"context"
	"path/filepath"
	"regexp"
	"testing"

	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/internal/typefacts/tsgo"
)

var (
	benchmarkFunctionPattern = regexp.MustCompile(`\bfunction\s+([A-Za-z_$][A-Za-z0-9_$]*)`)
	benchmarkReferenceCount  int
)

func BenchmarkProjectReferenceLookups(b *testing.B) {
	project, err := tsgo.OpenProject(context.Background(), filepath.Join("..", "engine", "testdata", "eslint-reactivity-v2", "tsconfig.json"))
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = project.Close() })

	files, err := project.SourceFiles(context.Background())
	if err != nil {
		b.Fatal(err)
	}
	ids := make([]typefacts.SymbolID, 0)
	for _, file := range files {
		for _, match := range benchmarkFunctionPattern.FindAllSubmatchIndex(file.Source, -1) {
			start, end := match[2], match[3]
			id, resolveErr := project.SymbolAt(context.Background(), typefacts.Location{
				Path: file.Path, StartByte: start, EndByte: end,
			})
			if resolveErr != nil {
				b.Fatal(resolveErr)
			}
			ids = append(ids, id)
		}
	}
	if len(ids) < 10 {
		b.Fatalf("resolved %d function symbols, want a representative set", len(ids))
	}

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		count := 0
		for _, id := range ids {
			references, lookupErr := project.References(context.Background(), id)
			if lookupErr != nil {
				b.Fatal(lookupErr)
			}
			count += len(references)
		}
		benchmarkReferenceCount = count
	}
	b.ReportMetric(float64(len(ids)), "symbols/op")
}
