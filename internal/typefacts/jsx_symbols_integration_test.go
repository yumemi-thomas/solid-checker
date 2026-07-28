package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

type jsxSymbolFixture struct {
	projectPath   string
	runtimePath   string
	consumerPath  string
	unrelatedPath string
	consumer      string
	demands       []typefacts.EntityDemand
	locations     map[string]typefacts.Location
}

func newJSXSymbolFixture(t *testing.T) jsxSymbolFixture {
	t.Helper()
	root := t.TempDir()
	write := func(name, source string) string {
		t.Helper()
		path := filepath.Join(root, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
		return path
	}
	projectPath := write("tsconfig.json", `{
		"compilerOptions": {
			"strict": true,
			"module": "esnext",
			"moduleResolution": "bundler",
			"target": "esnext",
			"jsx": "preserve",
			"noEmit": true
		},
		"include": ["*.ts", "*.tsx"]
	}`)
	write("jsx.d.ts", `declare namespace JSX {
		interface Element {}
		interface IntrinsicElements { div: {}; }
	}`)
	runtimePath := write("runtime.tsx", `export function Component() { return <div />; }
`)
	write("barrel.ts", `export { Component as ReExported } from "./runtime";
`)
	write("bridge.ts", `export { ReExported } from "./barrel";
`)
	unrelatedPath := write("unrelated.ts", `export const unrelated = 1;
`)
	consumer := `import { Component } from "./runtime";
import { Component as Alias } from "./runtime";
import * as Runtime from "./runtime";
import { ReExported } from "./bridge";
import type { Component as TypeOnlyComponent } from "./runtime";

function Local() { return <div />; }
const opening = <Component></Component>;
const selfClosing = <Alias />;
const namespaceMember = <Runtime.Component />;
const reexported = <ReExported />;
const local = <Local />;
const intrinsic = <div />;
const invalidTypeOnly = <TypeOnlyComponent />;
function render(Component: typeof Runtime.Component) {
	return <Component />;
}
`
	consumerPath := write("consumer.tsx", consumer)
	location := func(label, needle string, from int) typefacts.Location {
		t.Helper()
		offset := strings.Index(consumer[from:], needle)
		if offset < 0 {
			t.Fatalf("%s: %q not found", label, needle)
		}
		start := from + offset
		return typefacts.Location{
			Path:      consumerPath,
			StartByte: start,
			EndByte:   start + len(needle),
		}
	}
	locations := map[string]typefacts.Location{}
	locations["namespaceImport"] = location("namespace import", "Runtime", strings.Index(consumer, "import * as"))
	locations["opening"] = location("opening tag", "Component", strings.Index(consumer, "const opening"))
	locations["closing"] = location("closing tag", "Component", strings.Index(consumer, "</Component>"))
	locations["alias"] = location("aliased self-closing tag", "Alias", strings.Index(consumer, "const selfClosing"))
	locations["namespaceMember"] = location("namespace-member tag", "Runtime.Component", strings.Index(consumer, "const namespaceMember"))
	locations["reexported"] = location("re-exported tag", "ReExported", strings.Index(consumer, "const reexported"))
	locations["local"] = location("local tag", "Local", strings.Index(consumer, "const local"))
	locations["intrinsic"] = location("intrinsic tag", "div", strings.Index(consumer, "const intrinsic"))
	locations["typeOnly"] = location("type-only tag", "TypeOnlyComponent", strings.Index(consumer, "const invalidTypeOnly"))
	shadowFunction := strings.Index(consumer, "function render")
	locations["shadowDeclaration"] = location("shadow declaration", "Component", shadowFunction)
	locations["shadowUse"] = location("shadowed tag", "Component", strings.Index(consumer[shadowFunction:], "return <")+shadowFunction)

	demands := make([]typefacts.EntityDemand, 0, len(locations)-1)
	for label, demanded := range locations {
		if label == "shadowDeclaration" {
			continue
		}
		demands = append(demands, typefacts.EntityDemand{
			Location:        demanded,
			Symbol:          true,
			ReferenceSpace:  true,
			RuntimeIdentity: true,
		})
	}
	sort.Slice(demands, func(i, j int) bool {
		return demands[i].Location.StartByte < demands[j].Location.StartByte
	})
	return jsxSymbolFixture{
		projectPath:   projectPath,
		runtimePath:   runtimePath,
		consumerPath:  consumerPath,
		unrelatedPath: unrelatedPath,
		consumer:      consumer,
		demands:       demands,
		locations:     locations,
	}
}

func entitiesByLocation(entities []typefacts.EntityFact) map[typefacts.Location]typefacts.EntityFact {
	result := make(map[typefacts.Location]typefacts.EntityFact, len(entities))
	for _, entity := range entities {
		result[entity.Location] = entity
	}
	return result
}

func symbolFactsByID(facts []typefacts.SymbolFact) map[typefacts.SymbolID]typefacts.SymbolFact {
	result := make(map[typefacts.SymbolID]typefacts.SymbolFact, len(facts))
	for _, fact := range facts {
		result[fact.ID] = fact
	}
	return result
}

func canonicalSymbolFact(t *testing.T, facts map[typefacts.SymbolID]typefacts.SymbolFact, id typefacts.SymbolID) typefacts.SymbolFact {
	t.Helper()
	seen := make(map[typefacts.SymbolID]struct{})
	for id != "" {
		if _, duplicate := seen[id]; duplicate {
			t.Fatalf("alias cycle at %s", id)
		}
		seen[id] = struct{}{}
		fact, ok := facts[id]
		if !ok {
			t.Fatalf("symbol closure is missing %s", id)
		}
		if fact.AliasTarget == "" {
			return fact
		}
		id = fact.AliasTarget
	}
	t.Fatal("empty canonical symbol")
	return typefacts.SymbolFact{}
}

func TestJSXTagDemandsUseCompilerSymbolIdentity(t *testing.T) {
	fixture := newJSXSymbolFixture(t)
	ctx := context.Background()

	directProject, err := tsgo.OpenProject(ctx, fixture.projectPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	direct, err := directProject.(typefacts.SemanticEntityLookup).SemanticEntities(ctx, fixture.demands)
	if err != nil {
		t.Fatal(err)
	}
	if err := directProject.Close(); err != nil {
		t.Fatal(err)
	}
	for index, entity := range direct {
		if entity.Symbol == "" {
			t.Errorf("direct entity %d at %+v has no symbol", index, entity.Location)
		}
	}

	backend, err := tsgo.OpenProject(ctx, fixture.projectPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })
	table, err := closure.DemandTableForGroups(ctx, 1, []typefacts.DemandGroup{{
		Path: fixture.consumerPath, Demands: fixture.demands,
	}}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(table.Entities, direct) {
		t.Fatalf("batched SemanticEntities and demand closure differ:\ndirect: %+v\nclosure: %+v", direct, table.Entities)
	}
	batchedWire := typefacts.FactTableV2From(*table, fixture.projectPath, 1)

	entities := entitiesByLocation(table.Entities)
	symbols := symbolFactsByID(table.Symbols)
	entity := func(label string) typefacts.EntityFact {
		t.Helper()
		got, ok := entities[fixture.locations[label]]
		if !ok {
			t.Fatalf("missing %s entity at %+v", label, fixture.locations[label])
		}
		return got
	}
	canonical := func(label string) typefacts.SymbolFact {
		t.Helper()
		return canonicalSymbolFact(t, symbols, entity(label).Symbol)
	}
	component := canonical("opening")
	for _, label := range []string{"closing", "alias", "namespaceMember", "reexported"} {
		if got := canonical(label).ID; got != component.ID {
			t.Errorf("%s canonical symbol = %s, want component %s", label, got, component.ID)
		}
	}
	for _, label := range []string{"opening", "alias", "reexported"} {
		if got := symbols[entity(label).Symbol].AliasTarget; got == "" {
			t.Errorf("%s symbol %s has no alias target", label, entity(label).Symbol)
		}
	}
	componentRuntime := entity("opening").RuntimeIdentity
	if componentRuntime == "" {
		t.Fatal("imported JSX component has no runtime identity")
	}
	for _, label := range []string{"closing", "alias", "namespaceMember", "reexported"} {
		if got := entity(label).RuntimeIdentity; got != componentRuntime {
			t.Errorf("%s runtime identity = %q, want component identity %q", label, got, componentRuntime)
		}
	}
	if got, namespace := entity("namespaceMember").Symbol, entity("namespaceImport").Symbol; got == namespace {
		t.Errorf("namespace-member JSX resolved namespace symbol %s instead of the selected member", got)
	}
	if len(component.Declarations) == 0 ||
		filepath.Clean(component.Declarations[0].Location.Path) != filepath.Clean(fixture.runtimePath) ||
		component.Declarations[0].Location.StartByte != strings.Index(`export function Component() { return <div />; }`, "Component") {
		t.Errorf("component canonical declaration = %+v, want runtime Component export", component.Declarations)
	}
	local := canonical("local")
	if len(local.Declarations) == 0 || filepath.Clean(local.Declarations[0].Location.Path) != filepath.Clean(fixture.consumerPath) {
		t.Errorf("local component declaration = %+v", local.Declarations)
	}
	shadow := canonical("shadowUse")
	if len(shadow.Declarations) == 0 ||
		shadow.Declarations[0].Location.StartByte != fixture.locations["shadowDeclaration"].StartByte {
		t.Errorf("shadowed JSX declaration = %+v, want local parameter at %+v", shadow.Declarations, fixture.locations["shadowDeclaration"])
	}
	if entity("typeOnly").RuntimeIdentity != "" {
		t.Errorf("type-only JSX runtime identity = %q, want empty", entity("typeOnly").RuntimeIdentity)
	}
	if entity("intrinsic").RuntimeIdentity != "" {
		t.Errorf("intrinsic JSX runtime identity = %q, want empty", entity("intrinsic").RuntimeIdentity)
	}

	if _, err := closure.Update(ctx, []typefacts.FileChange{{
		Path: fixture.unrelatedPath, Version: 1, Source: []byte("export const unrelated = 2;\n"),
	}}); err != nil {
		t.Fatal(err)
	}
	retained, err := closure.DemandTableForGroups(ctx, 2, []typefacts.DemandGroup{{
		Path: fixture.consumerPath, Demands: fixture.demands,
	}}, nil)
	if err != nil {
		t.Fatal(err)
	}
	retainedWire := typefacts.FactTableV2From(*retained, fixture.projectPath, 2)
	if !reflect.DeepEqual(retainedWire.Entities, batchedWire.Entities) ||
		!reflect.DeepEqual(retainedWire.Symbols, batchedWire.Symbols) {
		t.Fatalf("retained JSX facts differ from the batched facts")
	}
	if closure.Stats().Retention.RetainedFiles == 0 {
		t.Fatal("JSX demand contribution was not retained across an unrelated edit")
	}
}

func TestJSXTagDemandDoesNotEagerlyResolveSymbolIdentity(t *testing.T) {
	fixture := newJSXSymbolFixture(t)
	backend, err := tsgo.OpenProject(context.Background(), fixture.projectPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer backend.Close()
	entities, err := backend.(typefacts.SemanticEntityLookup).SemanticEntities(
		context.Background(),
		[]typefacts.EntityDemand{{Location: fixture.locations["namespaceMember"]}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != 1 {
		t.Fatalf("entities = %d, want 1", len(entities))
	}
	if entity := entities[0]; entity.Symbol != "" || entity.ReferenceSpace != "" || entity.RuntimeIdentity != "" {
		t.Fatalf("symbol identity was computed without a symbol-bearing demand: %+v", entity)
	}
}
