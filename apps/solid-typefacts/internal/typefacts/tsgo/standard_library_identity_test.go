package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0149, handshake protocol 68: a call the
// checker resolves into the default library states that it invokes that
// declaration by identity only when no binding's declared type is what names
// the built-in. In a JavaScript file `let m = Math; m = { min }` keeps `m`
// typed `Math`, so `m.min()` resolves to `Math.min` and runs other code.
func TestDefaultLibraryCallsStateTheirIdentity(t *testing.T) {
	cases := []struct {
		name, prelude, body, call string
		identity                  bool
	}{
		{"globalMember", "", "return Math.min(a, 1);", "Math.min(a, 1)", true},
		{"globalFunction", "", "setTimeout(a, 0);", "setTimeout(a, 0)", true},
		{"prototypeChain", "", "return Object.prototype.toString.call(a);", "Object.prototype.toString.call(a)", true},
		{"stringLiteral", "", `return "a,b".split(",");`, `"a,b".split(",")`, true},
		{"constString", `const text = "a,b";`, `return text.split(",");`, `text.split(",")`, true},
		{"arrayLiteral", "", "return [1, 2].map(a);", "[1, 2].map(a)", true},
		{"regularExpression", "", "return /x/.test(a);", "/x/.test(a)", true},
		{"freshMap", "", "return new Map().get(a);", "new Map().get(a)", true},
		{"freshMapConstruction", "", "return new Map().get(a);", "new Map()", true},
		{"freshNamespacedConstruction", "", "return new Intl.Locale(a).maximize();", "new Intl.Locale(a).maximize()", true},
		// The hole: a binding typed by its declaration, holding something else.
		{"reassignedGlobalAlias", "", "let m = Math; m = { min: a }; return m.min(1);", "m.min(1)", false},
		{"reassignedString", "", `let s = ""; s = { split: a }; return s.split(",");`, `s.split(",")`, false},
		// A const binding holds the global, but the global escapes through it.
		{"constGlobalAlias", "", "const m = Math; return m.min(1);", "m.min(1)", false},
		{"writtenGlobalMember", "", "Math.min = a; return Math.min(1);", "Math.min(1)", false},
		{"writtenGlobalFunction", "", "setTimeout = a; setTimeout(a, 0);", "setTimeout(a, 0)", false},
		// A member of a fresh value's member is not a value the expression made.
		{"freshValueMemberChain", "", `return "ab".length.toFixed();`, `"ab".length.toFixed()`, false},
		{"optionalCall", "", "return Math.min?.(a);", "Math.min?.(a)", false},
		// A built-in's primitive result: its method is the wrapper prototype's.
		{"primitiveResult", "", "return Date.now().toString(36);", "Date.now().toString(36)", true},
		{"primitiveResultChain", "", "return Math.random().toString(36).slice(2);", "Math.random().toString(36).slice(2)", true},
		// A const holding a fresh array nothing else can reach.
		{"freshConst", "", "const values = Array.from(a); return values.map(a);", "values.map(a)", true},
		{"freshConstOfLiteral", "", "const out = []; out.push(1); return out.join();", "out.join()", true},
		// ...and one that escapes, or whose member is written.
		{"escapedConst", "", "const values = Array.from(a); a(values); return values.map(a);", "values.map(a)", false},
		{"memberWrittenConst", "", "const values = []; values.map = a; return values.map(a);", "values.map(a)", false},
		{"elementWrittenConst", "", "const values = []; values[0] = 1; return values.map(a);", "values.map(a)", false},
		// A global called, constructed or tested against hands nothing out.
		{"globalAlsoCalled", "", "const n = Array(2); return Array.isArray(a) || a instanceof Array || typeof Array;", "Array.isArray(a)", true},
		{"globalHandedOut", "", "a(Array); return Array.isArray(a);", "Array.isArray(a)", false},
		// A let written only with fresh values, reached only through its members.
		{"freshLet", "", "let stack = []; const clear = () => stack = []; clear(); return stack.push(a);", "stack.push(a)", true},
		{"letWrittenWithObject", "", "let stack = [1]; stack = { push: a }; return stack.push(1);", "stack.push(1)", false},
		{"returnedConst", "", "const out = []; out.push(a); return out;", "out.push(a)", false},
		// A result whose primitive type is only an instantiation over a binding.
		{"instantiatedResult", "", "let x = 1; x = { toFixed: a }; return [x].at(0).toFixed();", "[x].at(0).toFixed()", false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := tc.prelude + "\nexport function check(a) { " + tc.body + " }\nvoid check;\n"
			if err := os.WriteFile(
				filepath.Join(dir, "tsconfig.json"),
				[]byte(`{"compilerOptions":{"strict":true,"allowJs":true,"checkJs":false,"module":"esnext","target":"esnext","moduleResolution":"bundler","lib":["esnext","dom"]},"include":["*.js"]}`),
				0o644,
			); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(dir, "facts.js"), []byte(source), 0o644); err != nil {
				t.Fatal(err)
			}
			p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer p.Close()
			path := filepath.Join(dir, "facts.js")
			start := strings.LastIndex(source, "check")
			impl := strings.Index(source, "check")
			answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
				Location:               typefacts.Location{Path: path, StartByte: start, EndByte: start + 5},
				ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + 5},
			}})
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatal("missing implementation census")
			}
			var found *typefacts.ImplementationCall
			for index, call := range answer.Transcripts[0].Implementation.Calls {
				if source[call.Location.StartByte:call.Location.EndByte] == tc.call {
					found = &answer.Transcripts[0].Implementation.Calls[index]
				}
			}
			if found == nil {
				t.Fatalf("no call %q among %+v", tc.call, answer.Transcripts[0].Implementation.Calls)
			}
			if found.Declaration == nil || !found.Declaration.StandardLibrary {
				t.Fatalf("%s: declaration = %+v, want the default library", tc.call, found.Declaration)
			}
			if found.StandardLibraryIdentity != tc.identity {
				t.Fatalf("%s: standardLibraryIdentity = %v, want %v", tc.call, found.StandardLibraryIdentity, tc.identity)
			}
		})
	}
}
