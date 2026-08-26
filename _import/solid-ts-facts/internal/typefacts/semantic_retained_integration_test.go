package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

// demandSource is what a test needs in order to synthesise a client's demand
// list: the bulk syntax tables a real client derives from its own parser. It is
// deliberately not ClosureBackend — the closure never calls these, and keeping
// them out of its interface is the point of that interface.
type demandSource interface {
	SourceFiles(context.Context) ([]typefacts.SourceFile, error)
	SourceCalls(context.Context, string) ([]typefacts.SourceCall, error)
	SourceBindings(context.Context, string) ([]typefacts.SourceBinding, error)
}

// realisticDemands mirrors the shape of the Rust session's demand list: a
// symbol demand (with references) per binding, a type-descriptor and
// resolved-call demand per call callee, an async flag per file, and a
// sprinkling of structural-accessor demands so the suppression union is
// non-empty. Both sessions under comparison generate demands from the same
// content, so the lists are identical by construction.
func realisticDemands(t testing.TB, backend demandSource, ctx context.Context) []typefacts.EntityDemand {
	t.Helper()
	sources, err := backend.SourceFiles(ctx)
	if err != nil {
		t.Fatal(err)
	}
	var demands []typefacts.EntityDemand
	for _, source := range sources {
		path := filepath.Clean(source.Path)
		bindings, err := backend.SourceBindings(ctx, path)
		if err != nil {
			t.Fatal(err)
		}
		for index, binding := range bindings {
			for _, name := range binding.Names {
				if name.Path == "" || name.EndByte <= name.StartByte {
					continue
				}
				demands = append(demands, typefacts.EntityDemand{
					Location:           name,
					Symbol:             true,
					References:         index%3 == 0,
					StructuralAccessor: index%7 == 0,
				})
			}
		}
		calls, err := backend.SourceCalls(ctx, path)
		if err != nil {
			t.Fatal(err)
		}
		for index, call := range calls {
			demands = append(demands, typefacts.EntityDemand{
				Location:           call.Callee,
				Symbol:             true,
				TypeDescriptor:     index%2 == 0,
				ResolvedCall:       true,
				RuntimeValueDomain: index%3 == 0,
				Async:              index%5 == 0,
			})
		}
	}
	return demands
}

func groupedDemands(demands []typefacts.EntityDemand) []typefacts.DemandGroup {
	byPath := make(map[string][]typefacts.EntityDemand)
	for _, demand := range demands {
		path := filepath.Clean(demand.Location.Path)
		byPath[path] = append(byPath[path], demand)
	}
	groups := make([]typefacts.DemandGroup, 0, len(byPath))
	for path, demands := range byPath {
		groups = append(groups, typefacts.DemandGroup{Path: path, Demands: demands})
	}
	return groups
}

// canonicalDemandGroups builds per-file runs by sorting the flat list and
// splitting on path changes. groupedDemands above builds the same runs through a
// map instead; keeping the two constructions independent is deliberate, so the
// incremental and fresh sides of the oracle cannot agree merely because they
// share a helper.
func canonicalDemandGroups(demands []typefacts.EntityDemand) []typefacts.DemandGroup {
	sorted := make([]typefacts.EntityDemand, len(demands))
	copy(sorted, demands)
	sort.SliceStable(sorted, func(i, j int) bool {
		left, right := sorted[i].Location, sorted[j].Location
		leftPath, rightPath := filepath.Clean(left.Path), filepath.Clean(right.Path)
		if leftPath != rightPath {
			return leftPath < rightPath
		}
		if left.StartByte != right.StartByte {
			return left.StartByte < right.StartByte
		}
		return left.EndByte < right.EndByte
	})
	var groups []typefacts.DemandGroup
	for index := 0; index < len(sorted); {
		path := filepath.Clean(sorted[index].Location.Path)
		end := index
		for end < len(sorted) && filepath.Clean(sorted[end].Location.Path) == path {
			end++
		}
		groups = append(groups, typefacts.DemandGroup{Path: path, Demands: sorted[index:end]})
		index = end
	}
	return groups
}

func demandPaths(demands []typefacts.EntityDemand) []string {
	seen := make(map[string]struct{})
	for _, demand := range demands {
		seen[filepath.Clean(demand.Location.Path)] = struct{}{}
	}
	paths := make([]string, 0, len(seen))
	for path := range seen {
		paths = append(paths, path)
	}
	return paths
}

// TestRetainedDemandClosureMatchesFreshMaterialization drives one retained
// incremental session through an edit script and byte-compares every
// generation's wire table against a fresh session that first sees the same
// overlays in one update — a whole-batch materialization with nothing
// retained.
//
// Fixtures live in this repository. A missing one is a failure, never a skip:
// this oracle is the only check that retained output is byte-identical to a
// fresh materialization, so silently not running it is worse than not having
// it.
func TestRetainedDemandClosureMatchesFreshMaterialization(t *testing.T) {
	fixtures := []struct {
		name string
		// edit is byte-shifted then reverted; other is edited last and has
		// its complete demand run removed at the middle step. other must not
		// be reachable from edit, so that editing one leaves the other
		// retained.
		edit  string
		other string
	}{
		{name: "retained-closure", edit: "source.ts", other: "unrelated.ts"},
		{name: "aliased-import", edit: "source.ts", other: "unrelated.ts"},
	}
	for _, fixture := range fixtures {
		t.Run(fixture.name, func(t *testing.T) {
			root, err := filepath.Abs(filepath.Join("testdata", fixture.name))
			if err != nil {
				t.Fatal(err)
			}
			if _, err := os.Stat(filepath.Join(root, "tsconfig.json")); err != nil {
				t.Fatalf("fixture is missing, so the retained-closure oracle cannot run: %v", err)
			}
			assertRetainedMatchesFreshMaterialization(t, root, fixture.edit, fixture.other)
		})
	}
}

