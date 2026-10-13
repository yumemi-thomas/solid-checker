package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestAConstructionStatesWhatItKeepsForItsMembers pins ADR 0139's producer
// census. A parameter is stated only when its one store is a top-level
// `this.<key> = p` of the constructor, every other use a direct call there,
// the key is written nowhere else, and every read of it is a member call no
// construction reaches -- in a class that is exact: nothing extends or
// augments it, the instance never escapes, no member has a computed key, and
// no hidden function is hung on the instance. Each refusing row fails on a
// census that dropped the premise it names.
func TestAConstructionStatesWhatItKeepsForItsMembers(t *testing.T) {
	type kept struct {
		index       int
		key         string
		invocations int
	}
	const member = "\trun(value) {\n\t\treturn this.callback(value);\n\t}\n"
	for _, testCase := range []struct {
		name    string
		runtime string
		want    []kept
		why     string
	}{
		{
			name:    "keptForAMember",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			want:    []kept{{0, "callback", 1}},
			why:     "the one store is a top-level statement, and only a member nothing at construction reaches calls the key",
		},
		{
			name:    "calledOnceAndKept",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tcallback(0);\n\t\tthis.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			want:    []kept{{0, "callback", 1}},
			why:     "a direct call in the constructor's own frame is ADR 0100's item, confirmed by the call census on its own",
		},
		{
			name:    "twoParametersOneUnread",
			runtime: "var C = class {\n\tconstructor(options, callback) {\n\t\tthis.options = options;\n\t\tthis.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			want:    []kept{{0, "options", 0}, {1, "callback", 1}},
			why:     "a kept value no member reads is kept all the same",
		},
		{
			name:    "anInstalledMemberCallsIt",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\tthis.fire = () => this.callback(1);\n\t}\n};\nexport { C };\n",
			want:    []kept{{0, "callback", 1}},
			why:     "a closure the constructor installs is a member, and nothing at construction names it",
		},
		{
			name:    "aMemberConstructionReaches",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\tthis.run(0);\n\t}\n" + member + "};\nexport { C };\n",
			why:     "the constructor calls `run`, which calls the key: a construction-time invocation no item describes",
		},
		{
			name:    "anInstalledMemberConstructionCalls",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\tthis.fire = () => this.callback(1);\n\t\tthis.fire();\n\t}\n};\nexport { C };\n",
			why:     "router-core's shape: the installed `update` is called by the constructor and calls the key",
		},
		{
			name:    "aClosureTheConstructorHandsOn",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\t[1].forEach(() => this.callback(0));\n\t}\n};\nexport { C };\n",
			why:     "a closure the constructor creates without installing it may run during construction",
		},
		{
			name:    "aMemberHandsItOn",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t}\n\tlater() {\n\t\treturn [1].map(this.callback);\n\t}\n};\nexport { C };\n",
			why:     "a read of the key that is not a call hands the callable on",
		},
		{
			name:    "theKeyIsRewritten",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t}\n\treset(next) {\n\t\tthis.callback = next;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "a second write means what a member calls is not the argument",
		},
		{
			name:    "theInstanceEscapes",
			runtime: "const registry = [];\nvar C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\tregistry.push(this);\n\t}\n" + member + "};\nexport { C };\n",
			why:     "code that never received the constructed value can reach the callable through the registry",
		},
		{
			name:    "thePrototypeIsAugmented",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t}\n};\nC.prototype.run = function (value) {\n\treturn this.callback(value);\n};\nexport { C };\n",
			why:     "a member defined outside the class body is a reference to the class this census does not read",
		},
		{
			name:    "aHiddenMember",
			runtime: "function helper() {\n\treturn [this.callback];\n}\nvar C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\tthis.helper = helper;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "a function stored on the instance by name runs later as a member, with a body this census does not see",
		},
		{
			name:    "aComputedMember",
			runtime: "var C = class {\n\tconstructor(callback, key) {\n\t\tthis.callback = callback;\n\t\tthis[key] = 1;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "a computed member of `this` could be the key",
		},
		{
			name:    "theConstructorReturnsAValue",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t\treturn {};\n\t}\n" + member + "};\nexport { C };\n",
			why:     "`new` hands the caller the returned object, not the instance",
		},
		{
			name:    "aDefaultedParameter",
			runtime: "var C = class {\n\tconstructor(callback = () => 0) {\n\t\tthis.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "the value kept may be the default, not the caller's",
		},
		{
			name:    "aConditionalStore",
			runtime: "var C = class {\n\tconstructor(callback) {\n\t\tif (callback) this.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "only a top-level statement of the constructor is the store; the test reads the argument",
		},
		{
			name:    "aStaticMember",
			runtime: "var C = class {\n\tstatic make(callback) {\n\t\treturn new C(callback);\n\t}\n\tconstructor(callback) {\n\t\tthis.callback = callback;\n\t}\n" + member + "};\nexport { C };\n",
			why:     "a static member is class-value code, which no reviewed shape needs",
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			dir := t.TempDir()
			write := func(name, source string) {
				if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false},"files":["harness.ts","index.js","index.d.ts"]}`)
			write("index.d.ts", "export declare class C {\n\tconstructor(callback: (value: number) => number, other?: unknown);\n}\n")
			write("index.js", testCase.runtime)
			harness := "import { C } from \"./index.js\";\nvoid C;\n"
			write("harness.ts", harness)
			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			analyzer := opened.(typefacts.ExportValueAnalyzer)
			queryStart := strings.LastIndex(harness, "void C") + len("void ")
			implStart := strings.Index(testCase.runtime, "var C") + len("var ")
			answer, err := analyzer.ExportValueTranscripts(context.Background(),
				[]typefacts.ExportValueDemand{{
					Location: typefacts.Location{
						Path:      filepath.Join(dir, "harness.ts"),
						StartByte: queryStart, EndByte: queryStart + len("C"),
					},
					ImplementationLocation: &typefacts.Location{
						Path:      filepath.Join(dir, "index.js"),
						StartByte: implStart, EndByte: implStart + len("C"),
					},
				}})
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatalf("transcripts = %#v, want one with an implementation", answer.Transcripts)
			}
			got := answer.Transcripts[0].Implementation
			if got.Invocation != typefacts.CallKindConstruct {
				t.Fatalf("invocation = %q (open %v), want a construction", got.Invocation, got.OpenReasons)
			}
			var stated []kept
			for _, argument := range got.RetainedArguments {
				stated = append(stated, kept{argument.ParameterIndex, argument.Key, len(argument.Invocations)})
				if at := testCase.runtime[argument.Store.StartByte:argument.Store.EndByte]; at != "callback" && at != "options" {
					t.Fatalf("store names %q, want the parameter's own reference", at)
				}
			}
			if !reflect.DeepEqual(stated, testCase.want) {
				t.Fatalf("retained = %v, want %v: %s", stated, testCase.want, testCase.why)
			}
		})
	}
}
