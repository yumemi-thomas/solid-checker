package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

// TestRangeEntitiesProjectsARealRetainedAnswer demands one symbol through the
// real backend and reads the answer back through the retained table the
// closure returned, whose public Entities slice is empty.
func TestRangeEntitiesProjectsARealRetainedAnswer(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	source := "export const exact = 1;\nexport const other = exact;\n"
	for name, contents := range map[string]string{
		"tsconfig.json": `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","types":[]},"include":["*.ts"]}`,
		"main.ts":       source,
	} {
		if err := os.WriteFile(filepath.Join(root, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	path := filepath.Join(root, "main.ts")
	start := strings.LastIndex(source, "exact")
	demand := typefacts.EntityDemand{
		Location: typefacts.Location{Path: path, StartByte: start, EndByte: start + len("exact")},
		Symbol:   true,
	}

	backend, err := tsgo.OpenProject(ctx, filepath.Join(root, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer closure.Close()
	table, err := closure.DemandTableForGroups(ctx, 1,
		[]typefacts.DemandGroup{{Path: path, Demands: []typefacts.EntityDemand{demand}}},
		[]string{path})
	if err != nil {
		t.Fatal(err)
	}
	if table.Entities != nil {
		t.Fatalf("the public slice carries %d rows; the retained path did not run", len(table.Entities))
	}
	var answered []typefacts.EntityFact
	table.RangeEntities(func(entity typefacts.EntityFact) {
		if entity.Location == demand.Location {
			answered = append(answered, entity)
		}
	})
	if len(answered) != 1 || answered[0].Symbol == "" || answered[0].SymbolUnresolved {
		t.Fatalf("demand at %+v answered %+v", demand.Location, answered)
	}
}
