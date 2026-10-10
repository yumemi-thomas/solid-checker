package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// The 2026-09-28 amendment to ADR 0149: a coercion is left unrecorded only
// when every operand is *proved* a primitive -- by grammar, by a built-in's
// declared primitive result called by identity, as an unwritten parameter of
// the implementation, or in TypeScript source -- never by a JavaScript type
// alone, which is a binding's declaration: `let v = 0; … v = { valueOf: run
// }; v - 1` types `v` as `number` and runs `run`.
func TestACoercionOperandMustBeProvedPrimitive(t *testing.T) {
	cases := []struct {
		name, file, code string
		coercion         bool
	}{
		// The hole.
		{"reassignedLet", "facts.js", "export function check(run) { let v = 0; v = { valueOf: run }; return v - 1; }", true},
		// A member read is typed by its object's declaration: the accepted loss.
		{"memberRead", "facts.js", "const table = { n: 1 };\nexport function check() { return table.n - 1; }", true},
		// Grammar.
		{"literals", "facts.js", "export function check() { return 1 + 2; }", false},
		{"constLiteral", "facts.js", "const c = 1;\nexport function check() { return c - 1; }", false},
		{"primitiveLet", "facts.js", "export function check() { let r = 0; r += 2; return r - 1; }", false},
		{"template", "facts.js", "export function check() { return `${1 + 1}px`; }", false},
		// A built-in's primitive result, by identity.
		{"dateNow", "facts.js", "export function check() { return Date.now() - 1; }", false},
		{"mathMin", "facts.js", "export function check() { return Math.min(1, 2) - 1; }", false},
		// A const holding one.
		{"constOfBuiltinResult", "facts.js", "export function check() { const r = Math.random().toString(36); return `${r}-x`; }", false},
		// ...and a shadowed one.
		{"shadowedMath", "facts.js", "const Math = { min: () => ({}) };\nexport function check() { return Math.min(1, 2) - 1; }", true},
		// An unwritten parameter typed `any` in JavaScript still coerces.
		{"untypedParameter", "facts.js", "export function check(a) { return a - 1; }", true},
		// On the original program a parameter's type is its default's or a
		// JSDoc's, neither of which any caller is held to: the coercion is
		// recorded, and only the declared-signature premise (pinned by
		// TestDeclaredSignaturePremiseClearsACoercionOverADeclaredNumber) can
		// clear it.
		{"jsdocTypedParameter", "facts.js", "/** @param {number} a */\nexport function check(a) { return a - 1; }", true},
		{"defaultTypedParameter", "facts.js", "export function check(a = 1) { a += 1; return () => !--a; }", true},
		// TypeScript source holds every write to the declared type.
		{"typeScriptLet", "facts.ts", "export function check(n: number) { let m = n; m = n; return m - 1; }", false},
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
			recorded := false
			for _, form := range answer.Transcripts[0].Implementation.UncensusedInvokingForms {
				if form.Kind == typefacts.UncensusedCoercion {
					recorded = true
				}
			}
			if recorded != tc.coercion {
				t.Fatalf("coercion recorded = %v, want %v: %+v", recorded, tc.coercion,
					answer.Transcripts[0].Implementation.UncensusedInvokingForms)
			}
		})
	}
}
