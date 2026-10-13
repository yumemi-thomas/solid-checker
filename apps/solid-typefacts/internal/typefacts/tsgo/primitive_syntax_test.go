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
		// The 2026-09-28 amendment to ADR 0149: a `let` every value of which
		// is a primitive by grammar is one; one written an object is not.
		{"primitiveLet", `export function check() { let n = 0; n = 2; return n; }`, true},
		{"accumulatorLet", `export function check(a: any) { let r = 0; for (const x of a) r += x; return r; }`, true},
		{"objectWrittenLet", `export function check() { let n = 0; n = ({} as any); return n; }`, false},
		{"loopHeadWrittenLet", `export function check(a: any) { let n = 0; for (n of a); return n; }`, false},
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

// ADR 0168, handshake protocol 74: a call states, per written argument, whether
// the expression is a primitive by grammar, and a spread -- and every slot it
// displaces -- is false, because the runtime value at that position is not the
// one written there.
func TestCallStatesItsArgumentsPrimitiveSyntax(t *testing.T) {
	source := `declare function mk(...args: any[]): any;
declare function untrackLike(fn: () => any): any;
export function check(x: any, list: any[]) {
  mk(0);
  mk(x);
  mk();
  mk(1, {});
  mk("a", -1, true, null, undefined);
  mk(() => x());
  mk(...list);
  mk(0, ...list, 1);
  untrackLike(() => x());
  new Map([[0, 1]]);
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
	want := map[string][]bool{
		"mk(0)":                              {true},
		"mk(x)":                              {false},
		"mk()":                               nil,
		"mk(1, {})":                          {true, false},
		`mk("a", -1, true, null, undefined)`: {true, true, true, true, true},
		"mk(() => x())":                      {false},
		"mk(...list)":                        {false},
		"mk(0, ...list, 1)":                  {true, false, false},
		"untrackLike(() => x())":             {false},
	}
	seen := map[string]bool{}
	for _, call := range implementation.Calls {
		text := source[call.Location.StartByte:call.Location.EndByte]
		if call.Kind == typefacts.CallKindConstruct {
			if len(call.ArgumentsPrimitiveSyntax) != 0 {
				t.Fatalf("%s: a construction states none, got %v", text, call.ArgumentsPrimitiveSyntax)
			}
			continue
		}
		expected, ok := want[text]
		if !ok {
			continue
		}
		seen[text] = true
		got := call.ArgumentsPrimitiveSyntax
		if len(got) != len(expected) {
			t.Fatalf("%s: argumentsPrimitiveSyntax = %v, want %v", text, got, expected)
		}
		for index := range expected {
			if got[index] != expected[index] {
				t.Fatalf("%s: argumentsPrimitiveSyntax = %v, want %v", text, got, expected)
			}
		}
		// One entry per written argument, like ArgumentParameters.
		if len(call.ArgumentParameters) != len(got) {
			t.Fatalf("%s: %d primitive entries beside %d argument slots", text, len(got), len(call.ArgumentParameters))
		}
	}
	if len(seen) != len(want) {
		t.Fatalf("saw %v of %d calls", seen, len(want))
	}
}

// ADR 0162, handshake protocol 76: a call-result source states, per written
// argument, whether its grammar alone proves the value is not a function, and
// whether it is a plain options literal. Each is a fact about the written
// expression and never about a binding: an identifier, a member, a call, a
// construction and a function are all "not stated true", and a spread is false.
func TestCallResultSourcesStateNotFunctionAndPlainOptionsSyntax(t *testing.T) {
	cases := []struct {
		name, arguments string
		notFunction     []bool
		plainOptions    []bool
	}{
		{"primitive", `0`, []bool{true}, []bool{false}},
		{"undefined", `void 0, { ownedWrite: true }`, []bool{true, true}, []bool{false, true}},
		{"emptyArray", `[]`, []bool{true}, []bool{false}},
		{"arrayOfAnything", `[x, () => 1]`, []bool{true}, []bool{false}},
		{"objectLiteral", `{ a: x }`, []bool{true}, []bool{false}},
		{"emptyOptions", `0, {}`, []bool{true, true}, []bool{false, true}},
		{"flagOptions", `0, { ownedWrite: true, name: "n", 3: 1 }`, []bool{true, true}, []bool{false, true}},
		{"falseEquals", `0, { equals: false }`, []bool{true, true}, []bool{false, true}},
		{"optionsWithFunction", `0, { equals: () => true }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithBinding", `0, { ownedWrite: x }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithSpread", `0, { ...x }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithShorthand", `0, { x }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithComputedKey", `0, { [x]: 1 }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithMethod", `0, { equals() { return true; } }`, []bool{true, true}, []bool{false, false}},
		{"optionsWithAccessor", `0, { get equals() { return x; } }`, []bool{true, true}, []bool{false, false}},
		{"nestedObjectValue", `0, { a: { b: 1 } }`, []bool{true, true}, []bool{false, false}},
		{"binding", `x`, []bool{false}, []bool{false}},
		{"member", `x.value`, []bool{false}, []bool{false}},
		{"call", `x()`, []bool{false}, []bool{false}},
		{"arrow", `() => 1`, []bool{false}, []bool{false}},
		{"functionExpression", `function () { return 1; }`, []bool{false}, []bool{false}},
		{"construction", `new Map()`, []bool{false}, []bool{false}},
		{"spread", `...list`, []bool{false}, []bool{false}},
		{"spreadBeforeOptions", `...list, { equals: false }`, []bool{false, true}, []bool{false, true}},
		{"wrappedLiteral", `([] as any)`, []bool{true}, []bool{false}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			source := "declare function mk(...args: any[]): any;\nexport function check(x: any, list: any[]) {\n  return mk(" + tc.arguments + ");\n}\nvoid check;\n"
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
			if implementation == nil || implementation.ControlFlow == nil || len(implementation.ControlFlow.Returns) != 1 {
				t.Fatal("missing return census")
			}
			var source0 *typefacts.ImplementationValueSource
			for index := range implementation.ControlFlow.Returns[0].Sources {
				candidate := &implementation.ControlFlow.Returns[0].Sources[index]
				if candidate.Kind == typefacts.ImplementationValueCallResult && len(candidate.Path) == 0 {
					source0 = candidate
				}
			}
			if source0 == nil {
				t.Fatalf("no call-result source in %+v", implementation.ControlFlow.Returns[0].Sources)
			}
			equal := func(got, want []bool) bool {
				if len(got) != len(want) {
					return false
				}
				for index := range want {
					if got[index] != want[index] {
						return false
					}
				}
				return true
			}
			if !equal(source0.ArgumentsNotFunctionSyntax, tc.notFunction) {
				t.Fatalf("argumentsNotFunctionSyntax = %v, want %v", source0.ArgumentsNotFunctionSyntax, tc.notFunction)
			}
			if !equal(source0.ArgumentsPlainOptionsSyntax, tc.plainOptions) {
				t.Fatalf("argumentsPlainOptionsSyntax = %v, want %v", source0.ArgumentsPlainOptionsSyntax, tc.plainOptions)
			}
			wantNonSpread := make([]bool, len(tc.notFunction))
			for index := range wantNonSpread {
				wantNonSpread[index] = tc.name != "spread" && tc.name != "spreadBeforeOptions"
			}
			if !equal(source0.ArgumentsNonSpreadSyntax, wantNonSpread) {
				t.Fatalf("argumentsNonSpreadSyntax = %v, want %v", source0.ArgumentsNonSpreadSyntax, wantNonSpread)
			}
		})
	}
}
