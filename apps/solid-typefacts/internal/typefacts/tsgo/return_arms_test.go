package tsgo

import (
	"context"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"path/filepath"
	"strings"
	"testing"
)

// ADR 0115: a returned conditional or array literal states the values it can
// evaluate to, exhaustively or not at all. ADR 0116: an arm that calls an
// unwritten whole parameter names it, and a returned call of one is a one-arm
// root.
func TestReturnArmsDecomposeConditionalsAndArrayLiterals(t *testing.T) {
	type element struct {
		parameter int // -1: no identity
		spread    bool
	}
	type arm struct {
		text      string
		parameter int // -1: no identity
		array     bool
		elements  []element
		invoked   int // the invoked parameter's index + 1; 0: none
	}
	cases := []struct {
		name, code string
		want       []arm // nil: no arms stated
	}{
		{
			name: "asArray",
			code: `export const check = (value: any) => Array.isArray(value) ? value : value ? [value] : [];`,
			want: []arm{
				{text: "value", parameter: 0},
				{text: "[value]", parameter: -1, array: true, elements: []element{{parameter: 0}}},
				{text: "[]", parameter: -1, array: true},
			},
		},
		{
			name: "accessWith",
			code: `export function check(valueOrFn: any, ...args: any[]) { return typeof valueOrFn === "function" ? valueOrFn(...args) : valueOrFn; }`,
			want: []arm{
				{text: "valueOrFn(...args)", parameter: -1, invoked: 1},
				{text: "valueOrFn", parameter: 0},
			},
		},
		{
			name: "access",
			code: `export const check = (v: any) => typeof v === "function" && !v.length ? v() : v;`,
			want: []arm{
				{text: "v()", parameter: -1, invoked: 1},
				{text: "v", parameter: 0},
			},
		},
		{
			name: "callRoot",
			code: `export const check = (value: any, fn: any) => (fn as any)(value);`,
			want: []arm{{text: "(fn as any)(value)", parameter: -1, invoked: 2}},
		},
		{
			name: "optionalMemberAndNewCalls",
			code: `export const check = (f: any, o: any, v: any) => v ? f?.() : o ? o.run() : v === 1 ? new f() : v;`,
			want: []arm{
				{text: "f?.()", parameter: -1},
				{text: "o.run()", parameter: -1},
				{text: "new f()", parameter: -1},
				{text: "v", parameter: 2},
			},
		},
		{
			name: "writtenCallee",
			code: `export function check(f: any) { f = f || (() => 1); return f ? f() : f; }`,
			want: []arm{
				{text: "f()", parameter: -1},
				{text: "f", parameter: -1},
			},
		},
		{
			name: "callOfAnythingElse",
			code: `export const check = (value: any) => Array.from(value);`,
		},
		{
			name: "literalCondition",
			code: `export const check = (value: any) => true ? (value as any) : [value];`,
			want: []arm{{text: "value", parameter: 0}},
		},
		{
			name: "spreadAndHole",
			code: `export const check = (value: any, other: any) => [...value, other, , 1];`,
			want: []arm{{text: "[...value, other, , 1]", parameter: -1, array: true, elements: []element{
				{parameter: -1, spread: true}, {parameter: 1}, {parameter: -1}, {parameter: -1},
			}}},
		},
		{
			name: "plainIdentity",
			code: `export const check = (value: any) => value;`,
		},
		{
			name: "async",
			code: `export const check = async (value: any) => value ? value : [];`,
		},
		{
			name: "reassignedInOneArm",
			code: `export function check(value: any) { value = value || []; return value ? value : [value]; }`,
			want: []arm{
				{text: "value", parameter: -1},
				{text: "[value]", parameter: -1, array: true, elements: []element{{parameter: -1}}},
			},
		},
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
			// The export's own name at its declaration: the demand resolves
			// through the runtime binding from an exact identifier.
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
				t.Fatalf("returns = %+v; open %v", flow, answer.Transcripts[0].Implementation.OpenReasons)
			}
			arms := flow.Returns[0].Arms
			if tc.want == nil {
				if arms != nil {
					t.Fatalf("arms = %+v, want none", arms)
				}
				return
			}
			if len(arms) != len(tc.want) {
				t.Fatalf("arms = %+v, want %d", arms, len(tc.want))
			}
			identity := func(source *typefacts.ParameterValueSource) int {
				if source == nil {
					return -1
				}
				return source.ParameterIndex
			}
			for index, want := range tc.want {
				got := arms[index]
				if text := source[got.Location.StartByte:got.Location.EndByte]; text != want.text {
					t.Fatalf("arm %d spells %q, want %q", index, text, want.text)
				}
				if identity(got.Parameter) != want.parameter || got.ArrayLiteral != want.array || got.Value == nil ||
					identity(got.Invoked)+1 != want.invoked {
					t.Fatalf("arm %d = %+v, want %+v", index, got, want)
				}
				if len(got.Elements) != len(want.elements) {
					t.Fatalf("arm %d elements = %+v, want %+v", index, got.Elements, want.elements)
				}
				for position, element := range want.elements {
					if identity(got.Elements[position].Parameter) != element.parameter ||
						got.Elements[position].Spread != element.spread {
						t.Fatalf("arm %d element %d = %+v, want %+v", index, position, got.Elements[position], element)
					}
				}
			}
		})
	}
}
