package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"slices"
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

function lengthOf(axis) {
  return axis.max - axis.min;
}

export function applyBoxDelta(box, { x, y }) {
  applyAxisDelta(box.x, x.translate, x.scale, x.originPoint);
}

function applyAxisDelta(axis, translate = 0, scale = 1, originPoint, boxScale) {
  axis.min = applyPointDelta(axis.min, translate, scale, originPoint, boxScale);
}

function applyPointDelta(point, translate, scale, originPoint, boxScale) {
  if (boxScale !== undefined) {
    point = scalePoint(point, boxScale, originPoint);
  }
  return scalePoint(point, scale, originPoint) + translate;
}

function scalePoint(point, scale, originPoint) {
  const distanceFromOrigin = point - originPoint;
  const scaled = scale * distanceFromOrigin;
  return originPoint + scaled;
}

function scaleWithin(value, factor, bias) {
  return value * factor + bias;
}

function widenWithin(value, bias) {
  bias = 1;
  return value + bias;
}

export function viaAxisHelper(axis) {
  return lengthOf(axis);
}

export function viaOptionalAxisHelper(axis) {
  return optionalLength(axis);
}

function optionalLength(axis) {
  if (!axis) return 0;
  return axis.max - axis.min;
}

export function omitsTrailingArgument(value) {
  return scaleWithin(value, 2);
}

