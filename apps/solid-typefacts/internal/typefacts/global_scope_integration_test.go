package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

// TestGlobalScopeEditFailsClosedAndKeepsParity covers the one change that
// retention's central premise does not cover.
//
// Retention assumes a changed declaring file puts every referencing file into
// the affected set, which the backend establishes by walking import edges. That
// is sound only for external modules. Script-kind files share one global scope
// and reference each other with no import edge, so before the backend learned to
// fail closed here, editing a declaring file left every other file retained,
// holding durable identities whose declaration spans had moved: the retained
// table kept stale symbol IDs and grew a phantom symbol row, and the client
// received it as an ordinary delta with no error anywhere.
func TestGlobalScopeEditFailsClosedAndKeepsParity(t *testing.T) {
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

	// globals.ts declares the symbols every other file references, and nothing
	// imports it — there is no edge from it to its dependents.
	declaringPath := filepath.Clean(filepath.Join(root, "globals.ts"))
	original, err := os.ReadFile(declaringPath)
	if err != nil {
		t.Fatal(err)
	}
	// Prepending shifts every declaration span in the file, so any retained
	// identity minted from those spans becomes stale.
	edit := typefacts.FileChange{
		Path:    declaringPath,
		Version: 1,
		Source:  append([]byte("// global-scope edit\n"), original...),
	}

	incremental, backend := openClosure()
	demands := realisticDemands(t, backend, ctx)
	if _, err := incremental.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands)); err != nil {
		t.Fatal(err)
	}

	affected, err := incremental.Update(ctx, []typefacts.FileChange{edit})
	if err != nil {
		t.Fatal(err)
	}
	sources, err := backend.SourceFiles(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(affected.Files) < len(sources) {
		t.Fatalf("editing a global-scope declaring file reported %d affected of %d source files; "+
			"the import-edge walk cannot see global-scope references, so the affected set must fail closed",
			len(affected.Files), len(sources))
	}

	rebased := realisticDemands(t, backend, ctx)
	table, err := incremental.DemandTableForGroups(ctx, 2, groupedDemands(rebased), demandPaths(rebased))
	if err != nil {
		t.Fatal(err)
	}
	retention := incremental.Stats().Retention
	if retention.RetainedFiles != 0 {
		t.Fatalf("a global-scope edit retained %d files; every identity in the project may have moved, so none may be reused (retention = %+v)",
			retention.RetainedFiles, retention)
	}
	fresh, freshBackend := openClosure()
	if _, err := fresh.Update(ctx, []typefacts.FileChange{edit}); err != nil {
		t.Fatal(err)
	}
	freshDemands := realisticDemands(t, freshBackend, ctx)
	freshTable, err := fresh.DemandTableForGroups(ctx, 2, canonicalDemandGroups(freshDemands), nil)
	if err != nil {
		t.Fatal(err)
	}
	assertFullWireTransitionsIdentical(t, "table after a global-scope edit", 0, projectID, table, freshTable)
}
