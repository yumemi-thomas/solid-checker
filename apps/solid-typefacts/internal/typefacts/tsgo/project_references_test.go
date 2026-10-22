package tsgo

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestReleaseAnalysisStateRehydratesDurableIdentity(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	path := write("source.ts", "export function target() { return 1 }\nexport const value = target();\n")
	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	project := opened.(*project)
	source := "export function target() { return 1 }\nexport const value = target();\n"
	start := strings.LastIndex(source, "target")
	location := typefacts.Location{Path: path, StartByte: start, EndByte: start + len("target")}

	before, err := opened.SymbolAt(ctx, location)
	if err != nil {
		t.Fatal(err)
	}
	if !typefacts.DurableSymbolID(before) {
		t.Fatalf("symbol %q is not durable", before)
	}
	project.ReleaseAnalysisState()
	if project.checker != nil || len(project.idsBySymbol) != 0 || len(project.symbolsByID) != 0 {
		t.Fatal("checker-owned state survived ReleaseAnalysisState")
	}

	after, err := opened.SymbolAt(ctx, location)
	if err != nil {
		t.Fatal(err)
	}
	if after != before {
		t.Fatalf("rehydrated symbol = %q, want %q", after, before)
	}
}

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
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
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
	entryA, entryB := proj.referenceIndex.files[cleanA], proj.referenceIndex.files[cleanB]
	if entryA == nil || entryB == nil {
		t.Fatalf("expected per-file contributions for a.ts and b.ts, got %v", proj.referenceIndex.files)
	}
	for path, entry := range proj.referenceIndex.files {
		for _, group := range entry.refs {
			if group.spans != nil {
				t.Fatalf("merged index still duplicates retained spans for %s", path)
			}
		}
	}

	t.Run("unrelated update keeps contributions and answers", func(t *testing.T) {
		if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: cPath, Version: 1, Source: []byte("export const unrelated = 2;\n")}}); err != nil {
			t.Fatal(err)
		}
		if proj.referenceIndex.files[cleanA] != entryA || proj.referenceIndex.files[cleanB] != entryB {
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
		if proj.referenceIndex.files[cleanA] != entryA {
			t.Fatal("contribution of a.ts did not survive a shape-preserving edit")
		}
		if _, ok := proj.referenceIndex.files[cleanB]; ok {
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
		if refreshed := proj.referenceIndex.files[cleanB]; refreshed == nil || refreshed == entryB {
			t.Fatal("contribution of b.ts was not lazily replaced after its own edit")
		}
		if proj.referenceIndex.files[filepath.Clean(cPath)] == nil {
			t.Fatal("expected the unaffected c.ts contribution to be retained")
		}

		// A fresh project over the same tree must answer the same durable
		// ID with byte-identical locations.
		fresh, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
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
		if len(proj.referenceIndex.files) == 0 {
			t.Fatal("expected contributions before the multi-file update")
		}
		_, err := opened.Update(ctx, []typefacts.FileChange{
			{Path: cPath, Version: 2, Source: []byte("export const unrelated = 3;\n")},
			{Path: bPath, Version: 2, Source: []byte("export function makeThing(): number {\n  return 3;\n}\nexport const local = makeThing();\n")},
		})
		if err != nil {
			t.Fatal(err)
		}
		if len(proj.referenceIndex.files) != 0 {
			t.Fatalf("expected no contributions after a full rebuild, found %d", len(proj.referenceIndex.files))
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

func TestReferenceIndexBroadInvalidationKeepsExactSymbolEvidence(t *testing.T) {
	const files = 65
	index := referenceIndex{
		merged:         make(map[typefacts.SymbolID][]indexedReference, files),
		changedSymbols: make(map[typefacts.SymbolID]struct{}),
		files:          make(map[string]*fileReferences, files),
	}
	affected := make([]string, 0, files)
	for file := range files {
		path := filepath.Clean(filepath.Join("/project", fmt.Sprintf("file-%02d.ts", file)))
		id := typefacts.SymbolID(fmt.Sprintf("symbol-%02d", file))
		index.paths = append(index.paths, path)
		index.files[path] = &fileReferences{
			refs: []fileReferenceGroup{{
				id:    id,
				spans: []fileReferenceSpan{{start: 1, end: 2}},
			}},
			durable: true,
		}
		index.merged[id] = []indexedReference{{path: uint32(file), start: 1, end: 2}}
		affected = append(affected, path)
	}

	index.invalidate(nil, affected, func(string) bool { return false })

	if index.merged != nil {
		t.Fatal("broad invalidation should select a full merged-index rebuild")
	}
	if !index.deltaExact {
		t.Fatal("broad rebuild discarded exact changed-symbol evidence")
	}
	if len(index.changedSymbols) != files {
		t.Fatalf("changed symbols = %d, want %d", len(index.changedSymbols), files)
	}
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
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
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
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
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
	if _, ok := proj.referenceIndex.files[filepath.Clean(depPath)]; ok {
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

// A shorthand property `{ makeThing }` reads the value binding: it is a
// reference of the function, in its own file and through an import.
func TestShorthandPropertyIsAReferenceOfItsValue(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	aSource := "import { makeThing } from \"./b\";\n\nexport const value = makeThing();\nexport const held = { makeThing };\n"
	aPath := write("a.ts", aSource)
	bSource := "export function makeThing(): number {\n  return 1;\n}\nexport const local = { makeThing };\n"
	bPath := write("b.ts", bSource)

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	calls, err := opened.(typefacts.CallDiscoverer).SourceCalls(ctx, aPath)
	if err != nil || len(calls) != 1 {
		t.Fatalf("expected one source call in a.ts, got %d (%v)", len(calls), err)
	}
	references, err := opened.References(ctx, calls[0].Target)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]int{
		filepath.Clean(aPath): strings.LastIndex(aSource, "makeThing"),
		filepath.Clean(bPath): strings.LastIndex(bSource, "makeThing"),
	}
	for path, start := range want {
		found := false
		for _, reference := range references {
			if filepath.Clean(reference.Path) == path && reference.StartByte == start {
				found = true
			}
		}
		if !found {
			t.Fatalf("shorthand at %s:%d is not a reference: %+v", path, start, references)
		}
	}
}

// A broad invalidation (more than 64 affected files) drops the merged index
// but keeps contributions of unaffected files; the rebuilt index must still
// answer with their references, exactly as a fresh open does.
func TestReferenceIndexBroadInvalidationKeepsRetainedReferences(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	hubPath := write("hub.ts", "export const hub = 1;\n")
	for i := 0; i < 70; i++ {
		write(fmt.Sprintf("imp%02d.ts", i), "import { hub } from \"./hub\";\nexport const v = hub;\n")
	}
	keptPath := write("kept.ts", "export const shared = 1;\nexport const a = shared;\nexport const b = shared + shared;\n")

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	proj := opened.(*project)

	keptSource := "export const shared = 1;\nexport const a = shared;\nexport const b = shared + shared;\n"
	start := strings.Index(keptSource, "shared")
	shared, err := opened.SymbolAt(ctx, typefacts.Location{Path: keptPath, StartByte: start, EndByte: start + len("shared")})
	if err != nil {
		t.Fatal(err)
	}
	before, err := opened.References(ctx, shared)
	if err != nil {
		t.Fatal(err)
	}
	if len(before) != 3 {
		t.Fatalf("expected 3 references before the update, got %+v", before)
	}

	// Changing the exported type of hub is not a semantic cutoff, so all 70
	// importers are affected while kept.ts is not.
	if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: hubPath, Version: 1, Source: []byte("export const hub = \"s\";\n")}}); err != nil {
		t.Fatal(err)
	}
	if proj.referenceIndex.merged != nil || proj.referenceIndex.files[filepath.Clean(keptPath)] == nil {
		t.Skip("update did not take the broad retained path")
	}
	after, err := opened.References(ctx, shared)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(after, before) {
		t.Fatalf("broad invalidation lost retained references:\nbefore %+v\nafter  %+v", before, after)
	}
}

// ReleaseAnalysisState prunes merged buckets no retained fact used. A symbol
// first asked about after a release must still get its complete references,
// both directly and after an incremental update patched the pruned index.
func TestReferenceIndexAfterReleaseAnswersPrunedSymbol(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	declSource := "export const used = 1;\nexport const pruned = 2;\n"
	declPath := write("decl.ts", declSource)
	aSource := "import { used, pruned } from \"./decl\";\nexport const a = used + pruned;\n"
	aPath := write("a.ts", aSource)
	write("b.ts", "import { pruned } from \"./decl\";\nexport const b = pruned;\n")

	ctx := context.Background()
	opened, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	proj := opened.(*project)
	at := func(path, source, name string) typefacts.SymbolID {
		start := strings.Index(source, name)
		id, err := opened.SymbolAt(ctx, typefacts.Location{Path: path, StartByte: start, EndByte: start + len(name)})
		if err != nil {
			t.Fatal(err)
		}
		return id
	}
	usedID := at(declPath, declSource, "used")
	prunedID := at(declPath, declSource, "pruned")
	if _, err := opened.References(ctx, usedID); err != nil {
		t.Fatal(err)
	}
	want, err := opened.References(ctx, prunedID)
	if err != nil {
		t.Fatal(err)
	}
	if len(want) == 0 {
		t.Fatal("expected references to pruned before release")
	}
	// Only `used` is retained by a fact.
	proj.referenceIndex.used = map[typefacts.SymbolID]struct{}{}
	proj.referenceIndex.markUsed(usedID)
	proj.ReleaseAnalysisState()

	got, err := opened.References(ctx, prunedID)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("pruned symbol after release:\nwant %+v\ngot  %+v", want, got)
	}

	// Patching after a release must not leave a partial bucket either.
	proj.referenceIndex.used = map[typefacts.SymbolID]struct{}{}
	proj.referenceIndex.markUsed(usedID)
	proj.ReleaseAnalysisState()
	if _, err := opened.Update(ctx, []typefacts.FileChange{{Path: aPath, Version: 1, Source: []byte(aSource + "export const a2 = pruned;\n")}}); err != nil {
		t.Fatal(err)
	}
	patched, err := opened.References(ctx, prunedID)
	if err != nil {
		t.Fatal(err)
	}
	write("a.ts", aSource+"export const a2 = pruned;\n")
	fresh, err := OpenProject(ctx, filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer fresh.Close()
	freshID := at2(t, fresh, ctx, declPath, declSource, "pruned")
	if freshID != prunedID {
		t.Fatalf("durable id differs: %s vs %s", freshID, prunedID)
	}
	independent, err := fresh.References(ctx, freshID)
	if err != nil {
		t.Fatal(err)
	}
	if len(independent) != 3 || !reflect.DeepEqual(patched, independent) {
		t.Fatalf("patched pruned index diverged from a fresh open:\nfresh   %+v\npatched %+v", independent, patched)
	}
}

func at2(t *testing.T, p typefacts.Project, ctx context.Context, path, source, name string) typefacts.SymbolID {
	t.Helper()
	start := strings.Index(source, name)
	id, err := p.SymbolAt(ctx, typefacts.Location{Path: path, StartByte: start, EndByte: start + len(name)})
	if err != nil {
		t.Fatal(err)
	}
	return id
}
