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

// scaleCalleeDemands returns one demand per call whose callee resolves to the
// project's shared `scale` symbol, keyed by file. The fixture is script-kind
// (no imports, no exports), so every file's reference to `scale` resolves to
// the same declaration and therefore to the same symbol identity — which is
// what lets a structural-accessor demand in one file suppress a type
// descriptor demanded in another.
func scaleCalleeDemands(t *testing.T, backend demandSource, ctx context.Context) map[string][]typefacts.EntityDemand {
	t.Helper()
	sources, err := backend.SourceFiles(ctx)
	if err != nil {
		t.Fatal(err)
	}
	byPath := make(map[string][]typefacts.EntityDemand)
	for _, source := range sources {
		path := filepath.Clean(source.Path)
		calls, err := backend.SourceCalls(ctx, path)
		if err != nil {
			t.Fatal(err)
		}
		for _, call := range calls {
			byPath[path] = append(byPath[path], typefacts.EntityDemand{
				Location:       call.Callee,
				Symbol:         true,
				TypeDescriptor: true,
			})
		}
		sort.Slice(byPath[path], func(i, j int) bool {
			return byPath[path][i].Location.StartByte < byPath[path][j].Location.StartByte
		})
	}
	return byPath
}

func flattenDemandGroups(byPath map[string][]typefacts.EntityDemand) ([]typefacts.DemandGroup, []typefacts.EntityDemand) {
	paths := make([]string, 0, len(byPath))
	for path := range byPath {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	groups := make([]typefacts.DemandGroup, 0, len(paths))
	flat := make([]typefacts.EntityDemand, 0)
	for _, path := range paths {
		groups = append(groups, typefacts.DemandGroup{Path: path, Demands: byPath[path]})
		flat = append(flat, byPath[path]...)
	}
	return groups, flat
}

// TestSuppressionFlipReachesTheTransportDelta covers the one way a retained
// file's entity rows can change without that file appearing in any update's
// affected set or in any changed demand path: the structural-accessor union
// shifts, so a descriptor that was suppressed becomes visible (or vice versa)
// in a file nobody touched.
//
// The transport manifest is built from changed paths alone. If the
// suppression refresh does not contribute its paths, the delta silently omits
// those rows and the client's retained table diverges from a fresh one — which
// the full-table comparison in TestRetainedDemandClosureMatchesFreshMaterialization
// cannot see, because the producer's own table is correct either way.
func TestSuppressionFlipReachesTheTransportDelta(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.Abs(filepath.Join("testdata", "retained-scripts"))
	if err != nil {
		t.Fatal(err)
	}
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
		return closure, backend.(demandSource)
	}

	alphaPath := filepath.Clean(filepath.Join(root, "use-alpha.ts"))
	islandPath := filepath.Clean(filepath.Join(root, "island.ts"))
	islandOriginal, err := os.ReadFile(islandPath)
	if err != nil {
		t.Fatal(err)
	}
	islandEdit := typefacts.FileChange{
		Path:    islandPath,
		Version: 1,
		Source:  append([]byte("// suppression-flip edit\n"), islandOriginal...),
	}

	// withStructuralOwner marks the named file's demands as structural
	// accessors and leaves every other file demanding a plain descriptor.
	withStructuralOwner := func(byPath map[string][]typefacts.EntityDemand, owner string) map[string][]typefacts.EntityDemand {
		result := make(map[string][]typefacts.EntityDemand, len(byPath))
		for path, demands := range byPath {
			copied := make([]typefacts.EntityDemand, len(demands))
			copy(copied, demands)
			if path == owner {
				for index := range copied {
					copied[index].StructuralAccessor = true
				}
			}
			result[path] = copied
		}
		return result
	}

	incremental, backend := openClosure()
	base := scaleCalleeDemands(t, backend, ctx)

	// Generation 1: alpha owns the structural demands, so the shared symbol is
	// suppressed and beta/gamma carry no descriptor for it.
	suppressed := withStructuralOwner(base, alphaPath)
	groups, _ := flattenDemandGroups(suppressed)
	if _, err := incremental.DemandTableForGroups(ctx, 1, groups, nil); err != nil {
		t.Fatal(err)
	}

	// Advance a generation by editing a file nothing references, so every
	// other file stays retained and the reference delta becomes exact.
	if _, err := incremental.Update(ctx, []typefacts.FileChange{islandEdit}); err != nil {
		t.Fatal(err)
	}
	rebased := scaleCalleeDemands(t, backend, ctx)
	suppressed = withStructuralOwner(rebased, alphaPath)
	groups, _ = flattenDemandGroups(suppressed)
	previous, err := incremental.DemandTableForGroups(ctx, 2, groups, nil)
	if err != nil {
		t.Fatal(err)
	}

	// The flip: alpha stops demanding a structural accessor. The union loses
	// the shared symbol, so beta and gamma must now carry its descriptor even
	// though neither file changed and neither appears in a changed demand path.
	revealed := withStructuralOwner(rebased, "")
	groups, flat := flattenDemandGroups(revealed)
	table, err := incremental.DemandTableForGroups(ctx, 2, groups, []string{alphaPath})
	if err != nil {
		t.Fatal(err)
	}
	stats := incremental.Stats()
	if !stats.Retention.SuppressionRecompute {
		t.Fatalf("the flip did not trigger a suppression recompute; retention = %+v", stats.Retention)
	}
	if stats.Retention.RetainedFiles == 0 {
		t.Fatalf("the flip retained no files, so nothing was at risk; retention = %+v", stats.Retention)
	}

	// A fresh whole-batch run at the same generation with the same demands.
	fresh, freshBackend := openClosure()
	if _, err := fresh.Update(ctx, []typefacts.FileChange{islandEdit}); err != nil {
		t.Fatal(err)
	}
	freshGroups, _ := flattenDemandGroups(withStructuralOwner(scaleCalleeDemands(t, freshBackend, ctx), ""))
	freshTable, err := fresh.DemandTableForGroups(ctx, 2, freshGroups, nil)
	if err != nil {
		t.Fatal(err)
	}
	if fresh.Stats().Retention.RetainedFiles != 0 {
		t.Fatalf("the oracle session retained files; it must be a whole-batch run")
	}

	// The producer's own table is expected to be right either way.
	assertFullWireTransitionsIdentical(t, "retained table after suppression flip", 0, projectID, table, freshTable)

	// Guard the premise: the flip must actually change descriptor visibility,
	// or this test would pass without exercising anything.
	descriptors := func(table *typefacts.FactTable) int {
		count := 0
		for _, entity := range table.Entities {
			if entity.TypeDescriptor != nil {
				count++
			}
		}
		return count
	}
	if descriptors(table) <= descriptors(previous) {
		t.Fatalf("the flip revealed no descriptors (%d before, %d after); demands = %d",
			descriptors(previous), descriptors(table), len(flat))
	}

}

