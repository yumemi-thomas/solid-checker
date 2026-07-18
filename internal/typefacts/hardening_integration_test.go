package typefacts_test

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
	"github.com/yumemi-thomas/solid-check/internal/typefacts/tsgo"
)

func TestProjectResolvesChainedReexportsNamespaceGenericsAndPackageSubpaths(t *testing.T) {
	root := newAdvancedProject(t)
	project := openProject(t, root)
	usePath := filepath.Join(root, "use.ts")

	tests := []struct {
		call              string
		returnType        string
		declarationSuffix string
	}{
		{call: "api.identity(true)", returnType: "true", declarationSuffix: filepath.Join("lib", "value.ts")},
		{call: "api.choose(42)", returnType: "42", declarationSuffix: filepath.Join("lib", "value.ts")},
		{call: "feature('ok')", returnType: `"ok"`, declarationSuffix: "feature.d.ts"},
	}

	for _, test := range tests {
		t.Run(test.call, func(t *testing.T) {
			call, err := project.ResolvedCall(context.Background(), locationOf(t, usePath, test.call))
			if err != nil {
				t.Fatal(err)
			}
			if call.ReturnTypeText != test.returnType {
				t.Fatalf("return type = %q, want %q", call.ReturnTypeText, test.returnType)
			}
			declarations, err := project.Declarations(context.Background(), call.Target)
			if err != nil {
				t.Fatal(err)
			}
			found := false
			for _, declaration := range declarations {
				if strings.HasSuffix(declaration.Location.Path, test.declarationSuffix) {
					found = true
				}
			}
			if !found {
				t.Fatalf("declarations = %#v, want suffix %q", declarations, test.declarationSuffix)
			}
		})
	}
}

func TestProjectUsesExactUTF8ByteRangesAndRejectsSplitRunes(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true,"module":"ESNext","moduleResolution":"Bundler"},"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const 東京 = () => 1;\r\n")
	writeProjectFile(t, root, "use.ts", "import { 東京 as local } from './source';\r\nconst emoji = '😀'; local();\r\n")
	project := openProject(t, root)

	usePath := filepath.Join(root, "use.ts")
	alias := locationOf(t, usePath, "local()")
	alias.EndByte = alias.StartByte + len("local")
	aliasID, err := project.SymbolAt(context.Background(), alias)
	if err != nil {
		t.Fatal(err)
	}
	originalID, err := project.ResolveAlias(context.Background(), aliasID)
	if err != nil {
		t.Fatal(err)
	}
	declarations, err := project.Declarations(context.Background(), originalID)
	if err != nil {
		t.Fatal(err)
	}
	want := locationOf(t, filepath.Join(root, "source.ts"), "東京 =")
	if declarations[0].Location.StartByte != want.StartByte || declarations[0].Location.EndByte != want.StartByte+len("東京") {
		t.Fatalf("declaration range = %d:%d, want %d:%d", declarations[0].Location.StartByte, declarations[0].Location.EndByte, want.StartByte, want.StartByte+len("東京"))
	}

	source, err := os.ReadFile(filepath.Join(root, "source.ts"))
	if err != nil {
		t.Fatal(err)
	}
	start := strings.Index(string(source), "東京")
	_, err = project.SymbolAt(context.Background(), typefacts.Location{
		Path: filepath.Join(root, "source.ts"), StartByte: start + 1, EndByte: start + len("東京"),
	})
	if err == nil || !strings.Contains(err.Error(), "UTF-8") {
		t.Fatalf("split-rune SymbolAt() error = %v, want UTF-8 boundary error", err)
	}
}

