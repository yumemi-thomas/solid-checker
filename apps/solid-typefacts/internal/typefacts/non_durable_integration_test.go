package typefacts_test

import (
	"bytes"
	"context"
	"encoding/binary"
	"os"
	"path/filepath"
	"sort"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

func tableTransitionMode(t *testing.T, transition []byte) uint64 {
	t.Helper()
	_, width := binary.Uvarint(transition)
	if width <= 0 {
		t.Fatal("table transition has no version")
	}
	mode, width := binary.Uvarint(transition[width:])
	if width <= 0 {
		t.Fatal("table transition has no mode")
	}
	return mode
}

// nonDurableDemands asks for a symbol at every property name after a dot, plus
// every binding name. The mapped-type accesses in the fixture resolve to
// synthesized symbols, which is what makes the file non-durable.
func nonDurableDemands(t *testing.T, root string) []typefacts.EntityDemand {
	t.Helper()
	var demands []typefacts.EntityDemand
	names, err := filepath.Glob(filepath.Join(root, "*.ts"))
	if err != nil {
		t.Fatal(err)
	}
	sort.Strings(names)
	for _, name := range names {
		source, err := os.ReadFile(name)
		if err != nil {
			t.Fatal(err)
		}
		for offset := 0; ; {
			dot := bytes.IndexByte(source[offset:], '.')
			if dot < 0 {
				break
			}
			start := offset + dot + 1
			end := start
			for end < len(source) && (source[end] == '_' ||
				(source[end] >= 'a' && source[end] <= 'z') ||
				(source[end] >= 'A' && source[end] <= 'Z') ||
				(source[end] >= '0' && source[end] <= '9')) {
				end++
			}
			if end > start {
				demands = append(demands, typefacts.EntityDemand{
					Location:   typefacts.Location{Path: name, StartByte: start, EndByte: end},
					Symbol:     true,
					References: true,
				})
			}
			offset = start
		}
	}
	if len(demands) == 0 {
		t.Fatal("the fixture produced no property demands")
	}
	return demands
}

func openNonDurableSession(t *testing.T, root string) (*typefacts.Session, string) {
	t.Helper()
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(context.Background(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	session, err := typefacts.NewSession(backend, projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })
	return session, projectID
}

func nonDurableRequest(id uint64, operation typefacts.LifecycleOperation, projectID string, generation uint64) typefacts.LifecycleRequest {
	return typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV1,
		RequestID:  id,
		Operation:  operation,
		ProjectID:  projectID,
		Generation: generation,
	}
}

// TestNonDurableFilesStillGetADelta covers the cost cliff a single non-durable
// file used to impose on every edit.
//
// A mapped-type property access resolves to a synthesized symbol, so the
// producer mints a generation-scoped identity for it and the file can never be
// retained. That is unavoidable. What was avoidable is the consequence: because
// a recomputed-but-unnamed file's rows never reached the transport manifest, the
// producer fell back to packing the entire table — on every keystroke, for the
// whole project, because of one file. A delta can describe re-minted identities
// perfectly well; it just has to be told which paths moved.
func TestNonDurableFilesStillGetADelta(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.Abs(filepath.Join("testdata", "non-durable"))
	if err != nil {
		t.Fatal(err)
	}
	session, projectID := openNonDurableSession(t, root)
	demands := nonDurableDemands(t, root)

	cold := nonDurableRequest(1, typefacts.LifecycleAnalyze, projectID, 1)
	cold.ResetState = true
	cold.Demands = demands
	first := session.Lifecycle(ctx, cold)
	if !first.OK || len(first.TableTransition) == 0 || tableTransitionMode(t, first.TableTransition) != 0 {
		t.Fatalf("cold analyze = %+v", first)
	}
	if first.Timings == nil || first.Timings.NonDurableFiles == 0 {
		t.Fatalf("the fixture is durable, so this test proves nothing; timings = %+v", first.Timings)
	}
	t.Logf("non-durable files in the fixture: %d", first.Timings.NonDurableFiles)
	token := first.StateToken

	islandPath := filepath.Clean(filepath.Join(root, "island.ts"))
	original, err := os.ReadFile(islandPath)
	if err != nil {
		t.Fatal(err)
	}

	// Several edits, so this covers the steady state rather than one transition.
	generation := uint64(1)
	requestID := uint64(1)
	for edit := 1; edit <= 3; edit++ {
		generation++
		requestID++
		source := append([]byte(nil), original...)
		source = append(source, []byte("\nexport const edit"+string(rune('0'+edit))+" = 1;\n")...)
		update := nonDurableRequest(requestID, typefacts.LifecycleUpdate, projectID, generation)
		update.Changes = []typefacts.FileChangeV3{{Path: islandPath, Version: uint64(edit), Source: source}}
		if response := session.Lifecycle(ctx, update); !response.OK {
			t.Fatalf("update %d = %+v", edit, response)
		}

		requestID++
		analyze := nonDurableRequest(requestID, typefacts.LifecycleAnalyze, projectID, generation)
		analyze.StateToken = token
		analyzed := session.Lifecycle(ctx, analyze)
		if !analyzed.OK {
			t.Fatalf("analyze %d = %+v", edit, analyzed)
		}
		token = analyzed.StateToken

		if analyzed.Timings == nil || analyzed.Timings.NonDurableFiles == 0 {
			t.Fatalf("edit %d stopped being non-durable; the cliff is no longer under test", edit)
		}
		if len(analyzed.TableTransition) == 0 || tableTransitionMode(t, analyzed.TableTransition) != 1 {
			t.Fatalf(
				"edit %d did not carry a delta because %d file(s) are non-durable; "+
					"one synthesized symbol must not cost a project-wide pack per keystroke",
				edit, analyzed.Timings.NonDurableFiles)
		}
	}
}

// TestNonDurableRetentionMatchesFreshMaterialization is the correctness half:
// retained contributions must reproduce a fresh whole-batch table even when
// identities inside them were re-minted this generation.
func TestNonDurableRetentionMatchesFreshMaterialization(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.Abs(filepath.Join("testdata", "non-durable"))
	if err != nil {
		t.Fatal(err)
	}
	projectID := filepath.Join(root, "tsconfig.json")

	openClosure := func() *typefacts.DemandClosure {
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
		return closure
	}

	demands := nonDurableDemands(t, root)
	islandPath := filepath.Clean(filepath.Join(root, "island.ts"))
	original, err := os.ReadFile(islandPath)
	if err != nil {
		t.Fatal(err)
	}
	edit := typefacts.FileChange{
		Path:    islandPath,
		Version: 1,
		Source:  append([]byte("// non-durable delta edit\n"), original...),
	}

	incremental := openClosure()
	_, err = incremental.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands))
	if err != nil {
		t.Fatal(err)
	}
	if incremental.Stats().Retention.NonDurableFiles == 0 {
		t.Fatal("the fixture is durable, so this test proves nothing")
	}

	if _, err := incremental.Update(ctx, []typefacts.FileChange{edit}); err != nil {
		t.Fatal(err)
	}
	table, err := incremental.DemandTableForGroups(ctx, 2, groupedDemands(demands), nil)
	if err != nil {
		t.Fatal(err)
	}
	fresh := openClosure()
	if _, err := fresh.Update(ctx, []typefacts.FileChange{edit}); err != nil {
		t.Fatal(err)
	}
	freshTable, err := fresh.DemandTableForGroups(ctx, 2, canonicalDemandGroups(demands), nil)
	if err != nil {
		t.Fatal(err)
	}
	assertFullWireTransitionsIdentical(t, "retained table with non-durable files", 0, projectID, table, freshTable)
}
