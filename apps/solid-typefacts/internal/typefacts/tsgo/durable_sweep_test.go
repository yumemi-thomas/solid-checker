package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0364: no identity bookkeeping survives an update. A held ID of any file,
// departed or living, stops resolving until a location scan re-mints it, so a
// stale answer cannot be matched to a new generation and a long session
// cannot accrete every file it ever saw.
func TestUpdateDropsDurableIdentities(t *testing.T) {
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

	// A delete forces the full-rebuild update path.
	if _, err := backend.Update(ctx, []typefacts.FileChange{{Path: doomedPath, Version: 1, Deleted: true}}); err != nil {
		t.Fatal(err)
	}
	if after := refPaths(); len(after) != 0 {
		t.Fatalf("durable identities survived the update: %v", after)
	}
	// A living file re-mints its identity through a location scan.
	if _, err := backend.SymbolAt(ctx, typefacts.Location{Path: keptPath, StartByte: 13, EndByte: 17}); err != nil {
		t.Fatal(err)
	}
	if !refPaths()[filepath.Clean(keptPath)] {
		t.Fatal("a living file could not re-mint its durable identity")
	}
}
