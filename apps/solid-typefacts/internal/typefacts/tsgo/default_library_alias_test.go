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
// declaration and exactly one acceptable open reason. The test stays as the pin
// that the published shape -- not just the hand-written one -- states identity.
//
// Its original conclusion -- "whatever keeps those rows withheld is downstream
// of the producer" -- was measured on 2026-09-18 and is wrong for the package
// that carries the demand. `@solid-primitives/utils@7.0.0-next.4` states no
// alias at all, because one line of the same bundle hands `Object` to a callee
// as an argument. `TestDefaultLibraryAliasIsSuppressedByAReceiverEscapeInTheSameFile`
// below pins that, and `@floating-ui/utils`' `Math` aliases -- same producer,
// no escape -- do state the fact, so the detection is not what differs.
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

// TestDefaultLibraryAliasIsSuppressedByAReceiverEscapeInTheSameFile pins why
// ADR 0103 closes zero rows of the ecosystem corpus, which the test above
// recorded as an open question and attributed to something "downstream of the
// producer". For `@solid-primitives/utils@7.0.0-next.4` that attribution is
// wrong, and the cause is one line of the published bundle.
//
// `dist/index.js` aliases four reviewed members -- `Object.entries`,
// `Object.keys`, `Object.values` and `Object.is` -- and also contains
//
//	const defaultEquals = Object.is.bind(Object);
//
// The second `Object` there is a bare argument. `immutableAliasLibrarySourceIsStable`
// requires every occurrence of the *receiver* to be the expression of a
// property access, because a receiver handed to an arbitrary callee may be
// mutated by it before the alias is taken -- `Object.defineProperty(arg, …)`
// is the shape it exists to refuse. The check is deliberately whole-file, so
// that one argument withdraws the fact from every export of the bundle,
// `entries` and `keys` included. They are 37 and 22 consumer call sites in the
// pinned demand census, and their `reads` domain stays open for want of it.
//
// The refusal was not obviously wrong, and it was narrowed rather than removed.
// ADR 0112 reports `Container.member.bind(Container)` as a named escape instead
// of refusing it, because `Function.prototype.bind` cannot rewrite its argument
// and the *bound target* is a question the consumer's reviewed table already
// answers. What this test keeps pinning is the rest of the guard: an escape
// that is not that exact shape still withdraws the fact from the whole file.
//
// The paired negative is `TestDefaultLibraryAliasStatesIdentityThroughAPublishedDeclaration`
// above: same producer, same reviewed members, no escape, fact stated. So the
// detection works and the guard is what differs.
func TestDefaultLibraryAliasNamesAReviewableContainerEscape(t *testing.T) {
	const alias = "const entries = Object.entries;\n"
	cases := []struct {
		name string
		// The line placed *before* the alias, so an escape it contains
		// precedes the alias at module evaluation.
		extra string
		// nil means the fact must not be stated at all.
		wantEscapes []string
	}{
		// ADR 0112's subject: `@solid-primitives/utils@7.0.0-next.4` line 18.
		// The alias is stated, and the escape is named for review.
		{name: "bindTheContainerToItsOwnMember", extra: "const eq = Object.is.bind(Object);\n", wantEscapes: []string{"Object.is"}},
		// Two escapes of different members are both named, once each.
		{
			name:        "twoMembersAndNoDuplicates",
			extra:       "const eq = Object.is.bind(Object);\nconst k = Object.keys.bind(Object);\nconst k2 = Object.keys.bind(Object);\n",
			wantEscapes: []string{"Object.is", "Object.keys"},
		},
		// No escape at all still states no list, which is protocol 59's shape.
		{name: "noEscape", extra: "", wantEscapes: []string{}},

		// The negatives. Each is a way the shape could fail to be the reviewed
		// one, and each must withdraw the fact exactly as protocol 59 did.
		//
		// The container in a later argument slot is a value the bound target
		// receives rather than its `this`, which is a different claim.
		{name: "containerInASecondArgument", extra: "const eq = Object.is.bind(undefined, Object);\n"},
		// A plain call, not a `bind`: `Object.defineProperty(Object, …)` is
		// exactly what the guard exists to refuse.
		{name: "plainMutatingCall", extra: "Object.defineProperty(Object, \"x\", { value: 1 });\n"},
		// `bind` on something that is not a member of this container: the
		// bound target could be anything.
		{name: "bindOnAnUnrelatedFunction", extra: "function local() {}\nconst eq = local.bind(Object);\n"},
		// A `bind` this file installed is not `Function.prototype.bind`.
		{name: "shadowedBind", extra: "const wrapper = { bind: (x) => x };\nconst eq = wrapper.bind(Object);\n"},
		// The container bound to a member of a *different* default-library
		// container: still an escape this walk will not describe.
		{name: "bindAcrossContainers", extra: "const eq = Math.max.bind(Object);\n"},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			got := defaultLibraryAliasForTest(t, testCase.extra+alias+"export { entries };\n")
			if testCase.wantEscapes == nil {
				if got != nil {
					t.Fatalf("alias = %#v, want none: this escape shape is not the reviewed one", got)
				}
				return
			}
			if got == nil {
				t.Fatalf("no alias stated")
			}
			if got.Container != "Object" || got.Member != "entries" {
				t.Fatalf("alias = %#v, want Object.entries", got)
			}
			if len(got.ContainerEscapes) != len(testCase.wantEscapes) {
				t.Fatalf("escapes = %#v, want %#v", got.ContainerEscapes, testCase.wantEscapes)
			}
			for index, want := range testCase.wantEscapes {
				if got.ContainerEscapes[index] != want {
					t.Fatalf("escapes = %#v, want %#v", got.ContainerEscapes, testCase.wantEscapes)
				}
			}
		})
	}
}

