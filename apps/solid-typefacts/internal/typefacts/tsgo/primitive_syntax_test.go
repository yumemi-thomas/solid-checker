package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0145, handshake protocol 66: a return site states that its value is a
// primitive by grammar only when no binding it reads can make it anything
// else. A read of a captured `let` is typed by its declaration in a JavaScript
// file whatever was written to it, so it is never one.
func TestReturnSitesStatePrimitiveSyntax(t *testing.T) {
	cases := []struct {
		name, code string
		want       bool
	}{
		{"number", `export const check = () => 1;`, true},
		{"template", "export const check = (v: any) => `${v}-x`;", true},
		{"negation", `export const check = (v: any) => !v;`, true},
		{"increment", `export function check() { let n = 0; return ++n; }`, true},
		{"postfix", `export function check() { let n = 0; return n++; }`, true},
		{"typeof", `export const check = (v: any) => typeof v;`, true},
		{"void", `export const check = () => void 0;`, true},
		{"sum", `export const check = (a: any, b: any) => a + b;`, true},
		{"comparison", `export const check = (a: any) => a === 1;`, true},
		{"compound", `export function check(a: any) { let n = 0; return (n += a); }`, true},
		{"conditionalOfLiterals", `export const check = (c: any) => c ? 1 : "one";`, true},
		{"logicalOfLiterals", `export const check = (c: any) => c && 1;`, false},
		{"logicalBothLiterals", `export const check = () => 0 || "x";`, true},
		{"identifier", `export function check() { let n = 0; n = 2; return n; }`, false},
		{"member", `export const check = (o: any) => o.n;`, false},
		{"call", `export const check = () => Math.random();`, false},
		{"object", `export const check = () => ({});`, false},
		{"assignmentOfIdentifier", `export function check(o: any) { let n; return (n = o); }`, false},
		{"tagged", "export const check = (t: any) => t`x`;", false},
		{"undefinedIdentifier", `export const check = () => undefined;`, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := tc.code + "\nvoid check;\n"
			writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
			p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer p.Close()
			path := filepath.Join(dir, "facts.ts")
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
			flow := answer.Transcripts[0].Implementation.ControlFlow
			if flow == nil || len(flow.Returns) != 1 {
				t.Fatalf("returns = %+v", flow)
			}
			if got := flow.Returns[0].PrimitiveSyntax; got != tc.want {
				t.Fatalf("primitiveSyntax = %v, want %v", got, tc.want)
			}
		})
	}
}

// ADR 0146, handshake protocol 66: a call-result source states, per argument
// of the traced call, whether it is a primitive by grammar, and a return site
// names the call expression it hands back.
func TestCallResultSourcesStateTheirArgumentsAndReturnsTheirCall(t *testing.T) {
	source := `function mk(value?: any, options?: any): [() => any, (v: any) => void] { return [() => value, () => {}]; }
export function check(x: any) {
  const [a] = mk(0);
  const [b] = mk(x);
  const [c] = mk();
  const [d] = mk(1, {});
  b();
  c();
  d();
  return a();
}
void check;
`
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer p.Close()
	path := filepath.Join(dir, "facts.ts")
	start := strings.LastIndex(source, "check")
	impl := strings.Index(source, "check")
	answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location:               typefacts.Location{Path: path, StartByte: start, EndByte: start + 5},
		ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + 5},
	}})
	if err != nil {
		t.Fatal(err)
	}
	implementation := answer.Transcripts[0].Implementation
	if implementation == nil {
		t.Fatal("missing implementation census")
	}
	want := map[string][]bool{"a()": {true}, "b()": {false}, "c()": {}, "d()": {true, false}}
	seen := 0
	for _, call := range implementation.Calls {
		text := source[call.Location.StartByte:call.Location.EndByte]
		expected, ok := want[text]
		if !ok {
			continue
		}
		seen++
		if len(call.CalleeSources) != 1 {
			t.Fatalf("%s: callee sources = %+v", text, call.CalleeSources)
		}
		got := call.CalleeSources[0].ArgumentsPrimitiveSyntax
		if len(got) != len(expected) {
			t.Fatalf("%s: argumentsPrimitiveSyntax = %v, want %v", text, got, expected)
		}
		for index := range expected {
			if got[index] != expected[index] {
				t.Fatalf("%s: argumentsPrimitiveSyntax = %v, want %v", text, got, expected)
			}
		}
	}
	if seen != len(want) {
		t.Fatalf("saw %d of the %d calls", seen, len(want))
	}
	flow := implementation.ControlFlow
	if flow == nil || len(flow.Returns) != 1 || flow.Returns[0].Call == nil {
		t.Fatalf("returns = %+v", flow)
	}
	if text := source[flow.Returns[0].Call.StartByte:flow.Returns[0].Call.EndByte]; text != "a()" {
		t.Fatalf("returned call = %q", text)
	}
}
