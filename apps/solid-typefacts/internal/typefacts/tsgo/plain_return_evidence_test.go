package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// The 2026-09-28 amendment to ADR 0113, handshake protocol 67: a return site
// states the evidence a `plain` return rests on beside its type -- grammar
// (which now reaches a never-written `const` and the intrinsic `undefined`),
// a default-library member called by identity, or TypeScript source -- and
// states none for a JavaScript binding whose type is only its declaration's.
func TestReturnSitesStatePlainReturnEvidence(t *testing.T) {
	cases := []struct {
		name, file, code string
		syntax           bool
		call             string
		typeScript       bool
		typedNumber      bool
	}{
		// The hole: typed `number` by its declaration, holding a function.
		{"letReassignedToFunction", "facts.js", `export function check() { let x = 0; x = () => 1; return x; }`, false, "", false, true},
		{"constLiteral", "facts.js", "const x = 5;\nexport function check() { return x; }", true, "", false, true},
		{"constOfConst", "facts.js", "const a = 1;\nconst b = a;\nexport function check() { return b; }", true, "", false, true},
		// A `let` every value of which is a primitive by grammar (the
		// 2026-09-28 amendment to ADR 0149), and one that is not.
		{"constOfLet", "facts.js", "let a = 1;\nconst b = a;\nexport function check() { return b; }", true, "", false, true},
		{"constOfObjectWrittenLet", "facts.js", "let a = 1;\na = { valueOf: () => 1 };\nconst b = a;\nexport function check() { return b; }", false, "", false, true},
		{"constObject", "facts.js", "const o = { n: 1 };\nexport function check() { return o; }", false, "", false, false},
		{"undefined", "facts.js", `export function check() { return undefined; }`, true, "", false, false},
		{"shadowedUndefined", "facts.js", `export function check() { const undefined = {}; return undefined; }`, false, "", false, false},
		{"mathMin", "facts.js", `export function check(a, b) { return Math.min(a, b); }`, false, "Math.min", false, true},
		{"mathShadowed", "facts.js", "const Math = { min: () => ({}) };\nexport function check(a) { return Math.min(a); }", false, "", false, false},
		{"mathWritten", "facts.js", `export function check(a) { Math.min = () => 1; return Math.min(a); }`, false, "", false, true},
		{"mathAlias", "facts.js", "let M = Math;\nexport function check(a) { return M.min(a); }", false, "", false, true},
		{"optionalCall", "facts.js", `export function check(a) { return Math.min?.(a); }`, false, "", false, false},
		{"typeScriptLet", "facts.ts", `export function check(n: number): number { let m = n; m = n; return m; }`, false, "", true, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := tc.code + "\nvoid check;\n"
			if err := os.WriteFile(
				filepath.Join(dir, "tsconfig.json"),
				[]byte(`{"compilerOptions":{"strict":true,"allowJs":true,"checkJs":false,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts","*.js"]}`),
				0o644,
			); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(dir, tc.file), []byte(source), 0o644); err != nil {
				t.Fatal(err)
			}
			p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer p.Close()
			path := filepath.Join(dir, tc.file)
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
			site := flow.Returns[0]
			if site.PrimitiveSyntax != tc.syntax {
				t.Fatalf("primitiveSyntax = %v, want %v", site.PrimitiveSyntax, tc.syntax)
			}
			if site.DefaultLibraryCall != tc.call {
				t.Fatalf("defaultLibraryCall = %q, want %q", site.DefaultLibraryCall, tc.call)
			}
			if site.TypeScriptSource != tc.typeScript {
				t.Fatalf("typeScriptSource = %v, want %v", site.TypeScriptSource, tc.typeScript)
			}
			if tc.typedNumber {
				value := site.Value
				if value == nil || !value.Primitive.MayBeNumber || value.Primitive.MayBeObject ||
					value.Callability != typefacts.CallabilityNonCallable {
					t.Fatalf("value = %+v, want a number alone", value)
				}
			}
		})
	}
}
