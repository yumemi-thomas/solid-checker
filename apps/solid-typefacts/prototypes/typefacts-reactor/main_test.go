package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

// decoded is a response read back for assertions. Compared payloads are never
// rebuilt from it.
type decoded struct {
	OK          bool                       `json:"ok"`
	Table       json.RawMessage            `json:"table"`
	ModuleGraph *typefacts.ModuleInventory `json:"moduleGraph"`
	Error       string                     `json:"error"`
}

type decodedTable struct {
	Generation uint64         `json:"generation"`
	ProjectID  string         `json:"projectId"`
	Sources    []sourceDigest `json:"sources"`
}

// legacyResponse is the response type before the module graph existed.
type legacyResponse struct {
	OK    bool            `json:"ok"`
	Table json.RawMessage `json:"table,omitempty"`
	Error string          `json:"error,omitempty"`
}

// forms copies testdata/forms into a fresh realpath-resolved directory, so
// nothing above the project (a repository node_modules, a macOS temporary-root
// symlink) takes part in resolution.
func forms(t *testing.T) string {
	t.Helper()
	temporary, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	root := filepath.Join(temporary, "forms")
	err = filepath.WalkDir("testdata/forms", func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		relative, err := filepath.Rel("testdata/forms", path)
		if err != nil {
			return err
		}
		target := filepath.Join(root, relative)
		if entry.IsDir() {
			return os.MkdirAll(target, 0o755)
		}
		contents, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return os.WriteFile(target, contents, 0o644)
	})
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func call(t *testing.T, request request) ([]byte, decoded) {
	t.Helper()
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	output, status := respond(encoded)
	var answer decoded
	if err := json.Unmarshal(output, &answer); err != nil {
		t.Fatalf("response is not JSON: %v\n%s", err, output)
	}
	if status != 0 || !answer.OK {
		t.Fatalf("status %d, response %s", status, output)
	}
	return output, answer
}

func table(t *testing.T, answer decoded) decodedTable {
	t.Helper()
	var decodedTable decodedTable
	if err := json.Unmarshal(answer.Table, &decodedTable); err != nil {
		t.Fatal(err)
	}
	return decodedTable
}

// answeredEntity requires the table to answer demand with exactly one entity
// at its exact location, carrying a resolved, nonempty symbol.
func answeredEntity(t *testing.T, raw json.RawMessage, demand typefacts.EntityDemand) {
	t.Helper()
	var decoded struct {
		Entities []struct {
			Location         typefacts.Location `json:"location"`
			Symbol           string             `json:"symbol"`
			SymbolUnresolved bool               `json:"symbolUnresolved"`
		} `json:"entities"`
	}
	if err := json.Unmarshal(raw, &decoded); err != nil {
		t.Fatal(err)
	}
	matches := 0
	for _, entity := range decoded.Entities {
		if entity.Location != demand.Location {
			continue
		}
		matches++
		if entity.Symbol == "" || entity.SymbolUnresolved {
			t.Errorf("demand at %+v answered symbol %q unresolved %v", demand.Location, entity.Symbol, entity.SymbolUnresolved)
		}
	}
	if matches != 1 {
		t.Fatalf("demand at %+v answered %d times in %s", demand.Location, matches, raw)
	}
}

// span is the byte span of the occurrence-th (from 0) copy of needle in file.
func span(t *testing.T, file, needle string, occurrence int) (int, int) {
	t.Helper()
	contents, err := os.ReadFile(file)
	if err != nil {
		t.Fatal(err)
	}
	offset := 0
	for range occurrence {
		next := bytes.Index(contents[offset:], []byte(needle))
		if next < 0 {
			t.Fatalf("%s holds fewer than %d copies of %s", file, occurrence+1, needle)
		}
		offset += next + len(needle)
	}
	next := bytes.Index(contents[offset:], []byte(needle))
	if next < 0 {
		t.Fatalf("%s holds fewer than %d copies of %s", file, occurrence+1, needle)
	}
	return offset + next, offset + next + len(needle)
}

var sourceFiles = []string{
	"src/alias-use.ts",
	"src/alias/helper.ts",
	"src/ambient.d.ts",
	"src/dynamic.ts",
	"src/equals.ts",
	"src/forms.ts",
	"src/jsdoc.js",
	"src/local-bound.js",
	"src/param-bound.js",
	"src/ts-require.ts",
	"src/unbound.js",
	"src/unresolved.ts",
}

func paths(root string, relatives ...string) []string {
	absolute := make([]string, 0, len(relatives))
	for _, relative := range relatives {
		absolute = append(absolute, filepath.Join(root, relative))
	}
	return absolute
}

