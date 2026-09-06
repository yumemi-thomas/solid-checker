package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// The published shape ADR 0038 is about: an unannotated JavaScript
// implementation beside the declaration file a consumer compiles against, and
// a harness that imports the export the way certification's does.
const premiseRuntimeSource = `export function clamp(min, max, v) {
  return v > max ? max : v < min ? min : v;
}

export function widen(value) {
  return value + 1;
}

export const scale = (factor) => (p) => p * factor;

export function span(axis) {
  return axis.max - axis.min;
}

function subtract(a, b) {
  return a - b;
}

export function viaHelper(a, b) {
  return subtract(a, b);
}

export function spreadDeclared(items) {
  return [...items];
}

/** @param {number} n */
export function annotated(n, m) {
  return n + m;
}

export function arity(a) {
  return a + 1;
}

/**
 * Converts seconds to milliseconds.
 *
 * @param seconds - Time in seconds.
 * @return milliseconds - Converted time.
 */
export const documented = (seconds) => seconds * 1000;

export const mirrorAlias = (easing) => (p) => p <= 0.5 ? easing(2 * p) / 2 : (2 - easing(2 * (1 - p))) / 2;

export const aliasTuple = ([a, b, c, d]) => "cubic-bezier(" + a + ", " + b + ", " + c + ", " + d + ")";
`

const premiseDeclarationSource = `export declare function clamp(min: number, max: number, v: number): number;
export declare function widen(value: unknown): unknown;
export declare function scale(factor: number): (p: number) => number;
export interface Axis { min: number; max: number }
export declare function span(axis: Axis): number;
export declare function viaHelper(a: number, b: number): number;
export declare function spreadDeclared(items: Iterable<number>): number[];
export declare function annotated(n: number, m: number): number;
export declare function arity(a: number, b: number): number;
export declare const documented: (seconds: number) => number;
export type EasingFunction = (v: number) => number;
export declare const mirrorAlias: (easing: EasingFunction) => EasingFunction;
export type BezierDefinition = [number, number, number, number];
export declare const aliasTuple: (definition: BezierDefinition) => string;
export type ClampFn = typeof clamp;
export declare const clampConst: (min: number, max: number, v: number) => number;
`

func premiseProject(t *testing.T) (typefacts.ExportValueAnalyzer, string) {
	t.Helper()
	return premiseProjectWith(t, `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false,"types":[]},"files":["harness.ts","pkg/index.js","pkg/index.d.ts"]}`)
}

