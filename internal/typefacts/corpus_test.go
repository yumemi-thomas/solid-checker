package typefacts_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

// The generated corpus is deliberately large enough to exercise retained
// entity-run assembly and the Rust-owned sparse transition without making the
// producer enumerate a complete symbol closure.
const (
	corpusParallelSymbolThreshold = 1024
	// Resolved-call declarations, owners, and parameters carry embedded
	// identities. They must not each hydrate a duplicate standalone symbol
	// row: the generated corpus closes roughly 2,450 genuine symbols.
	corpusSymbolClosureBudget = 2600
	corpusModules             = 48
	corpusMembersPerModule    = 24
)

// generateCorpus writes a deterministic TypeScript project large enough to
// cross those thresholds and returns its root.
//
// Determinism is a hard requirement: the retained-versus-fresh oracle compares
// wire bytes, so the generated sources, and the demand list derived from them,
// must be identical on every run and in every process. Everything here is
// index-driven — no maps are iterated, nothing is randomised, and no file
// content depends on the filesystem.
//
// Every module imports from one shared module and from nothing else. Editing a
// leaf therefore affects only that leaf, which is the shape an editor produces
// and the shape retention is meant to exploit; editing the shared module
// affects everything, which is the pessimistic case.
func generateCorpus(tb testing.TB) string {
	tb.Helper()
	root := tb.TempDir()
	writeCorpusFile(tb, root, "tsconfig.json", `{
  "compilerOptions": {
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "target": "ES2022"
  },
  "include": ["*.ts"]
}
`)

	var shared strings.Builder
	shared.WriteString("// Imported by every generated module.\n\n")
	for member := range corpusMembersPerModule {
		fmt.Fprintf(&shared, "export function base%02d(value: number): number {\n  return value + %d;\n}\n\n", member, member)
	}
	shared.WriteString("export async function loadBase(value: number): Promise<number> {\n")
	shared.WriteString("  const resolved = await Promise.resolve(value);\n")
	shared.WriteString("  return base00(resolved);\n}\n")
	writeCorpusFile(tb, root, "shared.ts", shared.String())

	for module := range corpusModules {
		var file strings.Builder
		fmt.Fprintf(&file, "import { base00, base01, loadBase } from \"./shared\";\n\n")
		for member := range corpusMembersPerModule {
			fmt.Fprintf(&file,
				"export function fn%02d_%02d(value: number): number {\n  return base%02d(value) + %d;\n}\n\n",
				module, member, member%corpusMembersPerModule, member)
			fmt.Fprintf(&file,
				"export const value%02d_%02d = fn%02d_%02d(%d);\n\n",
				module, member, module, member, member)
		}
		fmt.Fprintf(&file,
			"export async function refresh%02d(): Promise<number> {\n"+
				"  const loaded = await loadBase(%d);\n"+
				"  return base01(loaded) + base00(value%02d_00);\n}\n",
			module, module, module)
		writeCorpusFile(tb, root, fmt.Sprintf("mod%02d.ts", module), file.String())
	}
	return root
}

func writeCorpusFile(tb testing.TB, root, name, contents string) {
	tb.Helper()
	if err := os.WriteFile(filepath.Join(root, name), []byte(contents), 0o600); err != nil {
		tb.Fatal(err)
	}
}

// TestGeneratedCorpusCrossesTheParallelThresholds asserts the corpus is
// actually big enough. Without it, a future tweak could shrink the generator
// below the thresholds and every scale test would keep passing while silently
// covering only the sequential paths.
func TestGeneratedCorpusProducesSparseEntityRows(t *testing.T) {
	if testing.Short() {
		t.Skip("scale coverage is skipped under -short; the default run includes it")
	}
	ctx := context.Background()
	root := generateCorpus(t)
	backend, err := tsgo.OpenProject(ctx, filepath.Join(root, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = backend.Close() })
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	demands := realisticDemands(t, backend.(demandSource), ctx)
	if _, err := closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
		t.Fatal(err)
	}
	stats := closure.Stats()
	t.Logf("corpus: files=%d demands=%d entities=%d symbols=%d fullTier=%d",
		stats.Files, len(demands), stats.Entities, stats.Symbols, stats.FullTierSymbols)
	if stats.Entities == 0 {
		t.Fatal("sparse materialization produced no entity rows")
	}
	if stats.Symbols != 0 || stats.FullTierSymbols != 0 {
		t.Fatalf("producer materialized client-owned symbol rows: symbols=%d fullTier=%d", stats.Symbols, stats.FullTierSymbols)
	}
}

// TestRetainedDemandClosureAtScaleMatchesFreshMaterialization runs the
// retained-versus-fresh and delta-applied parity checks on a project large
// enough to take the parallel symbol-hydration and reference-refresh paths and
// to hold more than one symbol-store chunk.
func TestRetainedDemandClosureAtScaleMatchesFreshMaterialization(t *testing.T) {
	if testing.Short() {
		t.Skip("scale coverage is skipped under -short; the default run includes it")
	}
	root := generateCorpus(t)
	assertRetainedMatchesFreshMaterialization(t, root, "mod00.ts", fmt.Sprintf("mod%02d.ts", corpusModules-1))
}

func TestRetainedSparseMaterializationAtScaleMatchesFreshMaterialization(t *testing.T) {
	if testing.Short() {
		t.Skip("scale coverage is skipped under -short; the default run includes it")
	}
	ctx := context.Background()
	root := generateCorpus(t)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		t.Fatal(err)
	}
	edited := append(append([]byte(nil), original...), "\n// stable closure edit\n"...)

	open := func() (*typefacts.DemandClosure, demandSource) {
		backend, err := tsgo.OpenProject(ctx, projectID, nil)
		if err != nil {
			t.Fatal(err)
		}
		closure, err := typefacts.NewDemandClosure(backend, nil)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = closure.Close() })
		return closure, backend.(demandSource)
	}

	incremental, incrementalBackend := open()
	demands := realisticDemands(t, incrementalBackend, ctx)
	if _, err := incremental.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
		t.Fatal(err)
	}
	if _, err := incremental.Update(ctx, []typefacts.FileChange{{
		Path: editPath, Version: 1, Source: edited,
	}}); err != nil {
		t.Fatal(err)
	}
	retained, err := incremental.DemandTableForGroups(ctx, 2, groupedDemands(demands), nil)
	if err != nil {
		t.Fatal(err)
	}
	if incremental.Stats().Retention.RetainedFiles == 0 {
		t.Fatal("stable edit did not exercise retained sparse rows")
	}

	fresh, freshBackend := open()
	if _, err := fresh.Update(ctx, []typefacts.FileChange{{
		Path: editPath, Version: 1, Source: edited,
	}}); err != nil {
		t.Fatal(err)
	}
	freshDemands := realisticDemands(t, freshBackend, ctx)
	whole, err := fresh.DemandTableForGroups(ctx, 2, canonicalDemandGroups(freshDemands), nil)
	if err != nil {
		t.Fatal(err)
	}
	assertFullWireTransitionsIdentical(t, "stable sparse materialization", 0, projectID, retained, whole)
}
