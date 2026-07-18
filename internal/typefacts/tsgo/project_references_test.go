package tsgo

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

func writeProject(t *testing.T, dir string) func(name, source string) string {
	t.Helper()
	return func(name, source string) string {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
		return path
	}
}

func referencesSorted(locations []typefacts.Location) bool {
	return sort.SliceIsSorted(locations, func(i, j int) bool {
		if locations[i].Path != locations[j].Path {
			return locations[i].Path < locations[j].Path
		}
		return locations[i].StartByte < locations[j].StartByte
	})
}

// The reference index is exercised through the typefacts.Project interface;
// only reuse/eviction assertions reach into the project internals, mirroring
// the source-fact memo tests.
func TestReferenceIndexAcrossGenerations(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	aPath := write("a.ts", "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n")
	bPath := write("b.ts", "export function makeThing(): number {\n  return 1;\n}\nexport const local = makeThing();\n")
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

	references, err := opened.References(ctx, target)
	if err != nil {
		t.Fatal(err)
	}
	if len(references) != 2 {
		t.Fatalf("expected references in a.ts and b.ts, got %+v", references)
	}
	if filepath.Base(references[0].Path) != "a.ts" || filepath.Base(references[1].Path) != "b.ts" {
		t.Fatalf("references out of path order: %+v", references)
	}
	if !referencesSorted(references) {
		t.Fatalf("references violate the ordering contract: %+v", references)
	}
	cleanA, cleanB := filepath.Clean(aPath), filepath.Clean(bPath)
	entryA, entryB := proj.referenceFiles[cleanA], proj.referenceFiles[cleanB]
	if entryA == nil || entryB == nil {
		t.Fatalf("expected per-file contributions for a.ts and b.ts, got %v", proj.referenceFiles)
	}

	t.Run("unrelated update keeps contributions and answers", func(t *testing.T) {
		if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: cPath, Version: 1, Source: []byte("export const unrelated = 2;\n")}}); err != nil {
			t.Fatal(err)
		}
		if proj.referenceFiles[cleanA] != entryA || proj.referenceFiles[cleanB] != entryB {
			t.Fatal("contributions of unaffected files did not survive an unrelated update")
		}
		reused, err := opened.References(ctx, target)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(reused, references) {
			t.Fatalf("references diverged across generations:\nbefore %+v\nafter  %+v", references, reused)
		}
	})

	t.Run("shape-preserving declaring-file edit retains its importers", func(t *testing.T) {
		if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: bPath, Version: 1, Source: []byte("export function makeThing(): number {\n  return 2;\n}\nexport const local = makeThing();\n")}}); err != nil {
			t.Fatal(err)
		}
		if proj.referenceFiles[cleanA] != entryA {
			t.Fatal("contribution of a.ts did not survive a shape-preserving edit")
		}
		if _, ok := proj.referenceFiles[cleanB]; ok {
			t.Fatal("contribution of b.ts survived its own edit")
		}
		recomputed, err := opened.References(ctx, target)
		if err != nil {
			t.Fatal(err)
		}
		// The edit does not move the declaration or the references.
		if !reflect.DeepEqual(recomputed, references) {
			t.Fatalf("recomputed references diverged:\nbefore %+v\nafter  %+v", references, recomputed)
		}
		if refreshed := proj.referenceFiles[cleanB]; refreshed == nil || refreshed == entryB {
			t.Fatal("contribution of b.ts was not lazily replaced after its own edit")
		}
		if proj.referenceFiles[filepath.Clean(cPath)] == nil {
			t.Fatal("expected the unaffected c.ts contribution to be retained")
		}

		// A fresh project over the same tree must answer the same durable
		// ID with byte-identical locations.
		fresh, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
		if err != nil {
			t.Fatal(err)
		}
		defer fresh.Close()
		if _, err := fresh.(typefacts.CallDiscoverer).SourceCalls(ctx, aPath); err != nil {
			t.Fatal(err)
		}
		independent, err := fresh.References(ctx, target)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(independent, recomputed) {
			t.Fatalf("memoized index diverged from a fresh scan:\nfresh    %+v\nretained %+v", independent, recomputed)
		}
	})

	t.Run("multi-file update clears the contributions wholesale", func(t *testing.T) {
		if _, err := opened.References(ctx, target); err != nil {
			t.Fatal(err)
		}
		if len(proj.referenceFiles) == 0 {
			t.Fatal("expected contributions before the multi-file update")
		}
		_, err := opened.Update(ctx, []typefacts.FileChange{
			{Path: cPath, Version: 2, Source: []byte("export const unrelated = 3;\n")},
			{Path: bPath, Version: 2, Source: []byte("export function makeThing(): number {\n  return 3;\n}\nexport const local = makeThing();\n")},
		})
		if err != nil {
			t.Fatal(err)
		}
		if len(proj.referenceFiles) != 0 {
			t.Fatalf("expected no contributions after a full rebuild, found %d", len(proj.referenceFiles))
		}
		rebuilt, err := opened.References(ctx, target)
		if err != nil {
			t.Fatal(err)
		}
		if !reflect.DeepEqual(rebuilt, references) {
			t.Fatalf("references diverged after a full rebuild:\nbefore %+v\nafter  %+v", references, rebuilt)
		}
	})
}

