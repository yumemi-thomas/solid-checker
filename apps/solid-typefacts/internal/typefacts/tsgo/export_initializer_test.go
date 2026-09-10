package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

func TestExportInitializerRequiresExactUnwrittenAliasChain(t *testing.T) {
	cases := []struct {
		name, code string
		bindings   int
	}{
		{"direct", `export const result = factory({});`, 1},
		{"alias", `var plugin = factory({}); var result = plugin; export {result};`, 2},
		{"wrappers", `var plugin = factory(({} as object)); var result = (plugin); export {result};`, 2},
		{"mutated", `var result = factory({}); result = {}; export {result};`, 0},
		{"aliasMutation", `var plugin = factory({}); var result = plugin; plugin = {}; export {result};`, 0},
		{"forward", `var result = plugin; var plugin = factory({}); export {result};`, 0},
		{"cycle", `var result = plugin; var plugin = result; export {result};`, 0},
		{"eval", `var result = factory({}); eval('result = {}'); export {result};`, 0},
		{"nestedEval", `var result = factory({}); function change() { eval('result = {}'); } export {result};`, 0},
		{"redeclared", `var result = factory({}); var result; export {result};`, 0},
		{"callableArgument", `export const result = factory(() => {});`, 0},
		{"spread", `export const result = factory(...[{}]);`, 0},
		{"optional", `export const result = factory?.({});`, 0},
		{"localFactory", `const local = (v: any) => v; export const result = local({});`, 0},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := "import { createPlugin as factory } from './factory';\n" + tc.code + "\nvoid result;\n"
			writeInvocationProject(t, dir, map[string]string{"index.ts": source, "factory.ts": `export function createPlugin<T>(value: T): T { return value; }`})
			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			p := opened.(*project)
			p.mu.Lock()
			defer p.mu.Unlock()
			start := strings.LastIndex(source, "result")
			file, err := p.sourceFileFor(typefacts.Location{Path: filepath.Join(dir, "index.ts"), StartByte: start, EndByte: start + 6})
			if err != nil {
				t.Fatal(err)
			}
			cursor := semanticNodeCursor{sourceFile: file}
			got := p.exportInitializerLocked(cursor.exactExpressionAt(start, start+6))
			if tc.bindings == 0 {
				if got != nil {
					t.Fatal("unsupported initializer gained a derivation")
				}
				return
			}
			if got == nil || len(got.bindings) != tc.bindings || got.call == nil || got.callee == nil || got.specifier != "./factory" || got.exportName != "createPlugin" || got.objectArguments[0] == nil {
				t.Fatalf("incomplete or wrong derivation: %+v", got)
			}
		})
	}
}

func TestExportInitializerTranscriptBindsImportedRuntimeQuery(t *testing.T) {
	dir := t.TempDir()
	harness := "import { result as query } from './runtime';\nvoid query;"
	runtime := "import { createPlugin as factory } from './factory';\nconst plugin = factory({}, 0, {});\nconst result: any = plugin;\nexport {result};"
	writeInvocationProject(t, dir, map[string]string{
		"harness.ts": harness,
		"runtime.ts": runtime,
		"factory.ts": `export function createPlugin<T>(value: T, ignored: number, other: object): T { return value; }`,
	})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.LastIndex(harness, "query")
	location := typefacts.Location{Path: filepath.Join(dir, "harness.ts"), StartByte: start, EndByte: start + 5}
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &location}})
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	got := transcript.Initializer
	if got == nil || got.Location != location || len(got.Bindings) != 2 || got.Target != transcript.Implementation.Target || got.Bindings[0].Declaration.Symbol != got.Target {
		t.Fatalf("initializer does not bind the runtime query: %+v", got)
	}
	if got.Bindings[0].Declaration.Name != "result" || got.Bindings[1].Declaration.Name != "plugin" || got.Call.Path != filepath.Join(dir, "runtime.ts") || got.Declaration.Location.Path != filepath.Join(dir, "factory.ts") || got.Specifier != "./factory" || got.ExportName != "createPlugin" {
		t.Fatalf("initializer lost exact runtime or dependency identity: %+v", got)
	}
	if len(got.ObjectArguments) != 2 || got.ObjectArguments[0].Index != 0 || got.ObjectArguments[1].Index != 2 || got.ObjectArguments[0].Location.StartByte >= got.ObjectArguments[1].Location.StartByte {
		t.Fatalf("object argument indexes/order changed: %+v", got.ObjectArguments)
	}
	if transcript.Value.Callability != typefacts.CallabilityUnknown || transcript.Value.Constructability != typefacts.InvocationConstructUnknown {
		t.Fatal("initializer source derivation became an unproved result verdict")
	}
	encoded, err := wirecbor.Marshal(transcript)
	if err != nil {
		t.Fatal(err)
	}
	var decoded typefacts.ExportValueTranscript
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	if decoded.Initializer == nil || decoded.Initializer.ObjectArguments[1].Index != 2 {
		t.Fatal("initializer lost in CBOR round trip")
	}
	unasked, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: location}})
	if err != nil {
		t.Fatal(err)
	}
	if unasked.Transcripts[0].Initializer != nil {
		t.Fatal("initializer emitted without a runtime demand")
	}
}
