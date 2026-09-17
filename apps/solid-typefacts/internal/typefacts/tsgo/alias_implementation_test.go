package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestExportImplementationFollowsAnExactAliasToTheRuntimeBody pins handshake
// protocol 20's fact. `@tanstack/query-core` exports `const defaultScheduler =
// systemSetTimeoutZero`, importing that name from a sibling module of a package
// that ships declarations beside its runtime; module resolution takes the import
// to `timeoutManager.d.ts`, which has no body, and a protocol-19 producer refused
// the binding as `implementationUnavailable`. The producer now follows the alias
// to the runtime module the specifier denotes and that module's export of the
// imported name: Declaration stays the demanded binding (its name is the export
// the verifier checks), and ImplementationOf names whose body was walked.
//
// Every inexact shape stays refused: a reassigned binding, a reassigned target,
// a non-identifier initializer, a body-less target, and a specifier the program
// cannot follow to a runtime file.
func TestExportImplementationFollowsAnExactAliasToTheRuntimeBody(t *testing.T) {
	const timeoutJS = "export function systemSetTimeoutZero(callback) {\n  return setTimeout(callback, 0);\n}\n"
	const timeoutDTS = "export declare function systemSetTimeoutZero(callback: () => void): number;\n"
	const notifyDTS = "export declare const defaultScheduler: (callback: () => void) => void;\n"
	cases := []struct {
		name     string
		notify   string
		timeout  string
		wantBody bool
	}{
		{
			"importedAlias",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nconst defaultScheduler = systemSetTimeoutZero;\nexport { defaultScheduler };\n",
			timeoutJS,
			true,
		},
		{
			"importedAliasThroughWrapper",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nconst defaultScheduler = (systemSetTimeoutZero);\nexport { defaultScheduler };\n",
			timeoutJS,
			true,
		},
		{
			"localAlias",
			"function local(callback) { return setTimeout(callback, 0); }\nconst defaultScheduler = local;\nexport { defaultScheduler };\n",
			timeoutJS,
			true,
		},
		{
			"reassignedBinding",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nlet defaultScheduler = systemSetTimeoutZero;\ndefaultScheduler = systemSetTimeoutZero;\nexport { defaultScheduler };\n",
			timeoutJS,
			false,
		},
		{
			"reassignedTarget",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nconst defaultScheduler = systemSetTimeoutZero;\nexport { defaultScheduler };\n",
			timeoutJS + "systemSetTimeoutZero = function (callback) { return 0; };\n",
			false,
		},
		{
			"callInitializer",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nconst defaultScheduler = systemSetTimeoutZero.bind(null);\nexport { defaultScheduler };\n",
			timeoutJS,
			false,
		},
		{
			"bodylessTarget",
			"import { systemSetTimeoutZero } from \"./timeout.js\";\nconst defaultScheduler = systemSetTimeoutZero;\nexport { defaultScheduler };\n",
			"export const systemSetTimeoutZero = globalThis.setTimeout;\n",
			false,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			write := func(name, source string) {
				if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false,"maxNodeModuleJsDepth":100},"files":["harness.ts","notify.js","timeout.js","notify.d.ts","timeout.d.ts"]}`)
			write("timeout.js", tc.timeout)
			write("timeout.d.ts", timeoutDTS)
			write("notify.js", tc.notify)
			write("notify.d.ts", notifyDTS)
			harness := "import { defaultScheduler as subject } from \"./notify.js\";\nvoid subject;\n"
			write("harness.ts", harness)
			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			analyzer := opened.(typefacts.ExportValueAnalyzer)
			queryStart := strings.LastIndex(harness, "void subject") + len("void ")
			implStart := strings.Index(tc.notify, " defaultScheduler") + 1
			location := typefacts.Location{Path: filepath.Join(dir, "harness.ts"), StartByte: queryStart, EndByte: queryStart + len("subject")}
			implementation := typefacts.Location{Path: filepath.Join(dir, "notify.js"), StartByte: implStart, EndByte: implStart + len("defaultScheduler")}
			answer, err := analyzer.ExportValueTranscripts(
				context.Background(),
				[]typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &implementation}},
			)
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatalf("transcripts = %#v, want one with an implementation", answer.Transcripts)
			}
			got := answer.Transcripts[0].Implementation
			if !tc.wantBody {
				if got.Complete || got.ImplementationOf != nil {
					t.Fatalf("expected an open transcript without an alias hop, got complete=%v implementationOf=%#v", got.Complete, got.ImplementationOf)
				}
				if len(got.OpenReasons) != 1 || got.OpenReasons[0] != "implementationUnavailable" {
					t.Fatalf("open reasons = %v, want implementationUnavailable", got.OpenReasons)
				}
				return
			}
			if !got.Complete {
				t.Fatalf("transcript stayed open: %v", got.OpenReasons)
			}
			if got.Declaration == nil || got.Declaration.Name != "defaultScheduler" ||
				!strings.HasSuffix(got.Declaration.Location.Path, "notify.js") {
				t.Fatalf("declaration = %#v, want the demanded binding in notify.js", got.Declaration)
			}
			if got.ImplementationOf == nil {
				t.Fatalf("no implementationOf on an aliased body")
			}
			wantName, wantFile := "systemSetTimeoutZero", "timeout.js"
			if tc.name == "localAlias" {
				wantName, wantFile = "local", "notify.js"
			}
			if got.ImplementationOf.Name != wantName || !strings.HasSuffix(got.ImplementationOf.Location.Path, wantFile) {
				t.Fatalf("implementationOf = %#v, want %s in %s", got.ImplementationOf, wantName, wantFile)
			}
			if len(got.ControlFlow.Returns) != 1 {
				t.Fatalf("census did not walk the aliased body: returns = %#v", got.ControlFlow.Returns)
			}
		})
	}
}
