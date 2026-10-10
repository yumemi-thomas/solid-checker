package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0152, handshake protocol 71: each call of an export's implementation
// census states the export parameter its callee is by binding identity, at any
// depth of nesting, and whether it runs exactly once on every normal
// completion of its flow owner.
func TestCallsStateTheirUnwrittenParameterAndWhetherTheyRunOnEveryCompletion(t *testing.T) {
	type want struct {
		parameter     int // -1: no calleeUnwrittenParameter
		unconditional bool
	}
	cases := []struct {
		name, code string
		calls      map[string]want
	}{
		{
			"a returned pipe",
			`export function check(a: (x: string) => number, b: (y: number) => number) { return (raw: string) => b(a(raw)); }`,
			map[string]want{"b(a(raw))": {1, true}, "a(raw)": {0, true}},
		},
		{
			"a statement body, a throw before it",
			`export function check(source: () => void) { return () => { if (!source) throw new Error("x"); source(); return 1; }; }`,
			map[string]want{"source()": {0, true}},
		},
		{
			"a guarded call",
			`export function check(cb?: () => void) { return () => { cb && cb(); }; }`,
			map[string]want{"cb()": {0, false}},
		},
		{
			"an if arm",
			`export function check(cb: () => void, flag: boolean) { return () => { if (flag) cb(); }; }`,
			map[string]want{"cb()": {0, false}},
		},
		{
			"an optional call",
			`export function check(cb?: () => void) { return () => { cb?.(); }; }`,
			map[string]want{"cb?.()": {0, false}},
		},
		{
			"a loop",
			`export function check(cb: () => void) { return () => { for (let i = 0; i < 2; i++) cb(); }; }`,
			map[string]want{"cb()": {0, false}},
		},
		{
			"a try block",
			`export function check(cb: () => number) { return () => { try { return cb(); } catch { return 0; } }; }`,
			map[string]want{"cb()": {0, false}},
		},
		{
			"an early return before it",
			`export function check(cb: () => void, flag: boolean) { return () => { if (flag) return; cb(); }; }`,
			map[string]want{"cb()": {0, false}},
		},
		{
			"a call nested one callable deeper",
			`export function check(cb: () => void) { return () => { queueMicrotask(() => cb()); }; }`,
			map[string]want{"cb()": {0, true}, "queueMicrotask(() => cb())": {-1, true}},
		},
		{
			"a defaulted parameter",
			`const fallback = () => 0; export function check(cb: () => number = fallback) { return () => cb(); }`,
			map[string]want{"cb()": {-1, true}},
		},
		{
			"a written parameter",
			`export function check(cb: () => void) { cb = () => {}; return () => cb(); }`,
			map[string]want{"cb()": {-1, true}},
		},
		{
			"the implementation's own call",
			`export function check(cb: () => void) { cb(); }`,
			map[string]want{"cb()": {0, true}},
		},
		{
			"an async flow owner",
			`export function check(cb: () => void) { return async () => { cb(); }; }`,
			map[string]want{"cb()": {0, false}},
		},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := testCase.code + "\nvoid check;\n"
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
			seen := 0
			for _, call := range implementation.Calls {
				text := source[call.Location.StartByte:call.Location.EndByte]
				expected, ok := testCase.calls[text]
				if !ok {
					continue
				}
				seen++
				got := -1
				if call.CalleeUnwrittenParameter != nil {
					got = *call.CalleeUnwrittenParameter
				}
				if got != expected.parameter {
					t.Errorf("%s: calleeUnwrittenParameter = %d, want %d", text, got, expected.parameter)
				}
				if call.Unconditional != expected.unconditional {
					t.Errorf("%s: unconditional = %v, want %v", text, call.Unconditional, expected.unconditional)
				}
			}
			if seen != len(testCase.calls) {
				t.Fatalf("saw %d of the %d calls", seen, len(testCase.calls))
			}
		})
	}
}
