package tsgo

import (
	"context"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"path/filepath"
	"strings"
	"testing"
)

func TestUnwrittenParameterBindingsAreAffirmativeSourceFacts(t *testing.T) {
	cases := []struct {
		name, code string
		want       int
	}{
		{"generic", `export function check<T>(value: T) { return value; }`, 1},
		{"property", `export function check(value: any) { void value.size; }`, 1},
		{"reassigned", `export function check(value: any) { value = {}; void value.size; }`, 0},
		{"redeclarationInitializer", `export function check(value: string) { var value = "local"; return value.slice(0); }`, 0},
		{"redeclarationReturn", `export function check(value: string) { var value = "local"; return value; }`, 0},
		{"otherRedeclaration", `export function check(value: string, other: string) { var other = "local"; return value; }`, 1},
		{"nestedWrite", `export function check(value: any) { function mutate() { value = {}; } void value.size; }`, 0},
		{"default", `export function check(value: any = {}) { void value.size; }`, 0},
		{"rest", `export function check(...value: any[]) { void value.length; }`, 0},
		{"destructure", `export function check({ value }: any) { void value.size; }`, 0},
		{"arguments", `export function check(value: any) { arguments[0] = {}; void value.size; }`, 0},
		{"eval", `export function check(value: any) { eval('value = {}'); void value.size; }`, 0},
		{"async", `export async function check(value: any) { await Promise.resolve(); void value.size; return value; }`, 1},
		{"asyncWrite", `export async function check(value: any) { await Promise.resolve(); value = {}; void value.size; }`, 0},
		{"asyncCapturedWrite", `export async function check(value: any) { const change = () => { value = {}; }; await Promise.resolve().then(change); void value.size; }`, 0},
		{"asyncDefault", `export async function check(value: any = {}) { await Promise.resolve(); void value.size; }`, 0},
		{"asyncArguments", `export async function check(value: any) { await Promise.resolve(); arguments[0] = {}; void value.size; }`, 0},
		{"asyncEval", `export async function check(value: any) { await Promise.resolve(); eval('value = {}'); void value.size; }`, 0},
		{"generator", `export function* check(value: any) { void value.size; }`, 0},
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
			start := strings.LastIndex(source, "check")
			impl := strings.Index(source, "check(")
			if impl < 0 {
				impl = strings.Index(source, "check<")
			}
			answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: start, EndByte: start + 5}, ImplementationLocation: &typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: impl, EndByte: impl + 5}}})
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatal("missing implementation census")
			}
			implementation := answer.Transcripts[0].Implementation
			if tc.name == "async" {
				if implementation.ControlFlow == nil || len(implementation.ControlFlow.Returns) != 1 || implementation.ControlFlow.Returns[0].Parameter != nil {
					t.Fatal("async binding identity must not establish unwrapped return identity")
				}
			}
			if len(implementation.UnwrittenParameters) != tc.want {
				t.Fatalf("bindings = %+v, want %d", implementation.UnwrittenParameters, tc.want)
			}
			if tc.name == "redeclarationReturn" || tc.name == "otherRedeclaration" {
				if implementation.ControlFlow == nil || len(implementation.ControlFlow.Returns) != 1 {
					t.Fatal("missing exact return census")
				}
				origin := implementation.ControlFlow.Returns[0].Parameter
				if tc.name == "redeclarationReturn" && origin != nil {
					t.Fatal("redeclaration return must not carry original parameter identity")
				}
				if tc.name == "otherRedeclaration" && (origin == nil || origin.ParameterIndex != 0) {
					t.Fatal("unrelated redeclaration must preserve original return identity")
				}
			}
			for _, binding := range implementation.UnwrittenParameters {
				parameter := implementation.Signature.Parameters[binding.ParameterIndex]
				if parameter.Declaration == nil || parameter.Declaration.Location != binding.Declaration {
					t.Fatal("binding differs from exact signature declaration")
				}
				if source[binding.Declaration.StartByte:binding.Declaration.EndByte] != "value" {
					t.Fatal("binding does not name parameter source")
				}
			}
		})
	}
}

func TestUnwrittenParameterBindingsDoNotInvalidateForeignImplementation(t *testing.T) {
	dir := t.TempDir()
	source := "import { check } from './helper'; void check;"
	writeInvocationProject(t, dir, map[string]string{
		"facts.ts":  source,
		"helper.ts": "export function check(value: any) { return value.size; }",
	})
	p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer p.Close()
	start := strings.LastIndex(source, "check")
	location := typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: start, EndByte: start + 5}
	answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &location}})
	if err != nil {
		t.Fatal(err)
	}
	implementation := answer.Transcripts[0].Implementation
	if implementation == nil || implementation.Signature == nil {
		t.Fatal("missing existing implementation signature")
	}
	if implementation.Signature.Parameters[0].Declaration.Location.Path == location.Path {
		t.Fatal("test must reach a foreign implementation")
	}
	if len(implementation.UnwrittenParameters) != 0 {
		t.Fatal("foreign source cannot supply the same-file additional premise")
	}
}