export function omitsWrittenArgument(value) {
  return widenWithin(value);
}
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
export declare function viaAxisHelper(axis: Axis): number;
export declare function viaOptionalAxisHelper(axis?: Axis): number;
export declare interface AxisDelta { translate: number; scale: number; originPoint: number }
export declare interface BoxDelta { x: AxisDelta; y: AxisDelta }
export declare interface Box { x: Axis; y: Axis }
export declare function applyBoxDelta(box: Box, delta: BoxDelta): void;
export declare function omitsTrailingArgument(value: number): number;
export declare function omitsWrittenArgument(value: number): number;
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
	write("pkg/shadow.d.ts", "export interface Axis { min: number; max: number }")
	write("harness.ts", `import { clamp, widen, scale, span, viaHelper, viaAxisHelper, viaOptionalAxisHelper, applyBoxDelta, omitsTrailingArgument, omitsWrittenArgument, spreadDeclared, annotated, arity, documented, mirrorAlias, aliasTuple } from "./pkg/index.js";
export const subjects = [clamp, widen, scale, span, viaHelper, viaAxisHelper, viaOptionalAxisHelper, applyBoxDelta, omitsTrailingArgument, omitsWrittenArgument, spreadDeclared, annotated, arity, documented, mirrorAlias, aliasTuple];
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
		// The root has no form of its own, but it calls a local helper: the
		// twin is built to record the argument types at that call (protocol
		// 23), and the premise binds.
		{"viaHelper", nil, true},
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

// The 2026-09-28 amendment to ADR 0149: the premise types these operands,
// and a type is not proof. `scale`'s `p` is the returned arrow's own
// parameter, whose value its invoker chooses, and `span`'s `axis.max` is a
// member read of the caller's object; each coercion is recorded under the
// declared signature, with the root the census would need.
func TestDeclaredSignaturePremiseTypesAReturnedArrowAndADeclaredMember(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, testCase := range []struct {
		export  string
		root    typefacts.SubjectRootDerivation
		refusal typefacts.SubjectRootRefusalReason
	}{
		{"scale", "", typefacts.SubjectRefusalNestedParameter},
		{"span", typefacts.SubjectRootParameter, ""},
	} {
		transcript := premiseTranscript(t, analyzer, dir, testCase.export)
		if len(transcript.ParameterPremises) == 0 {
			t.Errorf("%s: premise not bound: %q", testCase.export, transcript.ParameterPremiseRefusal)
			continue
		}
		coercions := 0
		for _, form := range transcript.UncensusedInvokingForms {
			if form.Kind != typefacts.UncensusedCoercion {
				continue
			}
			coercions++
			if form.CoercionSubjectRoot != testCase.root || form.CoercionSubjectRootRefusal != testCase.refusal {
				t.Errorf("%s: coercion root %q refusal %q, want %q %q", testCase.export,
					form.CoercionSubjectRoot, form.CoercionSubjectRootRefusal, testCase.root, testCase.refusal)
			}
		}
		if coercions != 1 {
			t.Errorf("%s: %d coercions under the declared signature, want the one", testCase.export, coercions)
		}
	}
	// `span` reads `axis.max` and `axis.min` on a declared interface: the
	// member is a declaration-file property, not runtime bytes, so the read
	// stays an unknown accessor — rooted at parameter 0, which is what the
	// verifier dispositions.
	span := premiseTranscript(t, analyzer, dir, "span")
	if len(span.UncensusedInvokingForms) != 3 {
		t.Fatalf("span forms = %#v, want the coercion and the two member reads", span.UncensusedInvokingForms)
	}
	for _, form := range span.UncensusedInvokingForms {
		if form.Kind == typefacts.UncensusedCoercion {
			continue
		}
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
// The coercions stand under it (the 2026-09-28 amendment to ADR 0149): their
// operands are a returned arrow's own parameter and destructured elements,
// which the declared types describe and nothing proves.
func TestDeclaredSignaturePremiseFallsBackToTheImportTwinForAliasedTypes(t *testing.T) {
	analyzer, dir := premiseProject(t)
	for _, export := range []string{"mirrorAlias", "aliasTuple"} {
		transcript := premiseTranscript(t, analyzer, dir, export)
		if len(transcript.ParameterPremises) != 1 {
			t.Errorf("%s: premise not bound: %q", export, transcript.ParameterPremiseRefusal)
			continue
		}
		if !slices.Contains(markerKinds(transcript.UncensusedInvokingForms), typefacts.UncensusedCoercion) {
			t.Errorf("%s: no coercion recorded over operands only a declared type describes", export)
		}
		for _, form := range transcript.UncensusedInvokingForms {
			if form.Kind == typefacts.UncensusedCoercion && form.CoercionSubjectRoot == typefacts.SubjectRootParameter {
				t.Errorf("%s: coercion rooted at the caller's parameter: %#v", export, form)
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

// premiseLocalTranscript demands a local declaration's implementation
// transcript the way the verifier's census does — the harness identifier of
// the export that reached it as the anchor, the exact span of the helper's
// declaration node, and the argument premises carried back from the caller.
func premiseLocalTranscript(
	t *testing.T,
	analyzer typefacts.ExportValueAnalyzer,
	dir, export, helper string,
	premises []typefacts.ParameterPremise,
) typefacts.ExportImplementationTranscript {
	t.Helper()
	harness, err := os.ReadFile(filepath.Join(dir, "harness.ts"))
	if err != nil {
		t.Fatal(err)
	}
	subjects := string(harness)
	subjectStart := strings.Index(subjects, "subjects = [") + len("subjects = [")
	offset := strings.Index(subjects[subjectStart:], export)
	anchor := typefacts.Location{
		Path:      filepath.Join(dir, "harness.ts"),
		StartByte: subjectStart + offset,
		EndByte:   subjectStart + offset + len(export),
	}
	start := strings.Index(premiseRuntimeSource, "function "+helper+"(")
	if start < 0 {
		t.Fatalf("runtime source declares no %q", helper)
	}
	end := start + strings.Index(premiseRuntimeSource[start:], "\n}\n") + len("\n}")
	declaration := typefacts.Location{
		Path:      filepath.Join(dir, "pkg", "index.js"),
		StartByte: start,
		EndByte:   end,
	}
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{
			Location:                 anchor,
			LocalDeclarationLocation: &declaration,
			ParameterPremises:        premises,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].LocalDeclaration == nil {
		t.Fatalf("transcripts for %q = %#v, want one carrying a local declaration", helper, answer.Transcripts)
	}
	local := *answer.Transcripts[0].LocalDeclaration
	if !local.Complete {
		t.Fatalf("%q local transcript is open: %v", helper, local.OpenReasons)
	}
	return local
}

// Protocol 23: the caller's premised census records the argument types at the
// call that reaches a local helper, and the helper's census is classified
// under exactly those types when they are demanded back — bound by text and
// declaration identity — and under its parameters' own `any` otherwise.
// ADR 0045: a coercion states the calls its clearance rests on, and every
// transcript answers whether the value it hands its caller is provably a
// primitive. The producer states both halves and neither is a verdict — the
// granting is the consumer's, and `overObjectHelper` is here to show a premise
// stated over a call whose completion will not grant it.
//
// The helper must stay unannotated: an annotated one returns a number, the
// call site is then already a primitive, and no coercion form is recorded at
// all. That vacuity is the trap the census fixtures keep walking into.
func TestCoercionPremiseNamesItsOperandCallsAndCompletionsFollowThePremise(t *testing.T) {
	const source = `const factor: number = 2;

export function untyped(value) {
	return value;
}

export function typedHelper(value: number): number {
	return value * factor;
}

export function boxOf(value: number) {
	return { value };
}

export function overHelper(base: number) {
	return untyped(base) + base;
}

export function overBoundHelper(base: number) {
	const scaled = untyped(base);
	return scaled + base;
}

export function overConditional(base: number) {
	const scaled = base === 0 ? base : untyped(base);
	return scaled + base;
}

export function overObjectHelper(base: number) {
	return boxOf(base) + base;
}

export function overWrittenBinding(base: number) {
	let scaled = untyped(base);
	scaled = boxOf(base);
	return scaled + base;
}

export function overLibraryCall(base: number) {
	return JSON.parse("1") + base;
}
`
	analyzer, dir := markerProject(t, map[string]string{"coercions.ts": source})
	path := filepath.Join(dir, "coercions.ts")
	for _, testCase := range []struct {
		export string
		calls  int
	}{
		// One operand is the helper's result, the other a declared number.
		{"overHelper", 1},
		// The same named through a local binding, and through both arms of a
		// conditional whose other arm is already a primitive.
		{"overBoundHelper", 1},
		{"overConditional", 1},
		// A helper whose completion is an object still names its call: the
		// premise says where the value came from, never that it clears.
		{"overObjectHelper", 1},
		// A written binding may hold something else by the time the coercion
		// runs, and a default-library callee is not this program's source.
		{"overWrittenBinding", 0},
		{"overLibraryCall", 0},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, testCase.export)
		var premised, coercions int
		for _, form := range transcript.UncensusedInvokingForms {
			if form.Kind != typefacts.UncensusedCoercion {
				continue
			}
			coercions++
			if form.CoercionPremise == nil {
				continue
			}
			premised++
			if len(form.CoercionPremise.Calls) != testCase.calls {
				t.Fatalf("%s: premise names %d call(s), want %d",
					testCase.export, len(form.CoercionPremise.Calls), testCase.calls)
			}
		}
		if coercions == 0 {
			t.Fatalf("%s: no coercion form was recorded, so the case pins nothing", testCase.export)
		}
		if want := testCase.calls > 0; (premised > 0) != want {
			t.Fatalf("%s: %d premised coercion(s), want stated=%v", testCase.export, premised, want)
		}
	}

	// The completion is read off the same program the forms were classified
	// on: the annotated helper hands back a number, the unannotated one an
	// `any` nobody has typed, and the boxing one an object.
	for _, testCase := range []struct {
		export    string
		primitive bool
	}{
		{"typedHelper", true},
		{"untyped", false},
		{"boxOf", false},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, testCase.export)
		if transcript.PrimitiveCompletion != testCase.primitive {
			t.Fatalf("%s: primitive completion = %v, want %v",
				testCase.export, transcript.PrimitiveCompletion, testCase.primitive)
		}
	}
}

// ADR 0049: an argument slot the call does not write receives `undefined`.
//
// It is a semantic fact — the call has fewer arguments than the callee has
// parameters — so it is available where no declaration is, and it is a
// *stronger* premise than the `any` an unannotated parameter carries. The
// second case pins the one slot this refuses to speak for: a parameter the
// callee's own body writes holds something else by the time the body reads it.
func TestOmittedArgumentSlotsArePremisedAsUndefined(t *testing.T) {
	analyzer, dir := premiseProject(t)
	caller := premiseTranscript(t, analyzer, dir, "omitsTrailingArgument")
	if len(caller.CallArgumentPremises) != 1 {
		t.Fatalf("call argument premises = %#v (refusal %q), want the one call to scaleWithin",
			caller.CallArgumentPremises, caller.ParameterPremiseRefusal)
	}
	recorded := caller.CallArgumentPremises[0].Arguments
	if len(recorded) != 3 {
		t.Fatalf("recorded arguments = %#v, want both written slots and the omitted one", recorded)
	}
	if recorded[2].Index != 2 || recorded[2].Type != "undefined" {
		t.Fatalf("omitted slot = %#v, want index 2 typed undefined", recorded[2])
	}

	// Demanded back, the helper binds it and its coercions clear — which is
	// also what pins that the identity built from the flag constant is the one
	// the checker's own `undefined` type carries.
	helper := premiseLocalTranscript(t, analyzer, dir, "omitsTrailingArgument", "scaleWithin", recorded)
	if len(helper.ParameterPremises) != 3 {
		t.Fatalf("scaleWithin premises = %#v (refusal %q), want all three bound",
			helper.ParameterPremises, helper.ParameterPremiseRefusal)
	}
	if kinds := markerKinds(helper.UncensusedInvokingForms); len(kinds) != 0 {
		t.Fatalf("scaleWithin forms under the premise = %v, want none", kinds)
	}

	// A parameter the callee writes is skipped: the premise would describe a
	// value the body has already replaced.
	written := premiseTranscript(t, analyzer, dir, "omitsWrittenArgument")
	if len(written.CallArgumentPremises) != 1 {
		t.Fatalf("omitsWrittenArgument premises = %#v (refusal %q)",
			written.CallArgumentPremises, written.ParameterPremiseRefusal)
	}
	for _, argument := range written.CallArgumentPremises[0].Arguments {
		if argument.Index == 1 {
			t.Fatalf("a written parameter was premised as undefined: %#v", argument)
		}
	}
}

// ADR 0049, end to end over the shape it was written for: motion-dom's
// projection geometry, where a root premised by its declared signature reaches
// a helper through a call that **omits** the trailing optional argument.
//
// Without the omitted slot the chain loses `boxScale` at the first hop and
// every premise below it degrades to `any`. With it the slot is carried as
// `undefined` and each hop re-states it, which is what this pins: two hops,
// and the premise still naming every parameter at the second.
func TestAnOmittedSlotSurvivesTheHelperPremiseChain(t *testing.T) {
	analyzer, dir := premiseProject(t)
	root := premiseTranscript(t, analyzer, dir, "applyBoxDelta")
	if root.ParameterPremiseRefusal != "" || len(root.ParameterPremises) != 2 {
		t.Fatalf("applyBoxDelta premises = %#v, refusal %q",
			root.ParameterPremises, root.ParameterPremiseRefusal)
	}
	if len(root.CallArgumentPremises) != 1 {
		t.Fatalf("applyBoxDelta call premises = %#v", root.CallArgumentPremises)
	}
	first := root.CallArgumentPremises[0].Arguments
	if len(first) != 5 || first[4].Index != 4 || first[4].Type != "undefined" {
		t.Fatalf("first hop = %#v, want four written slots and the omitted one", first)
	}

	// The second hop: the helper binds all five and re-states them at its own
	// call, which is the propagation the chain lives on.
	axis := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "applyAxisDelta", first)
	if axis.ParameterPremiseRefusal != "" || len(axis.ParameterPremises) != 5 {
		t.Fatalf("applyAxisDelta premises = %#v, refusal %q",
			axis.ParameterPremises, axis.ParameterPremiseRefusal)
	}
	if len(axis.CallArgumentPremises) != 1 {
		t.Fatalf("applyAxisDelta call premises = %#v", axis.CallArgumentPremises)
	}
	second := axis.CallArgumentPremises[0].Arguments
	if len(second) != 5 || second[4].Type != "undefined" {
		t.Fatalf("second hop = %#v, want all five slots with the omitted one still undefined", second)
	}
}

// ADR 0051: the omitted slot narrows to never in the first scalePoint call.
// Its explicit bottom type is not an unavailable type. The second call still
// has point:number despite the assignment above it; do not replace this
// measured chain with a reconstruction about an any-poisoned parameter.
func TestBottomTypePremiseSurvivesBothGeometryCalls(t *testing.T) {
	analyzer, dir := premiseProject(t)
	root := premiseTranscript(t, analyzer, dir, "applyBoxDelta")
	axis := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "applyAxisDelta", root.CallArgumentPremises[0].Arguments)
	point := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "applyPointDelta", axis.CallArgumentPremises[0].Arguments)
	if len(point.CallArgumentPremises) != 2 {
		t.Fatalf("point call premises = %#v, want both scalePoint calls", point.CallArgumentPremises)
	}
	// The parent still needs its callee's primitive completion. This is not a
	// vacuous case where annotating the caller erased the whole coercion.
	if len(point.UncensusedInvokingForms) != 1 || point.UncensusedInvokingForms[0].CoercionPremise == nil {
		t.Fatalf("point forms = %#v, want one completion-premised coercion", point.UncensusedInvokingForms)
	}
	for index, call := range point.CallArgumentPremises {
		middle := "number"
		if index == 0 {
			middle = "never"
		}
		if len(call.Arguments) != 3 || call.Arguments[0].Type != "number" ||
			call.Arguments[1].Type != middle || call.Arguments[1].Identity == "" || call.Arguments[2].Type != "number" {
			t.Fatalf("scalePoint call %d arguments = %#v", index, call.Arguments)
		}
		leaf := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "scalePoint", call.Arguments)
		if leaf.ParameterPremiseRefusal != "" || !slices.Equal(leaf.ParameterPremises, call.Arguments) {
			t.Fatalf("leaf %d did not bind the exact premise: %#v, refusal %q", index, leaf.ParameterPremises, leaf.ParameterPremiseRefusal)
		}
		if kinds := markerKinds(leaf.UncensusedInvokingForms); len(kinds) != 0 || !leaf.PrimitiveCompletion {
			t.Fatalf("leaf %d forms=%v primitive=%v, want a clear primitive completion", index, kinds, leaf.PrimitiveCompletion)
		}
	}
	first := point.CallArgumentPremises[0].Arguments
	// Removing the slot leaves an actual coercion: no premise is not bottom.
	missing := []typefacts.ParameterPremise{first[0], first[2]}
	leaf := premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "scalePoint", missing)
	if kinds := markerKinds(leaf.UncensusedInvokingForms); len(kinds) != 1 || kinds[0] != typefacts.UncensusedCoercion {
		t.Fatalf("missing bottom premise forms=%v, want a recorded coercion", kinds)
	}
	// A spelling of never with another type's identity must not bind.
	forged := slices.Clone(first)
	forged[1].Identity = first[0].Identity
	leaf = premiseLocalTranscript(t, analyzer, dir, "applyBoxDelta", "scalePoint", forged)
	if leaf.ParameterPremiseRefusal == "" || len(leaf.ParameterPremises) != 0 || len(leaf.UncensusedInvokingForms) == 0 {
		t.Fatalf("forged bottom identity was not refused: %#v", leaf)
	}
}

// The shared predicate serves the form classifier, operand-premise walk and
// completion fact. Explicit bottom has no object-valued normal completion;
// missing, any, unknown, generic and object types do not establish that fact.
func TestBottomTypeIsAnExplicitNonObjectFact(t *testing.T) {
	const source = `
export function bottom(value: never) { return +value; }
export function unknownValue(value: unknown) { return +value; }
export function anyValue(value: any) { return +value; }
export function genericValue<T>(value: T) { return +value; }
export function objectValue(value: object) { return +value; }
export function noCompletion(): never { throw 1; }
export function unknownCompletion(value: unknown) { return value; }
export function objectCompletion(value: object) { return value; }
`
	analyzer, dir := markerProject(t, map[string]string{"bottom.ts": source})
	path := filepath.Join(dir, "bottom.ts")
	for _, name := range []string{"bottom", "unknownValue", "anyValue", "genericValue", "objectValue"} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, name)
		want := 1
		if name == "bottom" {
			want = 0
		}
		if got := len(transcript.UncensusedInvokingForms); got != want {
			t.Fatalf("%s: %d forms, want %d", name, got, want)
		}
	}
	for _, name := range []string{"noCompletion", "unknownCompletion", "objectCompletion"} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, name)
		if transcript.PrimitiveCompletion != (name == "noCompletion") {
			t.Fatalf("%s: primitive completion=%v", name, transcript.PrimitiveCompletion)
		}
	}
	var unopened project
	if !unopened.mayBeObjectTypedLocked(nil) {
		t.Fatal("an unavailable type is not an explicit bottom type")
	}
}

// ADR 0046: a helper premise whose type the *helper's* module cannot name.
//
// `viaAxisHelper(axis: Axis)` hands its own parameter to a module-local
// `lengthOf`, so the recorded argument type is `Axis` — a name that resolves
// in the caller's twin and in nothing else. Spelled `@param {Axis} axis` into
// a JavaScript module the twin refuses it, the helper is censused over `any`,
// and `axis.max - axis.min` refuses for a reason that is about spelling rather
// than about the code.
func TestOptionalImportedHelperPremisePreservesEveryConstituentIdentity(t *testing.T) {
	analyzer, dir := premiseProject(t)
	caller := premiseTranscript(t, analyzer, dir, "viaOptionalAxisHelper")
	if len(caller.CallArgumentPremises) != 1 || len(caller.CallArgumentPremises[0].Arguments) != 1 {
		t.Fatalf("optional caller premises: %+v; refusal %q", caller.CallArgumentPremises, caller.ParameterPremiseRefusal)
	}
	recorded := caller.CallArgumentPremises[0].Arguments
	raw := premiseLocalTranscript(t, analyzer, dir, "viaOptionalAxisHelper", "optionalLength", nil)
	if !slices.Contains(markerKinds(raw.UncensusedInvokingForms), typefacts.UncensusedCoercion) {
		t.Fatal("unpremised helper must actually record its coercion")
	}
	if !strings.Contains(recorded[0].Spelling, "import(") || !strings.Contains(recorded[0].Spelling, "undefined") {
		t.Fatalf("optional imported type has no complete spelling: %+v", recorded[0])
	}
	helper := premiseLocalTranscript(t, analyzer, dir, "viaOptionalAxisHelper", "optionalLength", recorded)
	if len(helper.ParameterPremises) != 1 || helper.ParameterPremises[0] != recorded[0] {
		t.Fatalf("optional premise failed: %+v; refusal %q; forms %+v", helper.ParameterPremises, helper.ParameterPremiseRefusal, helper.UncensusedInvokingForms)
	}
	// The member reads are typed by the premise, not proved (the 2026-09-28
	// amendment to ADR 0149): the coercion stands, rooted at the parameter.
	for _, form := range helper.UncensusedInvokingForms {
		if form.Kind == typefacts.UncensusedCoercion && form.CoercionSubjectRoot != typefacts.SubjectRootParameter {
			t.Fatalf("optionalLength coercion is not rooted at the premised parameter: %+v", form)
		}
	}
	for _, spelling := range []string{
		strings.ReplaceAll(recorded[0].Spelling, "undefined", "null"),
		strings.ReplaceAll(recorded[0].Spelling, "/index", "/shadow"),
	} {
		forged := append([]typefacts.ParameterPremise(nil), recorded...)
		forged[0].Spelling = spelling
		refused := premiseLocalTranscript(t, analyzer, dir, "viaOptionalAxisHelper", "optionalLength", forged)
		if len(refused.ParameterPremises) != 0 || refused.ParameterPremiseRefusal == "" {
			t.Fatalf("forged union premise bound: %q -> %+v", spelling, refused.ParameterPremises)
		}
	}
}

func TestAHelperPremiseIsSpelledAsAnImportTypeWhenItsNameIsForeign(t *testing.T) {
	analyzer, dir := premiseProject(t)
	caller := premiseTranscript(t, analyzer, dir, "viaAxisHelper")
	if len(caller.CallArgumentPremises) != 1 {
		t.Fatalf("viaAxisHelper call argument premises = %#v (refusal %q), want the one call to lengthOf",
			caller.CallArgumentPremises, caller.ParameterPremiseRefusal)
	}
	recorded := caller.CallArgumentPremises[0].Arguments
	if len(recorded) != 1 || recorded[0].Type != "Axis" {
		t.Fatalf("recorded arguments = %#v, want the declared Axis", recorded)
	}
	if !strings.HasPrefix(recorded[0].Spelling, "import(") ||
		!strings.HasSuffix(recorded[0].Spelling, ").Axis") {
		t.Fatalf("recorded spelling = %q, want an import type naming Axis", recorded[0].Spelling)
	}

	// Demanded back, the helper binds the premise through that spelling. Its
	// coercion stands (the 2026-09-28 amendment to ADR 0149): `axis.max` is a
	// member read the premise types and nothing proves, so the form is
	// recorded, rooted at the premised parameter.
	helper := premiseLocalTranscript(t, analyzer, dir, "viaAxisHelper", "lengthOf", recorded)
	// The two reads of `axis` remain beside it, and remain rooted at the
	// parameter: the premise types them, it does not turn a declaration file
	// into runtime bytes, and ADR 0034 is what dispositions them.
	if kinds := markerKinds(helper.UncensusedInvokingForms); len(kinds) != 3 {
		t.Fatalf("lengthOf forms = %v, want the coercion and the two parameter-rooted reads", kinds)
	}
	for _, form := range helper.UncensusedInvokingForms {
		if form.Kind == typefacts.UncensusedCoercion {
			if form.CoercionSubjectRoot != typefacts.SubjectRootParameter ||
				!slices.Equal(form.CoercionSubjectParameters, []int{0}) {
				t.Fatalf("lengthOf coercion %#v is not rooted at the premised parameter", form)
			}
			continue
		}
		if form.SubjectParameter == nil || *form.SubjectParameter != 0 {
			t.Fatalf("lengthOf form %#v is not rooted at the premised parameter", form)
		}
	}
	if len(helper.ParameterPremises) != 1 || helper.ParameterPremises[0] != recorded[0] {
		t.Fatalf("lengthOf premises = %#v, want the demanded %#v echoed", helper.ParameterPremises, recorded)
	}
	if !helper.PrimitiveCompletion {
		t.Fatal("lengthOf hands back a number under the premise; its completion must say so")
	}

	// The spelling is a hint, never the premise: one that resolves to another
	// type is refused by the same falsifier a wrong printed text is.
	forged := []typefacts.ParameterPremise{{
		Index: 0, Type: recorded[0].Type, Identity: recorded[0].Identity,
		Spelling: strings.Replace(recorded[0].Spelling, ").Axis", ").EasingFunction", 1),
	}}
	refused := premiseLocalTranscript(t, analyzer, dir, "viaAxisHelper", "lengthOf", forged)
	if len(refused.ParameterPremises) != 0 || refused.ParameterPremiseRefusal == "" {
		t.Fatalf("lengthOf bound a spelling naming another type: premises %#v, refusal %q",
			refused.ParameterPremises, refused.ParameterPremiseRefusal)
	}
}

func TestCallArgumentPremisesReachALocalHelper(t *testing.T) {
	analyzer, dir := premiseProject(t)
	caller := premiseTranscript(t, analyzer, dir, "viaHelper")
	if len(caller.CallArgumentPremises) != 1 {
		t.Fatalf("viaHelper call argument premises = %#v, want the one call to subtract (refusal %q)", caller.CallArgumentPremises, caller.ParameterPremiseRefusal)
	}
	recorded := caller.CallArgumentPremises[0]
	if got := premiseRuntimeSource[recorded.Call.StartByte:recorded.Call.EndByte]; got != "subtract(a, b)" {
		t.Fatalf("recorded call names %q in the original bytes, want subtract(a, b)", got)
	}
	if len(recorded.Arguments) != 2 {
		t.Fatalf("recorded arguments = %#v, want both slots", recorded.Arguments)
	}
	for index, argument := range recorded.Arguments {
		if argument.Index != index || argument.Type != "number" || argument.Identity == "" {
			t.Fatalf("recorded argument %d = %#v, want number with an identity", index, argument)
		}
	}

	// The premise the caller recorded, demanded back: the coercion clears and
	// the transcript echoes exactly what it bound.
	helper := premiseLocalTranscript(t, analyzer, dir, "viaHelper", "subtract", recorded.Arguments)
	if kinds := markerKinds(helper.UncensusedInvokingForms); len(kinds) != 0 {
		t.Fatalf("subtract forms under the caller's argument types = %v, want none (refusal %q)", kinds, helper.ParameterPremiseRefusal)
	}
	if len(helper.ParameterPremises) != 2 ||
		helper.ParameterPremises[0] != recorded.Arguments[0] || helper.ParameterPremises[1] != recorded.Arguments[1] {
		t.Fatalf("subtract premises = %#v, want the demanded %#v echoed", helper.ParameterPremises, recorded.Arguments)
	}

	// No premise: the helper's parameters are `any` and the coercion stands.
	bare := premiseLocalTranscript(t, analyzer, dir, "viaHelper", "subtract", nil)
	if kinds := markerKinds(bare.UncensusedInvokingForms); len(kinds) != 1 || kinds[0] != typefacts.UncensusedCoercion {
		t.Fatalf("subtract forms without a premise = %v, want the coercion", kinds)
	}
	if len(bare.ParameterPremises) != 0 {
		t.Fatalf("subtract states a premise nobody demanded: %#v", bare.ParameterPremises)
	}

	// One slot only: the other stays `any`, and the coercion stands under a
	// premise the transcript still echoes, since it did bind what was asked.
	partial := premiseLocalTranscript(t, analyzer, dir, "viaHelper", "subtract", recorded.Arguments[:1])
	if kinds := markerKinds(partial.UncensusedInvokingForms); len(kinds) != 1 || kinds[0] != typefacts.UncensusedCoercion {
		t.Fatalf("subtract forms under one premised slot = %v, want the coercion", kinds)
	}
	if len(partial.ParameterPremises) != 1 || partial.ParameterPremises[0] != recorded.Arguments[0] {
		t.Fatalf("subtract premises under one slot = %#v, want the one demanded", partial.ParameterPremises)
	}

	// A text that spells `number` but claims another identity is a premise
	// the twin cannot re-establish: refused, census kept over `any`.
	forged := []typefacts.ParameterPremise{
		{Index: 0, Type: "number", Identity: "flags:1|alias:/elsewhere.d.ts:0"},
		recorded.Arguments[1],
	}
	refused := premiseLocalTranscript(t, analyzer, dir, "viaHelper", "subtract", forged)
	if len(refused.ParameterPremises) != 0 || refused.ParameterPremiseRefusal == "" {
		t.Fatalf("subtract bound a forged identity: premises %#v, refusal %q", refused.ParameterPremises, refused.ParameterPremiseRefusal)
	}
	if kinds := markerKinds(refused.UncensusedInvokingForms); len(kinds) != 1 || kinds[0] != typefacts.UncensusedCoercion {
		t.Fatalf("subtract forms under a refused premise = %v, want the coercion", kinds)
	}

	// A type the twin resolves to something else — `Axis` is declared in the
	// declaration file the JavaScript module never imports — is refused too.
	unresolvable := premiseLocalTranscript(t, analyzer, dir, "viaHelper", "subtract", []typefacts.ParameterPremise{
		{Index: 0, Type: "Axis", Identity: "flags:524288|symbol:/pkg/index.d.ts:0"},
	})
	if len(unresolvable.ParameterPremises) != 0 || unresolvable.ParameterPremiseRefusal == "" {
		t.Fatalf("subtract bound an unresolvable spelling: premises %#v, refusal %q", unresolvable.ParameterPremises, unresolvable.ParameterPremiseRefusal)
	}
}

// A rest parameter, admitted 2026-09-18.
//
// The guard this replaced refused every declared rest parameter, so
// `@solid-primitives/utils`' `substract` -- `(a, ...b) => { for (const n of b)
// a -= n; return a; }` beside a `.d.ts` declaring
// `(a: number, ...b: number[]) => number` -- was censused over its parameters'
// own `any` and `a -= n` recorded a coercion form. Across the pinned corpus 46
// census refusals named that guard, over six exports in three packages
// (docs/precision-backlog.md, wall 1).
//
// `rest` is the shape now admitted; `restLeading` is the same fact with a
// fixed slot before it, which is the published arithmetic exports' own shape.
// The trailing slot is held to the declared rest parameter's *array* type, not
// to `getTypeAtPosition`'s element type, so the premise the transcript states
// is `number[]` -- what `...b` actually binds.
//
// The negatives keep every other rest shape out, because none of them has a
// position-for-position reading to hold the twin to: a rest on the
// implementation that the declaration does not have, a rest on the declaration
// that the implementation does not have, and (through the arity guard) any
// mismatch of count. `iterationReachability` stays on every one of these
// transcripts and is not this premise's business: it is a
// ControlFlowReachabilityLowerBound, which an empty-closure claim can only be
// over-estimated by, and the corpus censuses already tolerate it -- 40 of the
// arithmetic five's refusals name the coercion form and none names an open
// transcript.
func TestDeclaredSignaturePremiseAdmitsAMatchedRestParameter(t *testing.T) {
	const runtime = `export function fixed(a, b) {
  let r = 0;
  r += a;
  r += b;
  return r;
}

export function rest(...b) {
  let r = 0;
  for (const n of b) r += n;
  return r;
}

export function restLeading(a, ...b) {
  for (const n of b) a -= n;
  return a;
}

export function restOnlyInImplementation(a, ...b) {
  for (const n of b) a -= n;
  return a;
}

export function restOnlyInDeclaration(a, b) {
  return a - b;
}
`
	const declarations = `export declare function fixed(a: number, b: number): number;
export declare function rest(...b: number[]): number;
export declare function restLeading(a: number, ...b: number[]): number;
export declare function restOnlyInImplementation(a: number, b: number[]): number;
export declare function restOnlyInDeclaration(a: number, ...b: number[]): number;
`
	dir := t.TempDir()
	write := func(name, source string) {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.MkdirAll(filepath.Join(dir, "pkg"), 0o755); err != nil {
		t.Fatal(err)
	}
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true,"checkJs":false,"types":[]},"files":["harness.ts","pkg/index.js","pkg/index.d.ts"]}`)
	write("pkg/index.js", runtime)
	write("pkg/index.d.ts", declarations)
	harness := "import { fixed, rest, restLeading, restOnlyInImplementation, restOnlyInDeclaration } from \"./pkg/index.js\";\n" +
		"export const subjects = [fixed, rest, restLeading, restOnlyInImplementation, restOnlyInDeclaration];\n"
	write("harness.ts", harness)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = opened.Close() }()
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}

	for _, testCase := range []struct {
		name        string
		wantPremise []string
		wantRefusal string
		wantForms   []typefacts.UncensusedInvokingFormKind
	}{
		// The control that proves the premise path is live here at all.
		{"fixed", []string{"number", "number"}, "", nil},
		// The premise binds the rest parameter; the loop binding over it is
		// typed by the premise and proved by nothing, so each coercion of it
		// stands (the 2026-09-28 amendment to ADR 0149).
		{"rest", []string{"number[]"}, "", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
		{"restLeading", []string{"number", "number[]"}, "", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
		{"restOnlyInImplementation", nil, "implementation has a rest parameter",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
		{"restOnlyInDeclaration", nil, "implementation has a rest parameter",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			subjectStart := strings.Index(harness, "subjects = [") + len("subjects = [")
			offset := strings.Index(harness[subjectStart:], testCase.name)
			if offset < 0 {
				t.Fatalf("harness lists no %q", testCase.name)
			}
			implementationStart := strings.Index(runtime, "function "+testCase.name+"(") + len("function ")
			answer, err := analyzer.ExportValueTranscripts(
				context.Background(),
				[]typefacts.ExportValueDemand{{
					Location: typefacts.Location{
						Path:      filepath.Join(dir, "harness.ts"),
						StartByte: subjectStart + offset,
						EndByte:   subjectStart + offset + len(testCase.name),
					},
					ImplementationLocation: &typefacts.Location{
						Path:      filepath.Join(dir, "pkg", "index.js"),
						StartByte: implementationStart,
						EndByte:   implementationStart + len(testCase.name),
					},
				}},
			)
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
				t.Fatalf("transcripts = %#v, want one carrying an implementation", answer.Transcripts)
			}
			got := answer.Transcripts[0].Implementation
			if got.ParameterPremiseRefusal != testCase.wantRefusal {
				t.Errorf("refusal = %q, want %q", got.ParameterPremiseRefusal, testCase.wantRefusal)
			}
			premised := make([]string, 0, len(got.ParameterPremises))
			for _, premise := range got.ParameterPremises {
				premised = append(premised, premise.Type)
			}
			if !slices.Equal(premised, testCase.wantPremise) {
				t.Errorf("premises = %v, want %v", premised, testCase.wantPremise)
			}
			if forms := markerKinds(got.UncensusedInvokingForms); !slices.Equal(forms, testCase.wantForms) {
				t.Errorf("forms = %v, want %v", kindStrings(forms), kindStrings(testCase.wantForms))
			}
		})
	}
}
