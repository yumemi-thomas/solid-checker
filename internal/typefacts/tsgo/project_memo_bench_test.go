package tsgo

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
)

// BenchmarkSourceFactsAfterLeafEdit models the narrow-affected-set workload:
// independent modules, one edited per generation, every file's source facts
// re-queried afterwards (as one engine Snapshot does). The generated large
// corpus cannot show this — its import chain puts nearly every file in every
// affected set — so this benchmark is the memo's win gate at mechanism level.
func BenchmarkSourceFactsAfterLeafEdit(b *testing.B) {
	const files = 60
	const callsPerFile = 30

	dir := b.TempDir()
	write := func(name, source string) string {
		b.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			b.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)

	moduleSource := func(index int, revision int) string {
		var source strings.Builder
		fmt.Fprintf(&source, "export function make%d(): number {\n  return %d;\n}\n", index, revision)
		for call := 0; call < callsPerFile; call++ {
			fmt.Fprintf(&source, "export const value%d_%d = make%d();\n", index, call, index)
		}
		return source.String()
	}
	paths := make([]string, files)
	for index := range paths {
		paths[index] = write(fmt.Sprintf("mod%d.ts", index), moduleSource(index, 0))
	}
	leafPath := write("leaf.ts", moduleSource(files, 0))

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
	if err != nil {
		b.Fatal(err)
	}
	defer opened.Close()
	discoverer := opened.(typefacts.CallDiscoverer)

	queryAll := func() {
		b.Helper()
		for _, path := range append(paths, leafPath) {
			if _, err := discoverer.SourceCalls(ctx, path); err != nil {
				b.Fatal(err)
			}
			if _, err := discoverer.(typefacts.BindingDiscoverer).SourceBindings(ctx, path); err != nil {
				b.Fatal(err)
			}
		}
	}
	queryAll()

	b.ResetTimer()
	for iteration := 0; iteration < b.N; iteration++ {
		change := typefacts.FileChange{
			Path:    leafPath,
			Version: uint64(iteration + 1),
			Source:  []byte(moduleSource(files, iteration+1)),
		}
		if _, err := opened.Update(ctx, []typefacts.FileChange{change}); err != nil {
			b.Fatal(err)
		}
		queryAll()
	}
}

// BenchmarkReferencesAfterLeafEdit is the reference index's counterpart to
// BenchmarkSourceFactsAfterLeafEdit: independent modules, one edited per
// generation, one References query per module afterwards (as one closure
// materialization does for every non-alias symbol). It is the win gate for
// incremental index maintenance; the generated large corpus, whose import
// chain puts nearly every file in every affected set, is the
// no-regression gate.
func BenchmarkReferencesAfterLeafEdit(b *testing.B) {
	const files = 60
	const callsPerFile = 30

	dir := b.TempDir()
	write := func(name, source string) string {
		b.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			b.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)

	moduleSource := func(index int, revision int) string {
		var source strings.Builder
		fmt.Fprintf(&source, "export function make%d(): number {\n  return %d;\n}\n", index, revision)
		for call := 0; call < callsPerFile; call++ {
			fmt.Fprintf(&source, "export const value%d_%d = make%d();\n", index, call, index)
		}
		return source.String()
	}
	paths := make([]string, files)
	for index := range paths {
		paths[index] = write(fmt.Sprintf("mod%d.ts", index), moduleSource(index, 0))
	}
	leafPath := write("leaf.ts", moduleSource(files, 0))

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
	if err != nil {
		b.Fatal(err)
	}
	defer opened.Close()
	discoverer := opened.(typefacts.CallDiscoverer)

	targets := make([]typefacts.SymbolID, 0, files+1)
	for _, path := range append(paths, leafPath) {
		calls, err := discoverer.SourceCalls(ctx, path)
		if err != nil {
			b.Fatal(err)
		}
		if len(calls) == 0 {
			b.Fatalf("no source calls in %s", path)
		}
		targets = append(targets, calls[0].Target)
	}
	queryAll := func() {
		b.Helper()
		for _, target := range targets {
			references, err := opened.References(ctx, target)
			if err != nil {
				b.Fatal(err)
			}
			if len(references) != callsPerFile {
				b.Fatalf("expected %d references, got %d", callsPerFile, len(references))
			}
		}
	}
	queryAll()

	b.ResetTimer()
	for iteration := 0; iteration < b.N; iteration++ {
		change := typefacts.FileChange{
			Path:    leafPath,
			Version: uint64(iteration + 1),
			Source:  []byte(moduleSource(files, iteration+1)),
		}
		if _, err := opened.Update(ctx, []typefacts.FileChange{change}); err != nil {
			b.Fatal(err)
		}
		queryAll()
	}
}
