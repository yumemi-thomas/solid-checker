package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestInitialParameterReadsBindOnlyTheOpeningPrefix(t *testing.T) {
	for _, tc := range []struct {
		name, code string
		want       int
	}{
		{"beforeWrite", `export function check(input: number[]) { const size = input.length; input = []; return size; }`, 1},
		{"assignmentReceiver", `export function check(input: string) { input = input.trim().replace(/x/g, ""); return input.trim(); }`, 1},
		{"untypedReceiver", `export function check(input) { input = input.trim().replace(/x/g, ""); return input.trim(); }`, 1},
		{"firstIterationGuarded", `export function check(input: string, args?: Record<string, string>) { if (args) for (const [key, value] of Object.entries(args)) input = input.replace(key, value); return input; }`, 1},
		{"firstIterationDirect", `export function check(input: string, args: string[]) { for (const value of args) { input = input.trim(); input = input.toUpperCase(); } return input; }`, 1},
		{"iterationAfterWrite", `export function check(input: string, args: string[]) { input = "local"; for (const value of args) input = input.trim(); return input; }`, 0},
		{"iterationHeaderWrite", `export function check(input: string, args: string[]) { for (const value of (input = "local", args)) input = input.trim(); return input; }`, 0},
		{"iterationBindingDefault", `export function check(input: string, args: string[][]) { for (const [value = (input = "local")] of args) input = input.trim(); return input; }`, 0},
		{"iterationBindingShadow", `export function check(input: string, args: string[][]) { for (const [input] of args) input = input.trim(); return input; }`, 0},
		{"iterationBodyWrite", `export function check(input: string, args: string[]) { for (const value of args) { input = "local"; input = input.trim(); } return input; }`, 0},
		{"iterationClosureWrite", `export function check(input: string, args: string[]) { function change() { input = "local"; } for (const value of args) input = input.trim(); return input; }`, 0},
		{"argumentWriteAfterReceiver", `export function check(input: string) { input = input.concat(input = "local"); return input.trim(); }`, 1},
		{"receiverAfterWrite", `export function check(input: string) { input = "local"; input = input.trim(); return input; }`, 0},
		{"receiverCommaWrite", `export function check(input: string) { input = (input = "local", input).trim(); return input; }`, 0},
		{"receiverAssignment", `export function check(input: string) { input = (input = "local").trim(); return input; }`, 0},
		{"receiverComputed", `export function check(input: string) { input = input["trim"](); return input; }`, 0},
		{"argumentIsNotReceiver", `export function check(input: string) { input = String(input.trim()); return input; }`, 1},
		{"memberAssignment", `export function check(input: any) { input.value = input.trim(); return input; }`, 0},
		{"compoundAssignment", `export function check(input: string) { input += input.trim(); return input; }`, 0},
		{"twoInputs", `export function check(input: number[], prev: number[]) { const size = input.length; const old = prev.length; input = input.slice(1); prev = []; return size + old; }`, 3},
		{"afterWrite", `export function check(input: number[]) { input = []; const size = input.length; return size; }`, 0},
		{"sameStatementWrite", `export function check(input: number[]) { const size = (input = []).length; return size; }`, 0},
		{"callBeforeRead", `export function check(input: number[]) { console.log(); const size = input.length; input = []; return size; }`, 1},
		{"nestedWrite", `export function check(input: number[]) { const size = input.length; function change() { input = []; } return size; }`, 0},
		{"default", `export function check(input: number[] = []) { const size = input.length; return size; }`, 0},
		{"otherDefault", `export function check(input: number[], other = (input = [])) { const size = input.length; return size; }`, 0},
		{"rest", `export function check(...input: number[]) { const size = input.length; return size; }`, 0},
		{"branch", `export function check(input: number[]) { if (input) { const size = input.length; } input = []; }`, 1},
		{"loop", `export function check(input: number[]) { while (input) { const size = input.length; input = []; } }`, 0},
		{"alias", `export function check(input: number[]) { const alias = input; const size = alias.length; input = []; return size; }`, 0},
		{"arguments", `export function check(input: number[]) { const size = input.length; arguments[0] = []; return size; }`, 0},
		{"eval", `export function check(input: number[]) { const size = input.length; eval('input = []'); return size; }`, 0},
		{"async", `export async function check(input: number[]) { const size = input.length; input = []; return size; }`, 0},
		{"generator", `export function* check(input: number[]) { const size = input.length; input = []; yield size; }`, 0},
		{"computed", `export function check(input: number[]) { const size = input['length']; input = []; return size; }`, 0},

		// ADR 0069. The premise is order, not the shape of the statements walked
		// past: with no nested callable, `arguments` or `eval` in the body, the
		// only writer of these bindings is a direct assignment here.
		{"branchExclusiveWrite", `export function check(input: { x: number }, flag: boolean, next: { x: number }[]) { if (flag) { return input.x; } else { for (const item of next) { input = item; } } return 0; }`, 1},
		{"readBeforeLoopWrite", `export function check(input: { x: number }, next: { x: number }[]) { console.log(input.x); for (const item of next) { input = item; } return input; }`, 1},
		{"readAfterBranchWrite", `export function check(input: { x: number }, flag: boolean, other: { x: number }) { if (flag) { input = other; } return input.x; }`, 0},
		{"readAfterLoopWrite", `export function check(input: { x: number }, next: { x: number }[]) { for (const item of next) { input = item; } return input.x; }`, 0},
		{"switchLaterCaseRead", `export function check(input: { x: number }, flag: number, other: { x: number }) { switch (flag) { case 0: input = other; break; case 1: return input.x; } return 0; }`, 0},
		{"destructuringWriteBeforeRead", `export function check(input: { x: number }, other: { input: { x: number } }) { ({ input } = other); return input.x; }`, 0},
		{"destructuringWriteAfterRead", `export function check(input: { x: number }, other: { input: { x: number } }) { console.log(input.x); ({ input } = other); return input; }`, 1},
		{"updateWriteBeforeRead", `export function check(input: any) { input++; return input.x; }`, 0},
		{"varRedeclarationBeforeRead", `export function check(input: string) { var input = "local"; const first = input.slice(0); input = "later"; return first; }`, 0},
		{"otherParameterRedeclaration", `export function check(input: string, other: string) { var other = "local"; const first = input.slice(0); input = "later"; return first; }`, 1},
		// ADR 0080: the strict-undefined branch has local origin; only the
		// complementary, defined-input executions retain caller identity.
		{"undefinedDefault", `export function check(input) { if (input === void 0) { input = []; } return input.concat([]); }`, 1},
		{"undefinedDefaultBranches", `export function check(input, flag) { var local; if (input === void 0) { input = []; } if (flag) { return input.concat([]); } return input.concat([1]); }`, 1},
		{"defaultFallbackOnly", `export function check(input) { const missing = input === void 0; if (input === void 0) { input = []; } if (missing) return input.concat([]); return []; }`, 0},
		{"defaultLaterPredicate", `export function check(input) { if (input === void 0) { input = []; } const current = input; if (current.length === 0) return input.concat([]); return []; }`, 0},
		{"defaultLooseGuard", `export function check(input) { if (input == void 0) { input = []; } return input.concat([]); }`, 0},
		{"defaultTruthyGuard", `export function check(input) { if (!input) { input = []; } return input.concat([]); }`, 0},
		{"defaultNamedUndefined", `export function check(input, undefined) { if (input === undefined) { input = []; } return input.concat([]); }`, 0},
		{"defaultOtherParameter", `export function check(input, other) { if (other === void 0) { input = []; } return input.concat([]); }`, 0},
		{"defaultSideEffectVoid", `export function check(input) { if (input === void (input = [])) { input = []; } return input.concat([]); }`, 0},
		{"defaultNonlocalFallback", `export function check(input, other) { if (input === void 0) { input = other; } return input.concat([]); }`, 0},
		{"defaultPriorWrite", `export function check(input) { input = []; if (input === void 0) { input = []; } return input.concat([]); }`, 0},
		{"defaultLaterWrite", `export function check(input) { if (input === void 0) { input = []; } input = []; return input.concat([]); }`, 0},
		{"defaultVarWrite", `export function check(input) { if (input === void 0) { input = []; } var input = []; return input.concat([]); }`, 0},
		{"defaultDestructuringWrite", `export function check(input, other) { if (input === void 0) { input = []; } ({input} = other); return input.concat([]); }`, 0},
		{"defaultLoopGuard", `export function check(input) { for (;;) { if (input === void 0) { input = []; } return input.concat([]); } }`, 0},
		{"defaultElseWrite", `export function check(input) { if (input === void 0) { input = []; } else { input = []; } return input.concat([]); }`, 0},
		{"defaultCapturedWrite", `export function check(input) { if (input === void 0) { input = []; } function change() { input = []; } return input.concat([]); }`, 0},
		{"defaultArgumentsWrite", `export function check(input) { if (input === void 0) { input = []; } arguments[0] = []; return input.concat([]); }`, 0},
		{"defaultParameterInitializer", `export function check(input = []) { if (input === void 0) { input = []; } return input.concat([]); }`, 0},
		{"defaultShadowedBinding", `export function check(input) { { let input; if (input === void 0) { input = []; } } input = []; return input.concat([]); }`, 0},
	} {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := tc.code + "\nvoid check;\n"
			writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
			p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer p.Close()
			start := strings.LastIndex(source, "check")
			impl := strings.Index(source, "check(")
			path := filepath.Join(dir, "facts.ts")
			answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: typefacts.Location{Path: path, StartByte: start, EndByte: start + 5}, ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + 5}}})
			if err != nil {
				t.Fatal(err)
			}
			implementation := answer.Transcripts[0].Implementation
			if implementation == nil {
				t.Fatal("missing implementation")
			}
			if len(implementation.InitialParameterReads) != tc.want {
				t.Fatalf("reads = %+v, want %d", implementation.InitialParameterReads, tc.want)
			}
			if tc.name == "untypedReceiver" {
				matched := false
				for _, call := range implementation.Calls {
					if call.Location.StartByte == implementation.InitialParameterReads[0].Use.StartByte && call.CalleeParameter != nil && call.CalleeParameter.ParameterIndex == 0 && len(call.CalleeParameter.Path) == 1 && call.CalleeParameter.Path[0].Property == "trim" {
						// Origin is an exact lexical binding fact even when the
						// untyped implementation has no resolved method symbol.
						if call.Target != "" {
							t.Fatal("untyped receiver unexpectedly has a resolved target")
						}
						matched = true
					}
				}
				if !matched {
					t.Fatal("missing exact initial callee parameter")
				}
			}
			for _, read := range implementation.InitialParameterReads {
				if read.FirstIterationOnly != strings.HasPrefix(tc.name, "firstIteration") {
					t.Fatal("first-iteration limitation does not match the source")
				}
				// The prefix and first-receiver rules own every case whose read
				// sits in the opening declarations or on the RHS of the first
				// assignment; every other admitted read here is established by
				// order alone and must say so, because the two rows are not
				// interchangeable at the consumer.
				prefixOwned := map[string]bool{
					"beforeWrite": true, "assignmentReceiver": true, "untypedReceiver": true,
					"firstIterationGuarded": true, "firstIterationDirect": true,
					"argumentWriteAfterReceiver": true, "twoInputs": true,
				}
				isDefault := strings.HasPrefix(tc.name, "undefinedDefault")
				if (read.UndefinedDefault != nil) != isDefault {
					t.Fatal("undefined-default limitation does not match the source")
				}
				if read.UndefinedDefault != nil {
					fallback := read.UndefinedDefault
					if source[fallback.Assignment.StartByte:fallback.Assignment.EndByte] != "input = []" ||
						!strings.HasPrefix(source[fallback.Guard.StartByte:fallback.Guard.EndByte], "if (input === void 0)") ||
						fallback.Guard.EndByte > read.Use.StartByte {
						t.Fatal("default guard/store is not bound before the use")
					}
				}
				if read.Positional == (prefixOwned[tc.name] || isDefault) {
					t.Fatalf("positional marker %v does not match the source", read.Positional)
				}
				parameter := implementation.Signature.Parameters[read.ParameterIndex]
				if parameter.Declaration == nil || parameter.Declaration.Location != read.Declaration {
					t.Fatal("unbound declaration")
				}
				if source[read.Use.StartByte:read.Use.EndByte] != source[read.Declaration.StartByte:read.Declaration.EndByte] {
					t.Fatal("use does not name the original parameter")
				}
			}
		})
	}
}