func TestProjectDiscoversParserDerivedCallAndArgumentSpans(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	source := "function wrap<T>(value: T, options: { nested: number }) { return value }\nwrap<string>(\"ok\", { nested: Number(1) })\n"
	path := filepath.Join(root, "calls.ts")
	writeProjectFile(t, root, "calls.ts", source)
	project := openProject(t, root)

	discoverer, ok := project.(typefacts.CallDiscoverer)
	if !ok {
		t.Fatal("TypeScript-Go project does not expose call discovery")
	}
	calls, err := discoverer.SourceCalls(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	var wrap typefacts.SourceCall
	for _, call := range calls {
		if source[call.Callee.StartByte:call.Callee.EndByte] == "wrap" {
			wrap = call
			break
		}
	}
	if len(wrap.Arguments) != 2 {
		t.Fatalf("wrap arguments = %#v, want two parser-derived spans", wrap.Arguments)
	}
	if got := source[wrap.Arguments[0].StartByte:wrap.Arguments[0].EndByte]; got != `"ok"` {
		t.Fatalf("first argument = %q", got)
	}
	if got := source[wrap.Arguments[1].StartByte:wrap.Arguments[1].EndByte]; got != "{ nested: Number(1) }" {
		t.Fatalf("second argument = %q", got)
	}
	declarations, err := project.Declarations(context.Background(), wrap.Target)
	if err != nil {
		t.Fatal(err)
	}
	if len(declarations) == 0 || declarations[0].Name != "wrap" {
		t.Fatalf("target declarations = %#v", declarations)
	}
}

func TestProjectDiscoversCallInitializedBindings(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	source := "declare function pair(): [() => number, number, (value: number) => void]\nconst direct = pair()\nconst [read, , write] = pair()\n"
	path := filepath.Join(root, "bindings.ts")
	writeProjectFile(t, root, "bindings.ts", source)
	project := openProject(t, root)

	discoverer, ok := project.(typefacts.BindingDiscoverer)
	if !ok {
		t.Fatal("TypeScript-Go project does not expose binding discovery")
	}
	bindings, err := discoverer.SourceBindings(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	if len(bindings) != 2 {
		t.Fatalf("bindings = %#v, want direct and tuple bindings", bindings)
	}
	if bindings[0].Array || len(bindings[0].Names) != 1 || source[bindings[0].Names[0].StartByte:bindings[0].Names[0].EndByte] != "direct" {
		t.Fatalf("direct binding = %#v", bindings[0])
	}
	if !bindings[1].Array || len(bindings[1].Names) != 3 {
		t.Fatalf("tuple binding = %#v", bindings[1])
	}
	if source[bindings[1].Names[0].StartByte:bindings[1].Names[0].EndByte] != "read" || bindings[1].Names[1].Path != "" || source[bindings[1].Names[2].StartByte:bindings[1].Names[2].EndByte] != "write" {
		t.Fatalf("tuple slots = %#v", bindings[1].Names)
	}
}

func TestProjectDiscoversNamedBlockBodiedFunctions(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.tsx"]}`)
	source := "export async function load(value: string) { return value }\nexport const View = async (props: { name: string }) => { return <div>{props.name}</div> }\nconst expression = (value: number) => value + 1\n"
	path := filepath.Join(root, "functions.tsx")
	writeProjectFile(t, root, "functions.tsx", source)
	project := openProject(t, root)

	discoverer, ok := project.(typefacts.FunctionDiscoverer)
	if !ok {
		t.Fatal("TypeScript-Go project does not expose function discovery")
	}
	functions, err := discoverer.SourceFunctions(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	if len(functions) != 2 {
		t.Fatalf("functions = %#v, want declaration and block-bodied arrow", functions)
	}
	for index, want := range []string{"load", "View"} {
		function := functions[index]
		if got := source[function.Name.StartByte:function.Name.EndByte]; got != want {
			t.Fatalf("function %d name = %q, want %q", index, got, want)
		}
		if !function.Exported || !function.Async || len(function.Parameters) != 1 || source[function.Body.StartByte] != '{' || source[function.Body.EndByte] != '}' {
			t.Fatalf("function %s facts = %#v", want, function)
		}
	}
	if functions[0].Arrow || !functions[1].Arrow {
		t.Fatalf("function kinds = %#v", functions)
	}
}

func TestProjectRejectsInvalidSourceRanges(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const value = 1;\n")
	project := openProject(t, root)
	path := filepath.Join(root, "source.ts")

	for _, location := range []typefacts.Location{
		{Path: path, StartByte: -1, EndByte: 0},
		{Path: path, StartByte: 10, EndByte: 9},
		{Path: path, StartByte: 0, EndByte: 1000},
	} {
		if _, err := project.SymbolAt(context.Background(), location); err == nil || !strings.Contains(err.Error(), "byte range") {
			t.Errorf("SymbolAt(%#v) error = %v, want byte range error", location, err)
		}
	}
}

func TestProjectSupportsProjectReferencesAndMixedJavaScript(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "core/tsconfig.json", `{
  "compilerOptions": {"composite": true, "strict": true, "module": "ESNext", "moduleResolution": "Bundler"},
  "include": ["source.ts", "identity.js"]
}`)
	writeProjectFile(t, root, "core/source.ts", "export const referenced = () => 1;\n")
	writeProjectFile(t, root, "core/identity.js", `
/** @template T @param {T} value @returns {T} */
export function identity(value) { return value; }
`)
	writeProjectFile(t, root, "app/tsconfig.json", `{
  "compilerOptions": {
    "composite": true,
    "strict": true,
    "allowJs": true,
    "checkJs": true,
    "module": "ESNext",
    "moduleResolution": "Bundler"
  },
  "references": [{"path": "../core"}],
  "include": ["app.ts"]
}`)
	writeProjectFile(t, root, "app/app.ts", `
import { referenced } from "../core/source";
import { identity } from "../core/identity";
referenced();
identity("mixed");
`)

	project, err := tsgo.OpenProject(context.Background(), filepath.Join(root, "app", "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = project.Close() })
	path := filepath.Join(root, "app", "app.ts")
	for callText, want := range map[string]string{
		"referenced()":      "number",
		`identity("mixed")`: `"mixed"`,
	} {
		call, err := project.ResolvedCall(context.Background(), locationOf(t, path, callText))
		if err != nil {
			t.Fatalf("ResolvedCall(%s): %v", callText, err)
		}
		if call.ReturnTypeText != want {
			t.Errorf("ResolvedCall(%s) return type = %q, want %q", callText, call.ReturnTypeText, want)
		}
	}
}

func TestProjectHonorsCanceledContexts(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const value = 1;\n")
	project := openProject(t, root)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := project.SourceFiles(ctx); !errors.Is(err, context.Canceled) {
		t.Fatalf("SourceFiles() error = %v, want context.Canceled", err)
	}
}

func TestProjectDurableIdentitiesNeverMisresolveAcrossUpdates(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const value = 1;\n")
	project := openProject(t, root)
	path := filepath.Join(root, "source.ts")
	location := locationOf(t, path, "value =")
	location.EndByte = location.StartByte + len("value")

	heldID, err := project.SymbolAt(context.Background(), location)
	if err != nil {
		t.Fatal(err)
	}

	// An update that leaves the declaration name unmoved keeps the durable
	// identity resolvable, and it resolves to the same declaration.
	_, err = project.Update(context.Background(), []typefacts.FileChange{{
		Path: path, Version: 1, Source: []byte("export const value = 2;\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	declarations, err := project.Declarations(context.Background(), heldID)
	if err != nil {
		t.Fatalf("Declarations(held ID) after unmoved-declaration update: %v", err)
	}
	if len(declarations) != 1 || declarations[0].Name != "value" {
		t.Fatalf("held ID resolved to %+v, want the value declaration", declarations)
	}

	// An update that replaces the declaration at that span must fail closed:
	// the held identity reports not-found rather than resolving to the new,
	// different symbol.
	_, err = project.Update(context.Background(), []typefacts.FileChange{{
		Path: path, Version: 2, Source: []byte("export const other = 3;\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := project.Declarations(context.Background(), heldID); !errors.Is(err, typefacts.ErrNotFound) {
		t.Fatalf("Declarations(held ID) after declaration replacement error = %v, want ErrNotFound", err)
	}
}

func TestProjectFailedUpdateIsTransactional(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const value = () => 1;\n")
	writeProjectFile(t, root, "use.ts", "import { value } from './source';\nvalue();\n")
	project := openProject(t, root)

	if _, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path: filepath.Join(root, "tsconfig.json"), Version: 1, Source: []byte(`{"compilerOptions":{"module":"NOT_A_MODULE"}}`),
	}}); err == nil {
		t.Fatal("invalid tsconfig update succeeded")
	}
	if _, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path: filepath.Join(root, "source.ts"), Version: 1, Source: []byte("export const value = () => 'ok';\n"),
	}}); err != nil {
		t.Fatalf("valid update after rejected update = %v, want success", err)
	}
	call, err := project.ResolvedCall(context.Background(), locationOf(t, filepath.Join(root, "use.ts"), "value()"))
	if err != nil {
		t.Fatal(err)
	}
	if call.ReturnTypeText != "string" {
		t.Fatalf("return type = %q, want string", call.ReturnTypeText)
	}
}

func TestProjectOverlayAndCleanBuildProduceEquivalentPublicFacts(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const value = () => 1;\n")
	writeProjectFile(t, root, "use.ts", "import { value } from './source';\nvalue();\n")
	overlay := openProject(t, root)
	updated := []byte("export const value = () => 'updated';\n")
	if _, err := overlay.Update(context.Background(), []typefacts.FileChange{{
		Path: filepath.Join(root, "source.ts"), Version: 1, Source: updated,
	}}); err != nil {
		t.Fatal(err)
	}

	writeProjectFile(t, root, "source.ts", string(updated))
	clean := openProject(t, root)
	callLocation := locationOf(t, filepath.Join(root, "use.ts"), "value()")
	overlayCall, err := overlay.ResolvedCall(context.Background(), callLocation)
	if err != nil {
		t.Fatal(err)
	}
	cleanCall, err := clean.ResolvedCall(context.Background(), callLocation)
	if err != nil {
		t.Fatal(err)
	}
	if overlayCall.ReturnTypeText != cleanCall.ReturnTypeText {
		t.Fatalf("overlay return type = %q, clean = %q", overlayCall.ReturnTypeText, cleanCall.ReturnTypeText)
	}

	overlayFiles, err := overlay.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	cleanFiles, err := clean.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(overlayFiles) != len(cleanFiles) {
		t.Fatalf("overlay files = %d, clean = %d", len(overlayFiles), len(cleanFiles))
	}
	for index := range overlayFiles {
		if overlayFiles[index].Path != cleanFiles[index].Path || string(overlayFiles[index].Source) != string(cleanFiles[index].Source) {
			t.Fatalf("source file %d differs: overlay=%#v clean=%#v", index, overlayFiles[index], cleanFiles[index])
		}
	}
}

func TestProjectImportChangeFallsBackToEquivalentProgram(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true,"module":"ESNext","moduleResolution":"Bundler"},"include":["*.ts"]}`)
	writeProjectFile(t, root, "number.ts", "export const value = () => 1;\n")
	writeProjectFile(t, root, "string.ts", "export const value = () => 'updated';\n")
	writeProjectFile(t, root, "use.ts", "import { value } from './number';\nvalue();\n")
	project := openProject(t, root)
	usePath := filepath.Join(root, "use.ts")
	updated := []byte("import { value } from './string';\nvalue();\n")

	if _, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path: usePath, Version: 1, Source: updated,
	}}); err != nil {
		t.Fatal(err)
	}
	call, err := project.ResolvedCall(context.Background(), typefacts.Location{
		Path: usePath, StartByte: strings.Index(string(updated), "value()"), EndByte: len(updated),
	})
	if err != nil {
		t.Fatal(err)
	}
	if call.ReturnTypeText != "string" {
		t.Fatalf("return type after import change = %q, want string", call.ReturnTypeText)
	}
}

func TestProjectOverlayAddsDeletesAndVersionsProjectFiles(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	writeProjectFile(t, root, "source.ts", "export const source = 1;\n")
	project := openProject(t, root)
	addedPath := filepath.Join(root, "added.ts")

	affected, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path: addedPath, Version: 1, Source: []byte("export const added = 1;\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(affected.Files) != 1 || affected.Files[0] != addedPath {
		t.Fatalf("affected files = %#v, want added file", affected.Files)
	}
	assertSourcePresence(t, project, "added.ts", true)

	affected, err = project.Update(context.Background(), []typefacts.FileChange{{
		Path: addedPath, Version: 1, Source: []byte("export const stale = 2;\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(affected.Files) != 0 {
		t.Fatalf("same-version affected files = %#v, want none", affected.Files)
	}
	files, err := project.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	for _, file := range files {
		if filepath.Base(file.Path) == "added.ts" && strings.Contains(string(file.Source), "stale") {
			t.Fatal("same-version overlay replaced accepted source")
		}
	}

	if _, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path: addedPath, Version: 2, Deleted: true,
	}}); err != nil {
		t.Fatal(err)
	}
	assertSourcePresence(t, project, "added.ts", false)
}

func newAdvancedProject(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "baseUrl": ".",
    "paths": { "@lib/*": ["lib/*"] }
  },
  "include": ["use.ts", "lib/**/*.ts"]
}`)
	writeProjectFile(t, root, "lib/value.ts", `
export default function identity<T>(value: T): T { return value; }
export function choose(value: string): string;
export function choose<T>(value: T): T;
export function choose<T>(value: T): T { return value; }
`)
	writeProjectFile(t, root, "lib/barrel.ts", `export { default as identity, choose as renamed } from "./value";`)
	writeProjectFile(t, root, "lib/index.ts", `export { identity, renamed as choose } from "./barrel";`)
	writeProjectFile(t, root, "packages/fixture-pkg/package.json", `{"name":"fixture-pkg","exports":{"./feature":{"types":"./feature.d.ts"}}}`)
	writeProjectFile(t, root, "packages/fixture-pkg/feature.d.ts", `export declare function feature<T>(value: T): T;`)
	if err := os.MkdirAll(filepath.Join(root, "node_modules"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(root, "packages", "fixture-pkg"), filepath.Join(root, "node_modules", "fixture-pkg")); err != nil {
		t.Skipf("create package-manager-style symlink: %v", err)
	}
	writeProjectFile(t, root, "use.ts", `
import * as api from "@lib/index";
import { feature } from "fixture-pkg/feature";
api.identity(true);
api.choose(42);
feature('ok');
`)
	return root
}

func openProject(t *testing.T, root string) typefacts.Project {
	t.Helper()
	project, err := tsgo.OpenProject(context.Background(), filepath.Join(root, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = project.Close() })
	return project
}

func writeProjectFile(t *testing.T, root, relative, source string) {
	t.Helper()
	path := filepath.Join(root, relative)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
}

func assertSourcePresence(t *testing.T, project typefacts.Project, basename string, want bool) {
	t.Helper()
	files, err := project.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, file := range files {
		if filepath.Base(file.Path) == basename {
			found = true
		}
	}
	if found != want {
		t.Fatalf("source %s presence = %v, want %v; files=%#v", basename, found, want, files)
	}
}