func premiseProjectWith(t *testing.T, tsconfig string) (typefacts.ExportValueAnalyzer, string) {
	t.Helper()
	dir := t.TempDir()
	write := func(name, source string) {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("tsconfig.json", tsconfig)
	if err := os.MkdirAll(filepath.Join(dir, "pkg"), 0o755); err != nil {
		t.Fatal(err)
	}
	write("pkg/index.js", premiseRuntimeSource)
	write("pkg/index.d.ts", premiseDeclarationSource)
	write("harness.ts", `import { clamp, widen, scale, span, viaHelper, spreadDeclared, annotated, arity, documented, mirrorAlias, aliasTuple } from "./pkg/index.js";
export const subjects = [clamp, widen, scale, span, viaHelper, spreadDeclared, annotated, arity, documented, mirrorAlias, aliasTuple];
`)
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

// premiseTranscript demands the export as certification does: the harness
// identifier is the demand, the runtime declaration's name is the
// implementation.
func premiseTranscript(t *testing.T, analyzer typefacts.ExportValueAnalyzer, dir, name string) typefacts.ExportImplementationTranscript {
	t.Helper()
	harness, err := os.ReadFile(filepath.Join(dir, "harness.ts"))
	if err != nil {
		t.Fatal(err)
	}
	subjects := string(harness)
	subjectStart := strings.Index(subjects, "subjects = [") + len("subjects = [")
	offset := strings.Index(subjects[subjectStart:], name)
	if offset < 0 {
		t.Fatalf("harness does not list %q", name)
	}
	anchor := typefacts.Location{
		Path:      filepath.Join(dir, "harness.ts"),
		StartByte: subjectStart + offset,
		EndByte:   subjectStart + offset + len(name),
	}
	implementationStart := strings.Index(premiseRuntimeSource, "function "+name+"(")
	if implementationStart >= 0 {
		implementationStart += len("function ")
	} else {
		implementationStart = strings.Index(premiseRuntimeSource, "const "+name+" =") + len("const ")
	}
	implementation := typefacts.Location{
		Path:      filepath.Join(dir, "pkg", "index.js"),
		StartByte: implementationStart,
		EndByte:   implementationStart + len(name),
	}
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{Location: anchor, ImplementationLocation: &implementation}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
		t.Fatalf("transcripts for %q = %#v, want one carrying an implementation", name, answer.Transcripts)
	}
	if answer.Transcripts[0].CallSignature == nil {
		t.Fatalf("%q: the harness identifier must resolve the declared call signature", name)
	}
	return *answer.Transcripts[0].Implementation
}

func TestDeclaredSignaturePremiseClearsACoercionOverADeclaredNumber(t *testing.T) {
	analyzer, dir := premiseProject(t)
	started := time.Now()
	transcript := premiseTranscript(t, analyzer, dir, "clamp")
	t.Logf("clamp transcript with premise twin took %s", time.Since(started))
	if !transcript.Complete {
		t.Fatalf("clamp transcript = %#v, want complete", transcript)
	}
	if got := markerKinds(transcript.UncensusedInvokingForms); len(got) != 0 {
		t.Fatalf("clamp uncensused forms under the declared signature = %v, want none (refusal %q)", got, transcript.ParameterPremiseRefusal)
	}
	if len(transcript.ParameterPremises) != 3 {
		t.Fatalf("clamp premises = %#v, want one per parameter (refusal %q)", transcript.ParameterPremises, transcript.ParameterPremiseRefusal)
	}
	for index, premise := range transcript.ParameterPremises {
		if premise.Index != index || premise.Type != "number" {
			t.Fatalf("clamp premise %d = %#v, want number at %d", index, premise, index)
		}
	}
}

func TestDeclaredSignaturePremiseKeepsRefusingUnknownAndHelpersAndProtocolContracts(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, testCase := range []struct {
		export string
		want   []typefacts.UncensusedInvokingFormKind
		// premised says whether the twin bound; the refusing exports below
		// still bind it, and refuse on the type the signature declares.
		premised bool
	}{
		// `unknown` is not provably a non-object; the premise binds and the
		// coercion stands under it.
		{"widen", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}, true},
		// `Iterable<number>` is a structural contract whose iterator is the
		// caller's; the premise binds and the iteration protocol stands.
		{"spreadDeclared", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol}, true},
		// The root has no form of its own: nothing to clear, no twin built.
		{"viaHelper", nil, false},
	} {
		transcript := premiseTranscript(t, analyzer, dir, testCase.export)
		if got := markerKinds(transcript.UncensusedInvokingForms); strings.Join(kindStrings(got), ",") != strings.Join(kindStrings(testCase.want), ",") {
			t.Errorf("%s uncensused forms = %v, want %v (refusal %q)", testCase.export, got, testCase.want, transcript.ParameterPremiseRefusal)
		}
		if (len(transcript.ParameterPremises) != 0) != testCase.premised {
			t.Errorf("%s premises = %#v, premised = %v (refusal %q)", testCase.export, transcript.ParameterPremises, testCase.premised, transcript.ParameterPremiseRefusal)
		}
	}
}