func aliasedDemand(t *testing.T, root string) typefacts.EntityDemand {
	t.Helper()
	file := filepath.Join(root, "src/alias-use.ts")
	start, end := span(t, file, "aliased", 0)
	return typefacts.EntityDemand{
		Location: typefacts.Location{Path: file, StartByte: start, EndByte: end},
		Symbol:   true,
	}
}

type row struct {
	Path       string
	Start, End int
	Text       string
	Resolution typefacts.ModuleResolution
}

func TestNoGraphRequestKeepsTheResponseShape(t *testing.T) {
	root := forms(t)
	base := request{
		ProjectID:  filepath.Join(root, "tsconfig.json"),
		Generation: 1,
		Demands:    []typefacts.EntityDemand{aliasedDemand(t, root)},
	}
	output, answer := call(t, base)
	// Shape only: before the retained projection the same request lost its
	// answer (entities null), so for a nonempty demand these bytes now differ
	// from the base by exactly that answer. Root keys and their order are kept.
	legacy, err := json.Marshal(legacyResponse{OK: true, Table: answer.Table})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(output, legacy) {
		t.Fatalf("no-graph response differs from the original encoding\n got %s\nwant %s", output, legacy)
	}
	var keys map[string]json.RawMessage
	if err := json.Unmarshal(output, &keys); err != nil {
		t.Fatal(err)
	}
	if _, present := keys["moduleGraph"]; present || len(keys) != 2 {
		t.Fatalf("no-graph response keys = %v, want ok and table", keys)
	}
	withGraph := base
	withGraph.ModuleGraph = &typefacts.ModuleInventoryDemand{Imports: true, Packages: true}
	_, graphed := call(t, withGraph)
	if !bytes.Equal(answer.Table, graphed.Table) {
		t.Fatalf("asking for the graph changed the table\nwithout %s\n   with %s", answer.Table, graphed.Table)
	}
	answeredEntity(t, answer.Table, base.Demands[0])
	answeredEntity(t, graphed.Table, base.Demands[0])

	// The generation is not an echoed label: a freshly opened program is
	// generation 1, and the closure refuses any other.
	stale := withGraph
	stale.Generation = 2
	encoded, err := json.Marshal(stale)
	if err != nil {
		t.Fatal(err)
	}
	output, status := respond(encoded)
	if status == 0 || !bytes.Equal(output, []byte(`{"ok":false,"error":"type facts generation mismatch"}`)) {
		t.Fatalf("generation 2 answered with status %d: %s", status, output)
	}
}

