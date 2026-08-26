package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// A deleted file's identities can never re-resolve, and re-entry re-mints
// them through location scans — so the full-rebuild update path must drop
// them instead of letting a long session accrete every file it ever saw.
// Identities of files still in the program must survive the same sweep.
func TestFullRebuildSweepsDepartedFilesDurableIdentities(t *testing.T) {
	dir := t.TempDir()
	write := func(name, source string) string {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o600); err != nil {
			t.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	keptPath := write("kept.ts", "export const kept = 1\n")
	doomedPath := write("doomed.ts", "export const doomed = 2\n")

	ctx := context.Background()
	backend, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = backend.Close() })
	project := backend.(*project)

	// Mint durable identities in both files ("kept"/"doomed" name spans).
	for _, location := range []typefacts.Location{
		{Path: keptPath, StartByte: 13, EndByte: 17},
		{Path: doomedPath, StartByte: 13, EndByte: 19},
	} {
		if _, err := backend.SymbolAt(ctx, location); err != nil {
			t.Fatalf("mint identity at %+v: %v", location, err)
		}
	}
	refPaths := func() map[string]bool {
		paths := make(map[string]bool)
		for _, ref := range project.durableRefs {
			paths[ref.path] = true
		}
		return paths
	}
	before := refPaths()
	if !before[filepath.Clean(keptPath)] || !before[filepath.Clean(doomedPath)] {
		t.Fatalf("identities were not minted for both files: %v", before)
	}

	// A delete forces the full-rebuild update path, which sweeps.
	if _, err := backend.Update(ctx, []typefacts.FileChange{{Path: doomedPath, Version: 1, Deleted: true}}); err != nil {
		t.Fatal(err)
	}
	after := refPaths()
	if after[filepath.Clean(doomedPath)] {
		t.Fatal("the deleted file's durable identities survived the sweep")
	}
	if !after[filepath.Clean(keptPath)] {
		t.Fatal("a living file's durable identities were swept")
	}
}
