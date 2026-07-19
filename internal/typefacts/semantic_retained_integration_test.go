package typefacts_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
	"github.com/yumemi-thomas/solid-check/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-check/internal/wirecbor"
)

// realisticDemands mirrors the shape of the Rust session's demand list: a
// symbol demand (with references) per binding, a type-descriptor and
// resolved-call demand per call callee, an async flag per file, and a
// sprinkling of structural-accessor demands so the suppression union is
// non-empty. Both sessions under comparison generate demands from the same
// content, so the lists are identical by construction.
func realisticDemands(t *testing.T, backend typefacts.ClosureBackend, ctx context.Context) []typefacts.EntityDemand {
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
				Location:       call.Callee,
				Symbol:         true,
				TypeDescriptor: index%2 == 0,
				ResolvedCall:   true,
				Async:          index%5 == 0,
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
func TestRetainedDemandClosureMatchesFreshMaterialization(t *testing.T) {
	root, err := filepath.Abs("../engine/testdata/eslint-reactivity-v2")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, "tsconfig.json")); err != nil {
		t.Skipf("fixture unavailable: %v", err)
	}
	ctx := context.Background()
	projectID := filepath.Join(root, "tsconfig.json")

	openClosure := func() (*typefacts.ClosureProject, typefacts.ClosureBackend) {
		t.Helper()
		backend, err := tsgo.OpenProject(ctx, projectID)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = backend.Close() })
		closure, err := typefacts.NewClosureProject(backend, false)
		if err != nil {
			t.Fatal(err)
		}
		full, ok := backend.(typefacts.ClosureBackend)
		if !ok {
			t.Fatal("tsgo backend must satisfy ClosureBackend")
		}
		return closure, full
	}

	editPath := filepath.Join(root, "component-props-read.tsx")
	original, err := os.ReadFile(editPath)
	if err != nil {
		t.Fatal(err)
	}
	otherPath := filepath.Join(root, "after-await-member.tsx")
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
	symbolCacheSeen := false
	referenceCacheSeen := false
	patchedSymbolRowsSeen := false
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
		retainedResponse := typefacts.ClosureResponse{
			Schema:     typefacts.TypeFactsSchemaVersionV2,
			ProjectID:  projectID,
			Generation: generation,
			Table:      typefacts.FactTableV2From(*table, projectID, generation),
		}
		stats := incremental.Stats()
		if stats.Retention.RetainedFiles > 0 {
			retainedSeen = true
		}
		if stats.Retention.RetainedAsyncFiles > 0 {
			asyncCacheSeen = true
		}
		if stats.Retention.CachedSymbolFacts > 0 {
			symbolCacheSeen = true
		}
		if stats.Retention.CachedReferenceFacts > 0 {
			referenceCacheSeen = true
		}
		if stats.Retention.PatchedSymbolRows > 0 {
			patchedSymbolRowsSeen = true
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
		freshResponse, err := fresh.DemandResponseFor(ctx, projectID, generation, freshDemands)
		if err != nil {
			t.Fatal(err)
		}
		freshStats := fresh.Stats()
		if freshStats.Retention.RetainedFiles != 0 {
			t.Fatalf("step %d: fresh session retained %d files; the oracle must be a whole-batch run", step, freshStats.Retention.RetainedFiles)
		}

		retainedBytes, err := wirecbor.Marshal(retainedResponse)
		if err != nil {
			t.Fatal(err)
		}
		freshBytes, err := wirecbor.Marshal(freshResponse)
		if err != nil {
			t.Fatal(err)
		}
		if string(retainedBytes) != string(freshBytes) {
			describe := func(table *typefacts.FactTableV2) string {
				references := 0
				descriptors := 0
				for _, symbol := range table.Symbols {
					references += len(symbol.References)
				}
				for _, entity := range table.Entities {
					if entity.TypeDescriptor != nil {
						descriptors++
					}
				}
				return fmt.Sprintf("entities=%d symbols=%d references=%d descriptors=%d files=%d",
					len(table.Entities), len(table.Symbols), references, descriptors, len(table.Files))
			}
			firstMismatch := func(left, right *typefacts.FactTableV2) string {
				for index := range min(len(left.Sources), len(right.Sources)) {
					l, _ := wirecbor.Marshal(left.Sources[index])
					r, _ := wirecbor.Marshal(right.Sources[index])
					if string(l) != string(r) {
						return fmt.Sprintf("sources[%d]:\n  retained %+v\n  fresh    %+v", index, left.Sources[index].Path, right.Sources[index].Path)
					}
				}
				for index := range min(len(left.Entities), len(right.Entities)) {
					l, _ := wirecbor.Marshal(left.Entities[index])
					r, _ := wirecbor.Marshal(right.Entities[index])
					if string(l) != string(r) {
						return fmt.Sprintf("entities[%d]:\n  retained %+v\n  fresh    %+v", index, left.Entities[index], right.Entities[index])
					}
				}
				for index := range min(len(left.Symbols), len(right.Symbols)) {
					l, _ := wirecbor.Marshal(left.Symbols[index])
					r, _ := wirecbor.Marshal(right.Symbols[index])
					if string(l) != string(r) {
						return fmt.Sprintf("symbols[%d]:\n  retained %+v\n  fresh    %+v", index, left.Symbols[index], right.Symbols[index])
					}
				}
				for index := range min(len(left.Files), len(right.Files)) {
					l, _ := wirecbor.Marshal(left.Files[index])
					r, _ := wirecbor.Marshal(right.Files[index])
					if string(l) != string(r) {
						return fmt.Sprintf("files[%d]:\n  retained %+v\n  fresh    %+v", index, left.Files[index], right.Files[index])
					}
				}
				return "no element-level mismatch found (header fields?)"
			}
			t.Fatalf("step %d (generation %d): retained table diverges from fresh materialization (%d vs %d bytes)\nretained: %s\nfresh:    %s\nfirst mismatch: %s",
				step, generation, len(retainedBytes), len(freshBytes),
				describe(&retainedResponse.Table), describe(&freshResponse.Table),
				firstMismatch(&retainedResponse.Table, &freshResponse.Table))
		}
	}
	if !retainedSeen {
		t.Fatal("the edit script never exercised retention; the test is vacuous")
	}
	if !asyncCacheSeen {
		t.Fatal("the edit script never reused unchanged async file facts; the async-cache parity check is vacuous")
	}
	if !symbolCacheSeen {
		t.Fatal("the edit script never reused a durable symbol fact; the symbol-cache parity check is vacuous")
	}
	if !referenceCacheSeen {
		t.Fatal("the edit script never reused an unchanged reference list; the reference-delta parity check is vacuous")
	}
	if !patchedSymbolRowsSeen {
		t.Fatal("the edit script never patched a retained canonical symbol table; the row-retention parity check is vacuous")
	}
}