// TestSplitSameLocationDemandsStillMergeIntoOneRow pins that a client may emit
// two demands for the same location without them being adjacent.
//
// entityAccumulator merges same-location demands by comparing against the last
// entity it appended, so a run that separates them would produce two rows for
// one location, and the order-sensitive per-file hash would defeat retention as
// well. Both fail silently, which is why the producer canonicalizes each run it
// is about to resolve rather than trusting the caller.
func TestSplitSameLocationDemandsStillMergeIntoOneRow(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.Abs(filepath.Join("testdata", "retained-closure"))
	if err != nil {
		t.Fatal(err)
	}
	projectID := filepath.Join(root, "tsconfig.json")

	analyze := func(demands []typefacts.EntityDemand) *typefacts.FactTable {
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
		table, err := closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands))
		if err != nil {
			t.Fatal(err)
		}
		return table
	}

	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = backend.Close() })
	usePath := filepath.Clean(filepath.Join(root, "use.ts"))
	calls, err := backend.(demandSource).SourceCalls(ctx, usePath)
	if err != nil {
		t.Fatal(err)
	}
	if len(calls) < 2 {
		t.Fatalf("fixture needs at least two calls in use.ts, found %d", len(calls))
	}

	// Two demands for one location, asking for different facts, with a demand
	// for a different location wedged between them.
	target, other := calls[0].Callee, calls[1].Callee
	split := []typefacts.EntityDemand{
		{Location: target, Symbol: true, References: true},
		{Location: other, Symbol: true, ResolvedCall: true},
		{Location: target, TypeDescriptor: true},
	}
	adjacent := []typefacts.EntityDemand{
		{Location: target, Symbol: true, References: true},
		{Location: target, TypeDescriptor: true},
		{Location: other, Symbol: true, ResolvedCall: true},
	}

	splitTable := analyze(split)
	adjacentTable := analyze(adjacent)

	rows := 0
	for _, entity := range splitTable.Entities {
		if entity.Location.Path == usePath && entity.Location.StartByte == target.StartByte {
			rows++
		}
	}
	if rows != 1 {
		t.Fatalf("the split run produced %d entity rows for one location, want 1; a location must appear exactly once", rows)
	}
	assertFullWireTransitionsIdentical(t, "table from a split same-location demand run", 0, projectID, splitTable, adjacentTable)
}
