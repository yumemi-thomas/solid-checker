package tsgo

import (
	"context"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"path/filepath"
	"strings"
	"testing"
)

func TestOriginalHelperReads(t *testing.T) {
	cases := []struct {
		name, source, export string
		want                 int
	}{
		{"beforeStore", `function read(key: string) { return key.slice(0); } export function check(input: string) { read(input); input="local"; read(input); }`, "check", 1},
		{"afterStore", `function read(key: string) { return key.slice(0); } export function check(input: string) { input="local"; read(input); }`, "check", 0},
		{"siblingStore", `function read(key: string) { return key.slice(0); } export function check(input: string, other=(input="local")) { read(input); }`, "check", 0},
		{"siblingLiteral", `function read(key: string) { return key.slice(0); } export function check(input: string, other="local") { read(input); input="local"; }`, "check", 1},
		{"loopStore", `function read(key: string) { return key.slice(0); } export function check(input: string) { for(let i=0;i<2;i++){ read(input); input="local"; } }`, "check", 0},
		{"calleeStore", `function read(key: string) { return key.slice(0); } read=()=>"local"; export function check(input: string) { read(input); }`, "check", 0},
		{"capturedStore", `function read(key: string) { return key.slice(0); } export function check(input: string, other=()=>input="local") { read(input); }`, "check", 0},
		{"deadCall", `function read(key: string) { return key.slice(0); } export function check(input: string) { if(false)read(input); }`, "check", 0},
		{"spread", `function read(...key: string[]) { return key.slice(0); } export function check(input: string) { read(...[], input); }`, "check", 0},
		{"varStore", `function read(key: string) { return key.slice(0); } export function check(input: string) { var input="local"; read(input); }`, "check", 0},
		{"varStoreThenAssignment", `function read(key: string) { return key.slice(0); } export function check(input: string) { var input="local"; read(input); input="later"; }`, "check", 0},
		{"moduleEval", `function read(key: string) { return key.slice(0); } eval("read=()=>null"); export function check(input: string) { read(input); }`, "check", 0},
		{"helperStore", `function read(key: string) { key="local"; return key.slice(0); } export function check(input: string) { read(input); }`, "check", 0},
		{"helperRedeclaration", `function read(key: string) { var key="local"; return key.slice(0); } export function check(input: string) { read(input); }`, "check", 0},
		{"wrongMember", `function read(key: string) { return key.trim(); } export function check(input: string) { read(input); }`, "check", 0},
		{"wrongSlot", `function read(local: string,key: string) { return key.slice(0); } export function check(input: string) { read(input,"local"); }`, "check", 0},
		{"asyncHelper", `async function read(key: string) { await 0; return key.slice(0); } export function check(input: string) { read(input); }`, "check", 0},
		{"generatorHelper", `function* read(key: string) { yield key.slice(0); } export function check(input: string) { read(input); }`, "check", 0},
		{"deadHelperRead", `function read(key: string) { if(false) key.slice(0); } export function check(input: string) { read(input); }`, "check", 0},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			source := tc.source + "\nvoid " + tc.export + ";\n"
			filename := "facts.ts"
			writeInvocationProject(t, dir, map[string]string{filename: source})
			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			p := opened.(*project)
			path := filepath.Join(dir, filename)
			start, impl := strings.LastIndex(source, tc.export), strings.Index(source, "function "+tc.export+"(")+len("function ")
			answer, err := p.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{Location: typefacts.Location{Path: path, StartByte: start, EndByte: start + len(tc.export)}, ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + len(tc.export)}}})
			if err != nil {
				t.Fatal(err)
			}
			if answer.Transcripts[0].Implementation == nil {
				t.Fatal("no implementation")
			}
			var found []typefacts.OriginalHelperRead
			for _, read := range answer.Transcripts[0].Implementation.OriginalHelperReads {
				if read.Property == "slice" {
					found = append(found, read)
				}
			}
			if len(found) != tc.want {
				for _, call := range answer.Transcripts[0].Implementation.Calls {
					t.Logf("callee declaration: %+v", call.Declaration)
				}
				t.Fatalf("candidate arguments=%+v, want %d; calls=%+v; uses=%+v", found, tc.want, answer.Transcripts[0].Implementation.Calls, answer.Transcripts[0].Implementation.ParameterUses)
			}
		})
	}
}