// The alias fact for a single-file JavaScript module whose export is `entries`,
// reached through an export specifier list as every bundler emits it.
func defaultLibraryAliasForTest(t *testing.T, runtime string) *typefacts.DefaultLibraryAlias {
	t.Helper()
	dir := t.TempDir()
	write := func(name, source string) {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("index.js", runtime)
	write("tsconfig.json", `{"compilerOptions":{"strict":false,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false,"types":[]},"files":["harness.ts","index.js"]}`)
	harness := "import { entries } from \"./index.js\";\nvoid entries;\n"
	write("harness.ts", harness)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { opened.Close() })
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	implementation := strings.LastIndex(runtime, "export { ") + len("export { ")
	query := strings.Index(harness, "void entries") + len("void ")
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{
			Location: typefacts.Location{
				Path: filepath.Join(dir, "harness.ts"), StartByte: query, EndByte: query + len("entries"),
			},
			ImplementationLocation: &typefacts.Location{
				Path: filepath.Join(dir, "index.js"), StartByte: implementation, EndByte: implementation + len("entries"),
			},
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
		t.Fatalf("transcripts = %#v, want one with an implementation", answer.Transcripts)
	}
	return answer.Transcripts[0].Implementation.DefaultLibraryAlias
}

func TestDefaultLibraryAliasIsSuppressedByAReceiverEscapeInTheSameFile(t *testing.T) {
	const alias = "const entries = Object.entries;\n"
	cases := []struct {
		name    string
		extra   string
		wantHit bool
	}{
		// The bundle's shape minus the escape: the fact is stated.
		{name: "aliasAlone", extra: "", wantHit: true},
		// Reading another member of the same container is a read, not an escape.
		{name: "siblingMemberAlias", extra: "const same = Object.is;\n", wantHit: true},
		// Calling a member is a call, not an escape.
		{name: "memberCall", extra: "const merge = (a) => Object.assign({}, a);\n", wantHit: true},
		// Binding to something else never names the container as an argument.
		{name: "bindToOther", extra: "const eq = Object.is.bind(undefined);\n", wantHit: true},
		// `@solid-primitives/utils@7.0.0-next.4`, line 18. Until ADR 0112 this
		// withdrew the fact from the whole file; it is now stated with the
		// escape named, which
		// `TestDefaultLibraryAliasNamesAReviewableContainerEscape` owns.
		{name: "containerAsArgumentIsNowNamed", extra: "const eq = Object.is.bind(Object);\n", wantHit: true},
		// An escape that is *not* the reviewed shape still withdraws it.
		{name: "containerAsAPlainArgument", extra: "Object.defineProperty(Object, \"x\", { value: 1 });\n", wantHit: false},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			dir := t.TempDir()
			write := func(name, source string) {
				if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			runtime := testCase.extra + alias + "export { entries };\n"
			write("index.js", runtime)
			write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false,"types":[]},"files":["harness.ts","index.js"]}`)
			harness := "import { entries } from \"./index.js\";\nvoid entries;\n"
			write("harness.ts", harness)

			opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()
			analyzer := opened.(typefacts.ExportValueAnalyzer)
			// The runtime binding the certification replays is the name in the
			// export specifier list, which is how every bundler emits this.
			implementation := strings.LastIndex(runtime, "export { ") + len("export { ")
			query := strings.Index(harness, "void entries") + len("void ")
			answer, err := analyzer.ExportValueTranscripts(
				context.Background(),
				[]typefacts.ExportValueDemand{{
					Location: typefacts.Location{
						Path: filepath.Join(dir, "harness.ts"), StartByte: query, EndByte: query + len("entries"),
					},
					ImplementationLocation: &typefacts.Location{
						Path: filepath.Join(dir, "index.js"), StartByte: implementation, EndByte: implementation + len("entries"),
					},
				}},
			)
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatalf("transcripts = %#v, want one with an implementation", answer.Transcripts)
			}
			got := answer.Transcripts[0].Implementation
			if testCase.wantHit {
				if got.DefaultLibraryAlias == nil {
					t.Fatalf("no alias stated; open reasons = %v", got.OpenReasons)
				}
				if got.DefaultLibraryAlias.Container != "Object" || got.DefaultLibraryAlias.Member != "entries" {
					t.Fatalf("alias = %#v, want Object.entries", got.DefaultLibraryAlias)
				}
				return
			}
			if got.DefaultLibraryAlias != nil {
				t.Fatalf(
					"alias = %#v: this escape is not ADR 0112's reviewed shape and must "+
						"still withdraw the fact from the whole file",
					got.DefaultLibraryAlias,
				)
			}
			// The open reason is unchanged either way: this guard withdraws the
			// fact, it does not add a refusal of its own.
			if len(got.OpenReasons) != 1 || got.OpenReasons[0] != "callSignatureNotUnique" {
				t.Fatalf("open reasons = %v, want exactly [callSignatureNotUnique]", got.OpenReasons)
			}
		})
	}
}
