package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

// The source-fact memo and durable symbol identity are exercised through the
// typefacts.Project interface; only reuse/eviction assertions reach into the
// project internals.
func TestSourceFactsMemoAcrossGenerations(t *testing.T) {
	dir := t.TempDir()
	write := func(name, source string) string {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	aPath := write("a.ts", "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n")
	bPath := write("b.ts", "export function makeThing(): number {\n  return 1;\n}\n")
	cPath := write("c.ts", "export const unrelated = 1;\n")

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	proj := opened.(*project)
	discoverer := opened.(typefacts.CallDiscoverer)

	calls, err := discoverer.SourceCalls(ctx, aPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected one source call in a.ts, got %d", len(calls))
	}
	target := calls[0].Target
	if !strings.HasPrefix(string(target), "symbol:h:") {
		t.Fatalf("expected a durable symbol ID, got %q", target)
	}
	declarations, err := opened.Declarations(ctx, target)
	if err != nil {
		t.Fatal(err)
	}
	if len(declarations) != 1 || filepath.Base(declarations[0].Location.Path) != "b.ts" {
		t.Fatalf("expected makeThing to declare in b.ts, got %+v", declarations)
	}

	t.Run("unrelated update keeps the memo and resolves held IDs", func(t *testing.T) {
		affected, err := opened.Update(ctx, []typefacts.FileChange{{Path: cPath, Version: 1, Source: []byte("export const unrelated = 2;\n")}})
		if err != nil {
			t.Fatal(err)
		}
		for _, path := range affected.Files {
			if filepath.Base(path) == "a.ts" {
				t.Fatalf("a.ts unexpectedly in affected set %v", affected.Files)
			}
		}
		memo, ok := proj.sourceFactsMemo[aPath]
		if !ok || !memo.hasCalls {
			t.Fatal("memo entry for a.ts did not survive an unrelated update")
		}
		reused, err := discoverer.SourceCalls(ctx, aPath)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(reused, calls) {
			t.Fatalf("memoized calls diverged:\nbefore %+v\nafter  %+v", calls, reused)
		}
		// The held ID must re-resolve in the new generation.
		redeclared, err := opened.Declarations(ctx, target)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(redeclared, declarations) {
			t.Fatalf("declarations diverged across generations:\nbefore %+v\nafter  %+v", declarations, redeclared)
		}
	})

	t.Run("shape-preserving imported body edit retains the importer", func(t *testing.T) {
		affected, err := opened.Update(ctx, []typefacts.FileChange{{Path: bPath, Version: 1, Source: []byte("export function makeThing(): number {\n  return 2;\n}\n")}})
		if err != nil {
			t.Fatal(err)
		}
		var sawImporter bool
		for _, path := range affected.Files {
			if filepath.Base(path) == "a.ts" {
				sawImporter = true
			}
		}
		if sawImporter {
			t.Fatalf("shape-preserving edit unexpectedly affected a.ts: %v", affected.Files)
		}
		if _, ok := proj.sourceFactsMemo[aPath]; !ok {
			t.Fatal("memo entry for a.ts did not survive a shape-preserving edit")
		}
		reused, err := discoverer.SourceCalls(ctx, aPath)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(reused, calls) {
			t.Fatalf("memoized calls diverged:\nbefore %+v\nafter  %+v", calls, reused)
		}
		// The declaration's name span is unchanged, so the durable identity is too.
		if reused[0].Target != target {
			t.Fatalf("durable ID changed for an unmoved declaration: %q -> %q", target, reused[0].Target)
		}
	})

	t.Run("exported signature change evicts the importer", func(t *testing.T) {
		affected, err := opened.Update(ctx, []typefacts.FileChange{{Path: bPath, Version: 2, Source: []byte("export function makeThing(): string {\n  return \"changed\";\n}\n")}})
		if err != nil {
			t.Fatal(err)
		}
		var sawImporter bool
		for _, path := range affected.Files {
			if filepath.Base(path) == "a.ts" {
				sawImporter = true
			}
		}
		if !sawImporter {
			t.Fatalf("expected a.ts in affected set, got %v", affected.Files)
		}
		if _, ok := proj.sourceFactsMemo[aPath]; ok {
			t.Fatal("memo entry for a.ts survived an exported signature change")
		}
	})

	t.Run("multi-file update clears the memo wholesale", func(t *testing.T) {
		if _, err := discoverer.SourceCalls(ctx, aPath); err != nil {
			t.Fatal(err)
		}
		if len(proj.sourceFactsMemo) == 0 {
			t.Fatal("expected memo entries before the multi-file update")
		}
		_, err := opened.Update(ctx, []typefacts.FileChange{
			{Path: cPath, Version: 3, Source: []byte("export const unrelated = 3;\n")},
			{Path: bPath, Version: 3, Source: []byte("export function makeThing(): number {\n  return 3;\n}\n")},
		})
		if err != nil {
			t.Fatal(err)
		}
		if len(proj.sourceFactsMemo) != 0 {
			t.Fatalf("expected an empty memo after a full rebuild, found %d entries", len(proj.sourceFactsMemo))
		}
		if _, err := opened.Declarations(ctx, target); err != nil {
			t.Fatalf("durable ID failed to re-resolve after a full rebuild: %v", err)
		}
	})
}