// A durable ID whose declaration disappears must fail closed even though
// retained contributions may still mention it.
func TestReferencesFailClosedWhenDeclarationDisappears(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	aPath := write("a.ts", "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\n")
	bPath := write("b.ts", "export function makeThing(): number {\n  return 1;\n}\n")

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	discoverer := opened.(typefacts.CallDiscoverer)

	calls, err := discoverer.SourceCalls(ctx, aPath)
	if err != nil {
		t.Fatal(err)
	}
	target := calls[0].Target
	if _, err := opened.References(ctx, target); err != nil {
		t.Fatal(err)
	}

	if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: bPath, Version: 1, Source: []byte("export function makeOther(): number {\n  return 1;\n}\n")}}); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.References(ctx, target); !errors.Is(err, typefacts.ErrNotFound) {
		t.Fatalf("expected ErrNotFound for a vanished declaration, got %v", err)
	}
}

// A file that leaves the program on an incremental update must stop
// contributing references even when it is not in the affected set.
func TestReferencesDropFileThatLeftProgram(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["main.ts"]}`)
	write("lib.ts", "export function helper(): number {\n  return 1;\n}\n")
	write("dep.ts", "import { helper } from \"./lib\";\nexport const fromDep = helper();\n")
	mainPath := write("main.ts", "import { helper } from \"./lib\";\nimport { fromDep } from \"./dep\";\nexport const value = helper() + fromDep;\n")

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	discoverer := opened.(typefacts.CallDiscoverer)

	calls, err := discoverer.SourceCalls(ctx, mainPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(calls) != 1 {
		t.Fatalf("expected one call in main.ts, got %d", len(calls))
	}
	target := calls[0].Target
	before, err := opened.References(ctx, target)
	if err != nil {
		t.Fatal(err)
	}
	sawDep := false
	for _, location := range before {
		if filepath.Base(location.Path) == "dep.ts" {
			sawDep = true
		}
	}
	if !sawDep {
		t.Fatalf("expected a dep.ts reference before the import is removed, got %+v", before)
	}

	depPath := filepath.Join(dir, "dep.ts")
	if _, err := discoverer.SourceCalls(ctx, depPath); err != nil {
		t.Fatal(err)
	}

	// Dropping main's dep import removes dep.ts from the program without
	// putting dep.ts in the affected set.
	if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: mainPath, Version: 1, Source: []byte("import { helper } from \"./lib\";\nexport const value = helper();\n")}}); err != nil {
		t.Fatal(err)
	}
	proj := opened.(*project)
	if _, ok := proj.referenceFiles[filepath.Clean(depPath)]; ok {
		t.Fatal("reference contribution of a departed file survived the update")
	}
	// The source-fact memo evicts departed files the same way: dep.ts must
	// answer like a file the program does not contain, not from the memo.
	if _, err := discoverer.SourceCalls(ctx, depPath); !errors.Is(err, typefacts.ErrNotFound) {
		t.Fatalf("expected ErrNotFound for source calls of a departed file, got %v", err)
	}
	after, err := opened.References(ctx, target)
	if err != nil {
		t.Fatal(err)
	}
	for _, location := range after {
		if filepath.Base(location.Path) == "dep.ts" {
			t.Fatalf("reference in departed file survived: %+v", after)
		}
	}
	if len(after) == 0 {
		t.Fatalf("expected the main.ts reference to remain, got none")
	}
}
