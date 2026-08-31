package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestRuntimeBindingKindCensusesInitializersAndDirectWrites(t *testing.T) {
	dir := t.TempDir()
	source := `export var EnumLike;
(function (EnumLike) { EnumLike["A"] = "a"; })(EnumLike || (EnumLike = {}));
export var Alias = EnumLike;
export var IIFEClass = (() => { class Inner {}; return Inner; })();
export var Mixed = {};
if (globalThis.flag) Mixed = () => {};
export var Open = globalThis.makeUnknown();
export var PropertyOnly = {};
PropertyOnly.callback = () => {};
export var Brave = !!globalThis.host && globalThis.host.fn && globalThis.host.fn.name === "fn";
`
	path := filepath.Join(dir, "artifact.js")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","target":"esnext"},"include":["*.js"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	p := opened.(*project)
	cursor := semanticNodeCursor{sourceFile: p.program.GetSourceFile(path)}

	cases := []struct {
		name string
		want typefacts.RuntimeBindingKind
	}{
		{"EnumLike", typefacts.RuntimeBindingNonCallable},
		{"Alias", typefacts.RuntimeBindingNonCallable},
		{"IIFEClass", typefacts.RuntimeBindingCallable},
		{"Mixed", typefacts.RuntimeBindingMixed},
		{"Open", typefacts.RuntimeBindingOpen},
		{"PropertyOnly", typefacts.RuntimeBindingNonCallable},
		{"Brave", typefacts.RuntimeBindingNonCallable},
	}
	for _, testCase := range cases {
		start := strings.Index(source, testCase.name)
		node := cursor.exactExpressionAt(start, start+len(testCase.name))
		if got := p.runtimeBindingKindLocked(node); got != testCase.want {
			t.Errorf("%s runtime binding kind = %d, want %d", testCase.name, got, testCase.want)
		}
	}
}

func TestRuntimeBindingKindAppearsOnlyForRuntimeIdentityDemand(t *testing.T) {
	dir := t.TempDir()
	source := "var EnumLike;\n(function (E) { E.a = 1; })(EnumLike || (EnumLike = {}));\nexport { EnumLike };\n"
	path := filepath.Join(dir, "artifact.js")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","target":"esnext"},"include":["*.js"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.LastIndex(source, "EnumLike")
	base := typefacts.EntityDemand{
		Location:         typefacts.Location{Path: path, StartByte: start, EndByte: start + len("EnumLike")},
		Callability:      true,
		Constructability: true,
		RuntimeIdentity:  true,
	}
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), []typefacts.EntityDemand{base})
	if err != nil {
		t.Fatal(err)
	}
	if got := entities[0].RuntimeBindingKind; got != typefacts.RuntimeBindingNonCallable {
		t.Fatalf("runtime binding kind = %d, want noncallable", got)
	}
	base.RuntimeIdentity = false
	entities, err = opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), []typefacts.EntityDemand{base})
	if err != nil {
		t.Fatal(err)
	}
	if got := entities[0].RuntimeBindingKind; got != typefacts.RuntimeBindingAbsent {
		t.Fatalf("undemanded runtime binding kind = %d, want absent", got)
	}
}

func TestRuntimeBindingKindFollowsExactRelativeRuntimeImportPastDeclarationSubstitution(t *testing.T) {
	dir := t.TempDir()
	indexSource := "import Func from \"./func.mjs\";\nexport { Func };\n"
	indexPath := filepath.Join(dir, "index.mjs")
	files := map[string]string{
		"index.mjs": indexSource,
		"func.mjs":  "export default function Func() {}\n",
		// TypeScript resolves the explicit .mjs import to this declaration.
		// Its any is deliberately not runtime-kind evidence.
		"func.d.mts": "declare const Func: any;\nexport default Func;\n",
	}
	for name, source := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["index.mjs","func.mjs","func.d.mts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.LastIndex(indexSource, "Func")
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path: indexPath, StartByte: start, EndByte: start + len("Func"),
		},
		Callability: true, Constructability: true, RuntimeIdentity: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := entities[0].RuntimeBindingKind; got != typefacts.RuntimeBindingCallable {
		t.Fatalf("runtime binding kind = %d, want callable runtime function", got)
	}
}

func TestRuntimeBindingKindCensusesRuntimeExportWhenSiblingDeclarationClosesSignatures(t *testing.T) {
	dir := t.TempDir()
	source := "export const published = () => {};\nexport const value = { kind: 'value' };\n"
	path := filepath.Join(dir, "index.js")
	files := map[string]string{
		"index.js":   source,
		"index.d.ts": "export declare const published: () => void;\nexport declare const value: { kind: string };\n",
	}
	for name, contents := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"allowJs":true,"checkJs":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"files":["index.js","index.d.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.Index(source, "published")
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path: path, StartByte: start, EndByte: start + len("published"),
		},
		Callability: true, Constructability: true, RuntimeIdentity: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := entities[0].RuntimeBindingKind; got != typefacts.RuntimeBindingCallable {
		t.Fatalf("runtime binding kind = %d, want callable runtime export", got)
	}
}
