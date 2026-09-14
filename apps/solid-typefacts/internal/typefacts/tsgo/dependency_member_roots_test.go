package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// dependencyMemberProject builds a project with a `solid-js` dependency that
// ships declarations only, which is the shape the corpus has: the importing
// package's runtime source is walked, and the imported names resolve to a
// `.d.ts` in node_modules.
func dependencyMemberProject(t *testing.T, source string) (typefacts.ExportValueAnalyzer, string) {
	t.Helper()
	dir := t.TempDir()
	write := func(name, contents string) {
		full := filepath.Join(dir, name)
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(full, []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}`)
	write("node_modules/solid-js/package.json", `{"name":"solid-js","version":"1.9.14","types":"./index.d.ts"}`)
	write("node_modules/solid-js/index.d.ts", `export declare const sharedConfig: { context?: unknown };
export declare const other: { context?: unknown };
declare const bundle: { context?: unknown };
export default bundle;
export declare function createSignal<T>(v: T, o?: unknown): [() => T, (n: unknown) => void];
export declare function onMount(fn: () => void): void;
export declare const isServer: boolean;
`)
	// A declaration file, so the relative import is an alias to something the
	// artifact's own runtime source never built -- ADR 0044 would otherwise
	// root a local object literal and there would be no form to refuse.
	write("local.d.ts", "export declare const table: { context?: unknown };\n")
	write("subject.ts", source)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}
	return analyzer, dir
}

// TestDependencyMemberRootStatesTheImportedPair pins ADR 0104's fact and, more
// importantly, the shapes it refuses. The premise is that the receiver of a
// property read is a name this module imported from a bare specifier and never
// assigned; deciding whether that named export is a data-only object is the
// certifier's reviewed act, so everything the producer can get wrong here is a
// question of *which binding* the read is rooted at.
func TestDependencyMemberRootStatesTheImportedPair(t *testing.T) {
	for _, testCase := range []struct {
		name          string
		source        string
		wantSpecifier string
		wantName      string
		wantRefusal   typefacts.SubjectRootRefusalReason
		why           string
	}{
		{
			name: "namedImport",
			source: `import { sharedConfig } from "solid-js";
export function subject() { return sharedConfig.context ? 1 : 0; }
`,
			wantSpecifier: "solid-js",
			wantName:      "sharedConfig",
			why:           "the shape the corpus has: a named import from a bare specifier, never assigned here",
		},
		{
			name: "renamedImportStatesTheExportedName",
			source: `import { sharedConfig as config } from "solid-js";
export function subject() { return config.context ? 1 : 0; }
`,
			wantSpecifier: "solid-js",
			wantName:      "sharedConfig",
			why:           "a consumer reviews the exporting module's own export, not the local alias this module happened to pick",
		},
		{
			name: "namespaceImportStatesNothing",
			source: `import * as solid from "solid-js";
export function subject() { return solid.sharedConfig.context ? 1 : 0; }
`,
			wantRefusal: typefacts.SubjectRefusalImportedBinding,
			why:         "`solid.sharedConfig` reads the module namespace object, whose properties the specification installs as accessors, and which member it lands on is a second question",
		},
		{
			name: "defaultImportStatesNothing",
			source: `import bundle from "solid-js";
export function subject() { return bundle.context ? 1 : 0; }
`,
			wantRefusal: typefacts.SubjectRefusalImportedBinding,
			why:         "what a default export holds is the exporting module's own expression, and `default` does not name a reviewable member",
		},
		{
			name: "relativeImportStatesNothing",
			source: `import { table } from "./local.js";
export function subject() { return table.context ? 1 : 0; }
`,
			wantRefusal: typefacts.SubjectRefusalImportedBinding,
			why:         "a relative specifier names a file of this same artifact, whose census already walks it; handing a consumer \"./local.js\" invites it to match a table entry against a path",
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			analyzer, dir := dependencyMemberProject(t, testCase.source)
			transcript := implementationTranscriptFor(
				t, analyzer, filepath.Join(dir, "subject.ts"), testCase.source, "subject")
			var accessor *typefacts.UncensusedInvokingForm
			for i, form := range transcript.UncensusedInvokingForms {
				if form.Kind == typefacts.UncensusedPropertyAccessUnknownAccessor {
					accessor = &transcript.UncensusedInvokingForms[i]
					break
				}
			}
			if accessor == nil {
				t.Fatalf("no property-access form stated; forms = %#v", transcript.UncensusedInvokingForms)
			}
			if testCase.wantSpecifier == "" {
				if accessor.SubjectRoot != "" || accessor.SubjectImport != nil {
					t.Fatalf("stated root %q import %#v, want none: %s",
						accessor.SubjectRoot, accessor.SubjectImport, testCase.why)
				}
				if accessor.SubjectRootRefusal != testCase.wantRefusal {
					t.Fatalf("refusal = %q, want %q", accessor.SubjectRootRefusal, testCase.wantRefusal)
				}
				return
			}
			if accessor.SubjectRoot != typefacts.SubjectRootDependencyMember {
				t.Fatalf("root = %q, want %q (refusal %q): %s", accessor.SubjectRoot,
					typefacts.SubjectRootDependencyMember, accessor.SubjectRootRefusal, testCase.why)
			}
			if accessor.SubjectImport == nil ||
				accessor.SubjectImport.Specifier != testCase.wantSpecifier ||
				accessor.SubjectImport.Name != testCase.wantName {
				t.Fatalf("import = %#v, want %s:%s", accessor.SubjectImport,
					testCase.wantSpecifier, testCase.wantName)
			}
			// A stated root and a refusal are mutually exclusive by
			// construction; the invariant test pins it generally, and this
			// keeps the pair honest for the derivation added here.
			if accessor.SubjectRootRefusal != "" {
				t.Fatalf("stated both a root and refusal %q", accessor.SubjectRootRefusal)
			}
			if accessor.SubjectParameter != nil || accessor.SubjectDeclaration != nil {
				t.Fatalf("stated a parameter or declaration beside a dependency-member root: %#v", accessor)
			}
		})
	}
}