func TestGraphCarriesEveryCanonicalForm(t *testing.T) {
	root := forms(t)
	absent := filepath.Join(root, "src/absent.ts")
	demand := aliasedDemand(t, root)
	_, answer := call(t, request{
		ProjectID:  filepath.Join(root, "tsconfig.json"),
		Generation: 1,
		Demands:    []typefacts.EntityDemand{demand},
		ModuleGraph: &typefacts.ModuleInventoryDemand{
			Imports:     true,
			Packages:    true,
			ImportPaths: append(paths(root, sourceFiles...), absent),
		},
	})

	answered := table(t, answer)
	if answered.Generation != 1 || answered.ProjectID != filepath.Join(root, "tsconfig.json") {
		t.Fatalf("table header = %d %q", answered.Generation, answered.ProjectID)
	}
	if demand.Location.StartByte != 53 || demand.Location.EndByte != 60 {
		t.Fatalf("the demand is not alias-use.ts[53,60): %+v", demand.Location)
	}
	answeredEntity(t, answer.Table, demand)
	digested := map[string]bool{}
	for _, source := range answered.Sources {
		if !strings.HasPrefix(source.SHA256, "sha256:") {
			t.Fatalf("source digest %q", source.SHA256)
		}
		digested[source.Path] = true
	}
	for _, file := range paths(root, "node_modules/pkg/index.js", "src/unbound.js", "src/forms.ts") {
		if !digested[file] {
			t.Errorf("no source digest for %s", file)
		}
	}
	for _, file := range paths(root, "node_modules/pkg/index.d.ts", "src/ambient.d.ts") {
		if digested[file] {
			t.Errorf("declaration %s carries a source digest", file)
		}
	}

	graph := answer.ModuleGraph
	if graph == nil {
		t.Fatal("no module graph")
	}
	modules := map[string]typefacts.ModuleFact{}
	bundled := 0
	for _, module := range graph.Modules {
		modules[module.Path] = module
		if strings.HasPrefix(module.Path, "bundled:") && module.DeclarationFile {
			bundled++
		}
	}
	if bundled == 0 {
		t.Error("no bundled library declaration in the inventory")
	}
	for _, file := range paths(root, append([]string{"node_modules/pkg/index.js", "node_modules/pkg/index.d.ts"}, sourceFiles...)...) {
		module, ok := modules[file]
		if !ok {
			t.Errorf("inventory lacks %s", file)
			continue
		}
		if module.DeclarationFile != strings.HasSuffix(file, ".d.ts") {
			t.Errorf("%s declarationFile = %v", file, module.DeclarationFile)
		}
	}
	if !reflect.DeepEqual(graph.UnknownImportPaths, []string{absent}) {
		t.Errorf("unknownImportPaths = %v, want [%s]", graph.UnknownImportPaths, absent)
	}

	pkg := func(file string, occurrence int) row {
		start, end := span(t, filepath.Join(root, file), `"pkg"`, occurrence)
		return row{filepath.Join(root, file), start, end, "pkg", typefacts.ModuleResolutionNodeModules}
	}
	unresolved := func(file, needle, text string) row {
		start, end := span(t, filepath.Join(root, file), needle, 0)
		return row{filepath.Join(root, file), start, end, text, typefacts.ModuleResolutionUnresolved}
	}
	aliasStart, aliasEnd := span(t, filepath.Join(root, "src/alias-use.ts"), `"@alias/helper"`, 0)
	want := []row{
		{filepath.Join(root, "src/alias-use.ts"), aliasStart, aliasEnd, "@alias/helper", typefacts.ModuleResolutionNonRelative},
		// Non-relative only: the ambient body's "./local" is never collected.
		pkg("src/ambient.d.ts", 0),
		pkg("src/dynamic.ts", 0),
		pkg("src/equals.ts", 0),
		pkg("src/forms.ts", 0),
		pkg("src/forms.ts", 1),
		pkg("src/forms.ts", 2),
		pkg("src/jsdoc.js", 0),
		pkg("src/local-bound.js", 0),
		pkg("src/param-bound.js", 0),
		// src/ts-require.ts: TypeScript files contribute no require rows.
		pkg("src/unbound.js", 0),
		unresolved("src/unbound.js", `""`, ""),
		unresolved("src/unresolved.ts", `"missing-package"`, "missing-package"),
	}
	got := make([]row, 0, len(graph.Imports))
	for _, fact := range graph.Imports {
		got = append(got, row{fact.Specifier.Path, fact.Specifier.StartByte, fact.Specifier.EndByte, fact.Text, fact.Resolution})
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("import rows\n got %+v\nwant %+v", got, want)
	}
	for _, fact := range graph.Imports {
		switch fact.Text {
		case "pkg":
			wantPackage := typefacts.PackageIdentity{
				ManifestPath: filepath.Join(root, "node_modules/pkg/package.json"),
				Name:         "pkg",
				Version:      "1.2.3",
			}
			if fact.ResolvedPath != filepath.Join(root, "node_modules/pkg/index.d.ts") ||
				fact.Extension != ".d.ts" || fact.Package == nil || *fact.Package != wantPackage ||
				fact.ResolverPackage == nil || fact.ResolverPackage.Name != "pkg" ||
				fact.ResolverPackage.Version != "1.2.3" {
				t.Errorf("npm row %+v package %+v resolver %+v", fact, fact.Package, fact.ResolverPackage)
			}
		case "@alias/helper":
			if fact.ResolvedPath != filepath.Join(root, "src/alias/helper.ts") || fact.PathsPattern != "@alias/*" {
				t.Errorf("alias row %+v", fact)
			}
		default:
			if fact.ResolvedPath != "" || fact.Package != nil {
				t.Errorf("unresolved row %+v", fact)
			}
		}
	}
}

func graphOf(t *testing.T, root string, demand typefacts.ModuleInventoryDemand) typefacts.ModuleInventory {
	t.Helper()
	_, answer := call(t, request{
		ProjectID:   filepath.Join(root, "tsconfig.json"),
		Generation:  1,
		ModuleGraph: &demand,
	})
	if answer.ModuleGraph == nil {
		t.Fatal("no module graph")
	}
	return *answer.ModuleGraph
}

