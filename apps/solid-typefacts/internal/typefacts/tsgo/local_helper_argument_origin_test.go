package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// A resolved helper and a parameter slot identify bindings, not the value the
// caller originally supplied. Keep this distinction explicit before adding a
// proof that composes a helper's member read through an argument transfer.
func TestLocalHelperArgumentBindingDoesNotEstablishOriginalValue(t *testing.T) {
	for _, tc := range []struct{ name, code string }{
		{"beforeAndAfterStore", `export function check(input: string) { read(input); input = "local"; read(input); }`},
		{"siblingDefaultStore", `export function check(input: string, other = (input = "local")) { read(input); }`},
		{"loopStore", `export function check(input: string) { for (let i = 0; i < 2; i++) { read(input); input = "local"; } }`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			source := `function read(key: string) { return key.slice(0); }` + "\n" + tc.code + "\nvoid check;\n"
			dir := t.TempDir()
			writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
			p, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer p.Close()
			start, impl := strings.LastIndex(source, "check"), strings.Index(source, "check(")
			path := filepath.Join(dir, "facts.ts")
			answer, err := p.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
				Location:               typefacts.Location{Path: path, StartByte: start, EndByte: start + 5},
				ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + 5},
			}})
			if err != nil {
				t.Fatal(err)
			}
			implementation := answer.Transcripts[0].Implementation
			if implementation == nil {
				t.Fatal("missing implementation")
			}
			if len(implementation.InitialParameterReads) != 0 {
				t.Fatal("a helper argument must not masquerade as a direct original-member read")
			}
			for _, binding := range implementation.UnwrittenParameters {
				if binding.ParameterIndex == 0 {
					t.Fatal("a written binding must not establish original input identity")
				}
			}
			matched := 0
			for _, call := range implementation.Calls {
				if source[call.Location.StartByte:call.Location.EndByte] != "read(input)" {
					continue
				}
				matched++
				if call.Declaration == nil || call.Declaration.Location.Path != path ||
					len(call.ArgumentParameters) != 1 || call.ArgumentParameters[0] == nil ||
					call.ArgumentParameters[0].ParameterIndex != 0 || len(call.ArgumentParameters[0].Path) != 0 {
					t.Fatalf("expected an exact local callee and argument binding, got %+v", call)
				}
			}
			want := strings.Count(tc.code, "read(input)")
			if matched != want {
				t.Fatalf("resolved helper calls = %d, want %d", matched, want)
			}
		})
	}
}