func assertRetainedMatchesFreshMaterialization(t *testing.T, root, editFile, otherFile string) {
	t.Helper()
	ctx := context.Background()
	projectID := filepath.Join(root, "tsconfig.json")

	openClosure := func() (*typefacts.DemandClosure, demandSource) {
		t.Helper()
		backend, err := tsgo.OpenProject(ctx, projectID, nil)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = backend.Close() })
		closure, err := typefacts.NewDemandClosure(backend, nil)
		if err != nil {
			t.Fatal(err)
		}
		source, ok := backend.(demandSource)
		if !ok {
			t.Fatal("tsgo backend must expose the bulk syntax tables tests derive demands from")
		}
		return closure, source
	}

	editPath := filepath.Join(root, editFile)
	original, err := os.ReadFile(editPath)
	if err != nil {
		t.Fatal(err)
	}
	otherPath := filepath.Join(root, otherFile)
	otherOriginal, err := os.ReadFile(otherPath)
	if err != nil {
		t.Fatal(err)
	}

	// The edit script: shift bytes in one leaf, revert it, edit another
	// file. Each step is one accepted update and one analyzed generation.
	script := []typefacts.FileChange{
		{Path: editPath, Version: 1, Source: append([]byte("// retained-closure edit\n"), original...)},
		{Path: editPath, Version: 2, Source: original},
		{Path: otherPath, Version: 3, Source: append([]byte("// retained-closure edit\n"), otherOriginal...)},
	}

	incremental, incrementalBackend := openClosure()
	demands := realisticDemands(t, incrementalBackend, ctx)
	if _, err := incremental.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
		t.Fatal(err)
	}

	generation := uint64(1)
	retainedSeen := false
	asyncCacheSeen := false
	asyncFactsExist := false
	for step, change := range script {
		if _, err := incremental.Update(ctx, []typefacts.FileChange{change}); err != nil {
			t.Fatal(err)
		}
		generation++
		demands := realisticDemands(t, incrementalBackend, ctx)
		// Exercise both removal and restoration of a complete per-file
		// demand run. The grouped API must not retain rows from the removed
		// run, and must reproduce the flat canonical result when restored.
		if step == 1 {
			filtered := demands[:0]
			for _, demand := range demands {
				if filepath.Clean(demand.Location.Path) != filepath.Clean(otherPath) {
					filtered = append(filtered, demand)
				}
			}
			demands = filtered
		}
		table, err := incremental.DemandTableForGroups(
			ctx,
			generation,
			groupedDemands(demands),
			[]string{editPath, otherPath},
		)
		if err != nil {
			t.Fatal(err)
		}
		stats := incremental.Stats()

		if stats.Retention.RetainedFiles > 0 {
			retainedSeen = true
		}
		// RetainedAsyncFiles counts retained async demand groups, and a group
		// with no async functions at all still counts (durableAsyncFunctions of
		// an empty list is true). So require a retained generation whose table
		// actually carries async facts — and only require it of a fixture that
		// has async functions to begin with.
		for _, file := range table.Files {
			if len(file.AsyncFunctions) > 0 {
				asyncFactsExist = true
				if stats.Retention.RetainedAsyncFiles > 0 {
					asyncCacheSeen = true
				}
				break
			}
		}

		// The fresh oracle: a new project that receives every overlay up
		// to this step in one update, so its only materialization is a
		// whole-batch run at the same generation number... generations
		// advance per accepted update, so replay the script's prefix as
		// individual updates without analyzing between them.
		fresh, freshBackend := openClosure()
		for _, replay := range script[:step+1] {
			if _, err := fresh.Update(ctx, []typefacts.FileChange{replay}); err != nil {
				t.Fatal(err)
			}
		}
		freshDemands := realisticDemands(t, freshBackend, ctx)
		if step == 1 {
			filtered := freshDemands[:0]
			for _, demand := range freshDemands {
				if filepath.Clean(demand.Location.Path) != filepath.Clean(otherPath) {
					filtered = append(filtered, demand)
				}
			}
			freshDemands = filtered
		}
		freshTable, err := fresh.DemandTableForGroups(ctx, generation, canonicalDemandGroups(freshDemands), nil)
		if err != nil {
			t.Fatal(err)
		}
		freshStats := fresh.Stats()
		if freshStats.Retention.RetainedFiles != 0 {
			t.Fatalf("step %d: fresh session retained %d files; the oracle must be a whole-batch run", step, freshStats.Retention.RetainedFiles)
		}

		assertFullWireTransitionsIdentical(t, "retained materialization", step, projectID, table, freshTable)
	}
	if !retainedSeen {
		t.Fatal("the edit script never exercised retention; the test is vacuous")
	}
	if asyncFactsExist && !asyncCacheSeen {
		t.Fatal("the fixture has async functions but no retained generation carried them; the async-cache parity check is vacuous")
	}
}