func TestGraphDemandIsPassedVerbatim(t *testing.T) {
	root := forms(t)

	inventory := graphOf(t, root, typefacts.ModuleInventoryDemand{})
	if len(inventory.Modules) == 0 || inventory.Imports != nil || inventory.UnknownImportPaths != nil {
		t.Fatalf("zero demand = %d modules, %d imports, unknown %v; want the inventory alone",
			len(inventory.Modules), len(inventory.Imports), inventory.UnknownImportPaths)
	}

	everything := graphOf(t, root, typefacts.ModuleInventoryDemand{Imports: true, Packages: true})
	all := make([]string, 0, len(inventory.Modules))
	for _, module := range inventory.Modules {
		all = append(all, module.Path)
	}
	listed := graphOf(t, root, typefacts.ModuleInventoryDemand{Imports: true, Packages: true, ImportPaths: all})
	if len(everything.Imports) == 0 || !reflect.DeepEqual(everything, listed) {
		t.Fatalf("imports with no paths is not every program file:\nempty  %+v\nlisted %+v", everything.Imports, listed.Imports)
	}

	scoped := paths(root, sourceFiles...)
	withPackages := graphOf(t, root, typefacts.ModuleInventoryDemand{Imports: true, Packages: true, ImportPaths: scoped})
	withoutPackages := graphOf(t, root, typefacts.ModuleInventoryDemand{Imports: true, ImportPaths: scoped})
	if len(withPackages.Imports) != len(withoutPackages.Imports) {
		t.Fatalf("packages changed the row count: %d vs %d", len(withPackages.Imports), len(withoutPackages.Imports))
	}
	identities := 0
	for index, fact := range withPackages.Imports {
		if fact.Package != nil {
			identities++
		}
		fact.Package, fact.ResolverPackage = nil, nil
		if !reflect.DeepEqual(fact, withoutPackages.Imports[index]) {
			t.Errorf("row %d differs beyond package identity:\n with %+v\n  w/o %+v", index, fact, withoutPackages.Imports[index])
		}
	}
	if identities == 0 {
		t.Fatal("packages:true carried no package identity")
	}
}

// failingGraph is the opened closure with only its graph answer refused.
type failingGraph struct {
	*typefacts.DemandClosure
}

func (failingGraph) ModuleGraph(context.Context, typefacts.ModuleInventoryDemand) (typefacts.ModuleInventory, error) {
	return typefacts.ModuleInventory{}, errors.New("module graph refused")
}

func TestGraphFailureFailsTheWholeResponse(t *testing.T) {
	root := forms(t)
	backend, err := tsgo.OpenProject(context.Background(), filepath.Join(root, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer closure.Close()
	asked := request{
		ProjectID:   filepath.Join(root, "tsconfig.json"),
		Generation:  1,
		Demands:     []typefacts.EntityDemand{aliasedDemand(t, root)},
		ModuleGraph: &typefacts.ModuleInventoryDemand{Imports: true},
	}
	if table, graph, err := assemble(context.Background(), asked, closure); err != nil || table == nil || graph == nil {
		t.Fatalf("the real closure did not answer: %v", err)
	}
	output, status := encode(assemble(context.Background(), asked, failingGraph{closure}))
	if status == 0 {
		t.Fatalf("status 0 for a refused graph: %s", output)
	}
	var keys map[string]json.RawMessage
	if err := json.Unmarshal(output, &keys); err != nil {
		t.Fatal(err)
	}
	if string(keys["ok"]) != "false" || keys["table"] != nil || keys["moduleGraph"] != nil ||
		string(keys["error"]) != `"module graph refused"` {
		t.Fatalf("refused graph response = %s", output)
	}
}

// TestEncodeFailsAtomicallyOnAnUnencodableTable drives the marshal branch of
// the shared encoder with a table that is not JSON.
func TestEncodeFailsAtomicallyOnAnUnencodableTable(t *testing.T) {
	output, status := encode(json.RawMessage("{"), &typefacts.ModuleInventory{}, nil)
	var keys map[string]json.RawMessage
	if err := json.Unmarshal(output, &keys); err != nil {
		t.Fatal(err)
	}
	var message string
	if err := json.Unmarshal(keys["error"], &message); err != nil {
		t.Fatal(err)
	}
	if status == 0 || string(keys["ok"]) != "false" || keys["table"] != nil || keys["moduleGraph"] != nil || message == "" {
		t.Fatalf("unencodable table answered status %d: %s", status, output)
	}
}

func TestChangedSourceChangesItsDigest(t *testing.T) {
	digests := func(root string) map[string]string {
		_, answer := call(t, request{ProjectID: filepath.Join(root, "tsconfig.json"), Generation: 1})
		byPath := map[string]string{}
		for _, source := range table(t, answer).Sources {
			relative, err := filepath.Rel(root, source.Path)
			if err != nil {
				t.Fatal(err)
			}
			byPath[relative] = source.SHA256
		}
		return byPath
	}
	intact := forms(t)
	changed := forms(t)
	if err := os.WriteFile(filepath.Join(changed, "src/alias/helper.ts"), []byte("export const helper = 2;\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	before, after := digests(intact), digests(changed)
	keys := make([]string, 0, len(before))
	for key := range before {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		if (before[key] == after[key]) == (key == "src/alias/helper.ts") {
			t.Errorf("%s: %s -> %s", key, before[key], after[key])
		}
	}
	if len(before) != len(after) || before["src/alias/helper.ts"] == "" {
		t.Fatalf("source sets differ: %v vs %v", before, after)
	}
}
