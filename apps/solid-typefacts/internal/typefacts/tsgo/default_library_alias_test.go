package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestDefaultLibraryAliasStatesIdentityForBuiltInReExports pins ADR 0103's
// fact. `@solid-primitives/utils` exports `const keys = Object.keys` and
// `@floating-ui/utils` exports `const floor = Math.floor`; both are the
// built-in by identity, and both leave the transcript open for different
// reasons -- `keys` with `callSignatureNotUnique` (the member is overloaded,
// so no signature is selectable) and `floor` with `implementationUnavailable`
// (its only declaration is a body-less signature). The fact is stated on both,
// beside the open reason rather than instead of it.
//
// The refusing rows are the point of the test: identity has to be proven, not
// guessed. A computed member, a shadowed container, a written binding, a
// written container member, and an alias of something that is not a
// default-library member all state nothing.
func TestDefaultLibraryAliasStatesIdentityForBuiltInReExports(t *testing.T) {
	cases := []struct {
		name          string
		source        string
		wantContainer string
		wantMember    string
		wantReason    string
	}{
		{
			name:          "overloadedObjectMember",
			source:        "export const subject = Object.keys;\n",
			wantContainer: "Object",
			wantMember:    "keys",
			wantReason:    "callSignatureNotUnique",
		},
		{
			name:          "bodylessMathMember",
			source:        "export const subject = Math.floor;\n",
			wantContainer: "Math",
			wantMember:    "floor",
			wantReason:    "implementationUnavailable",
		},
		{
			name:          "parenthesizedStillExact",
			source:        "export const subject = (Object.entries);\n",
			wantContainer: "Object",
			wantMember:    "entries",
			wantReason:    "callSignatureNotUnique",
		},
		{
			// A computed member does not fix which member is read.
			name:   "computedMemberStatesNothing",
			source: "const which = \"keys\";\nexport const subject = Object[which];\n",
		},
		{
			// The container is a local object, not the global.
			name:   "shadowedContainerStatesNothing",
			source: "const Object = { keys: (value: unknown) => [] as string[] };\nexport const subject = Object.keys;\n",
		},
		{
			// The alias itself is rewritten later in the file.
			name:   "writtenBindingStatesNothing",
			source: "export let subject = Object.keys;\nsubject = (() => []) as typeof Object.keys;\n",
		},
		{
			// The container's member is rewritten before the alias is read.
			name:   "writtenContainerMemberStatesNothing",
			source: "(Object as any).keys = () => [];\nexport const subject = Object.keys;\n",
		},
		{
			// Not a default-library member at all.
			name:   "localNamespaceStatesNothing",
			source: "const Helpers = { pick: (value: unknown) => value };\nexport const subject = Helpers.pick;\n",
		},
		{
			// An identifier alias is ADR 0059's shape, not this one.
			name:   "identifierAliasStatesNothing",
			source: "function local(value: unknown) { return value; }\nexport const subject = local;\n",
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
			write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"files":["harness.ts","subject.ts"]}`)
			write("subject.ts", tc.source)
			harness := "import { subject } from \"./subject.js\";\nvoid subject;\n"
			write("harness.ts", harness)

			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			analyzer := opened.(typefacts.ExportValueAnalyzer)
			queryStart := strings.LastIndex(harness, "void subject") + len("void ")
			implStart := strings.Index(tc.source, " subject") + 1
			location := typefacts.Location{
				Path: filepath.Join(dir, "harness.ts"), StartByte: queryStart, EndByte: queryStart + len("subject"),
			}
			implementation := typefacts.Location{
				Path: filepath.Join(dir, "subject.ts"), StartByte: implStart, EndByte: implStart + len("subject"),
			}
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

			if tc.wantContainer == "" {
				if got.DefaultLibraryAlias != nil {
					t.Fatalf("stated %#v, want no default-library alias", got.DefaultLibraryAlias)
				}
				return
			}
			if got.DefaultLibraryAlias == nil {
				t.Fatalf("no default-library alias stated; open reasons = %v", got.OpenReasons)
			}
			if got.DefaultLibraryAlias.Container != tc.wantContainer ||
				got.DefaultLibraryAlias.Member != tc.wantMember {
				t.Fatalf("alias = %#v, want %s.%s", got.DefaultLibraryAlias, tc.wantContainer, tc.wantMember)
			}
			// The fact is stated *beside* the refusal, never instead of it: a
			// consumer with no reviewed entry for the member has to keep
			// refusing, and it can only do that if the reason is still there.
			if got.Complete {
				t.Fatalf("transcript went complete; the alias fact must not close it")
			}
			if len(got.OpenReasons) != 1 || got.OpenReasons[0] != tc.wantReason {
				t.Fatalf("open reasons = %v, want exactly [%s]", got.OpenReasons, tc.wantReason)
			}
			if got.Declaration == nil || got.Declaration.Name != "subject" {
				t.Fatalf("declaration = %#v, want the exported binding for identity", got.Declaration)
			}
		})
	}
}

// TestDefaultLibraryAliasStatesIdentityThroughAPublishedDeclaration is the
// shape the corpus actually has, and the one the table-driven test above does
// not reach: a published package ships `dist/index.js` with the alias and a
// sibling `dist/index.d.ts` that declares the export with its *own* signature
// rather than `typeof Object.keys`. Module resolution lands on the `.d.ts`,
// whose const has no initializer, so a detection that read whichever
// declaration the checker happened to pick would state nothing -- while the
// runtime value is still the built-in.
//
// It was written to test that as the explanation for ADR 0103 closing zero
// corpus rows, and it *refutes* it: the fact is stated on this shape, with a
// declaration and exactly one acceptable open reason. Whatever keeps those
// rows withheld is downstream of the producer. The test stays as the pin that
// the published shape -- not just the hand-written one -- states identity.
func TestDefaultLibraryAliasStatesIdentityThroughAPublishedDeclaration(t *testing.T) {
	dir := t.TempDir()
	write := func(name, source string) {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false},"files":["harness.ts","index.js","index.d.ts"]}`)
	const runtime = "export const keys = Object.keys;\n"
	// The published declaration, spelled the way @solid-primitives/utils@6.4.1
	// spells it: a generic signature of its own, not `typeof Object.keys`.
	write("index.d.ts", "export declare const keys: <T extends object>(object: T) => (keyof T)[];\n")
	write("index.js", runtime)
	harness := "import { keys } from \"./index.js\";\nvoid keys;\n"
	write("harness.ts", harness)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	queryStart := strings.LastIndex(harness, "void keys") + len("void ")
	implStart := strings.Index(runtime, " keys") + 1
	location := typefacts.Location{
		Path: filepath.Join(dir, "harness.ts"), StartByte: queryStart, EndByte: queryStart + len("keys"),
	}
	implementation := typefacts.Location{
		Path: filepath.Join(dir, "index.js"), StartByte: implStart, EndByte: implStart + len("keys"),
	}
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
	if got.DefaultLibraryAlias == nil {
		t.Fatalf(
			"no default-library alias stated for a published alias; open reasons = %v, declaration = %#v",
			got.OpenReasons, got.Declaration,
		)
	}
	if got.DefaultLibraryAlias.Container != "Object" || got.DefaultLibraryAlias.Member != "keys" {
		t.Fatalf("alias = %#v, want Object.keys", got.DefaultLibraryAlias)
	}
	// The consumer reads the fact only from a transcript open with exactly one
	// reason drawn from the two this premise answers. A published package that
	// opened with a third reason, or with two, would state the fact and still
	// be unreadable -- which is the shape that closed zero corpus rows.
	if got.Complete {
		t.Fatalf("transcript went complete; the alias fact must not close it")
	}
	if len(got.OpenReasons) != 1 ||
		(got.OpenReasons[0] != "callSignatureNotUnique" && got.OpenReasons[0] != "implementationUnavailable") {
		t.Fatalf(
			"open reasons = %v, want exactly one of callSignatureNotUnique/implementationUnavailable",
			got.OpenReasons,
		)
	}
	if got.Declaration == nil {
		t.Fatalf("no declaration stated; the consumer requires one for identity")
	}
}