func TestDeclaredSignaturePremiseTypesAReturnedArrowAndADeclaredMember(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, export := range []string{"scale", "span"} {
		transcript := premiseTranscript(t, analyzer, dir, export)
		kinds := markerKinds(transcript.UncensusedInvokingForms)
		if len(transcript.ParameterPremises) == 0 {
			t.Errorf("%s: premise not bound: %q", export, transcript.ParameterPremiseRefusal)
			continue
		}
		for _, kind := range kinds {
			if kind == typefacts.UncensusedCoercion {
				t.Errorf("%s: coercion still recorded under the declared signature: %v", export, kinds)
			}
		}
	}
	// `span` reads `axis.max` and `axis.min` on a declared interface: the
	// member is a declaration-file property, not runtime bytes, so the read
	// stays an unknown accessor — rooted at parameter 0, which is what the
	// verifier dispositions. The premise cleared the coercion, not the read.
	span := premiseTranscript(t, analyzer, dir, "span")
	if len(span.UncensusedInvokingForms) != 2 {
		t.Fatalf("span forms = %#v, want the two member reads", span.UncensusedInvokingForms)
	}
	for _, form := range span.UncensusedInvokingForms {
		if form.Kind != typefacts.UncensusedPropertyAccessUnknownAccessor || form.SubjectParameter == nil || *form.SubjectParameter != 0 {
			t.Fatalf("span form = %#v, want a parameter-0-rooted unknown accessor", form)
		}
		if form.Location.Path != filepath.Join(dir, "pkg", "index.js") ||
			!strings.HasPrefix(premiseRuntimeSource[form.Location.StartByte:form.Location.EndByte], "axis.m") {
			t.Fatalf("span form location %#v does not name the original bytes: %q", form.Location, premiseRuntimeSource[form.Location.StartByte:form.Location.EndByte])
		}
	}
}

// A JSDoc that documents parameters without typing them — the shape bundled
// output keeps from the authors' comments — carries no type tag, so the
// premise binds over it.
func TestDeclaredSignaturePremiseBindsOverADescriptionOnlyJSDoc(t *testing.T) {
	analyzer, dir := premiseProject(t)
	transcript := premiseTranscript(t, analyzer, dir, "documented")
	if len(transcript.ParameterPremises) != 1 || transcript.ParameterPremises[0].Type != "number" {
		t.Fatalf("documented premises = %#v (refusal %q), want the declared number", transcript.ParameterPremises, transcript.ParameterPremiseRefusal)
	}
	if got := markerKinds(transcript.UncensusedInvokingForms); len(got) != 0 {
		t.Fatalf("documented forms = %v, want none under the declared signature", got)
	}
}

// A parameter or return type the declaration names through an alias cannot
// be spelled from the twin's scope; the `import()` twin binds it by identity.
func TestDeclaredSignaturePremiseFallsBackToTheImportTwinForAliasedTypes(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, export := range []string{"mirrorAlias", "aliasTuple"} {
		transcript := premiseTranscript(t, analyzer, dir, export)
		if len(transcript.ParameterPremises) != 1 {
			t.Errorf("%s: premise not bound: %q", export, transcript.ParameterPremiseRefusal)
			continue
		}
		for _, form := range transcript.UncensusedInvokingForms {
			if form.Kind == typefacts.UncensusedCoercion {
				t.Errorf("%s: coercion still recorded under the declared signature: %#v", export, form)
			}
		}
	}
}

func TestDeclaredSignaturePremiseRefusesAnnotatedAndMismatchedDeclarations(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, testCase := range []struct {
		export  string
		refusal string
	}{
		{"annotated", "already carries a JSDoc type tag"},
		{"arity", "implementation declares 1 parameter(s), the signature 2"},
	} {
		transcript := premiseTranscript(t, analyzer, dir, testCase.export)
		if len(transcript.ParameterPremises) != 0 {
			t.Errorf("%s: premise bound %#v, want refusal %q", testCase.export, transcript.ParameterPremises, testCase.refusal)
		}
		if !strings.Contains(transcript.ParameterPremiseRefusal, testCase.refusal) {
			t.Errorf("%s: refusal = %q, want %q", testCase.export, transcript.ParameterPremiseRefusal, testCase.refusal)
		}
		if got := markerKinds(transcript.UncensusedInvokingForms); len(got) != 1 || got[0] != typefacts.UncensusedCoercion {
			t.Errorf("%s: forms = %v, want the coercion over the parameter's own type", testCase.export, got)
		}
	}
}

func kindStrings(kinds []typefacts.UncensusedInvokingFormKind) []string {
	out := make([]string, len(kinds))
	for index, kind := range kinds {
		out[index] = string(kind)
	}
	return out
}
