package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// markerSource carries one exported function per UncensusedInvokingFormKind
// plus the negative controls that must produce no row at all. Everything the
// classifier decides from a *type* — an awaited `Promise` versus a
// `PromiseLike`, a coerced object versus a coerced string, a resolved data
// property versus an unresolvable member — needs both halves here, because a
// classifier that recorded every occurrence of a syntax would pass a
// positive-only test while saying nothing.
const markerSource = `export function taggedForm(tag: (parts: TemplateStringsArray) => string): string {
	return tag` + "`plain`" + `;
}

class Box {
	get value(): number {
		return 1;
	}
	set value(next: number) {
		void next;
	}
	readonly plain: number = 2;
}
const box = new Box();

export function getAccessorForm(): number {
	return box.value;
}

export function setAccessorForm(): void {
	box.value = 3;
}

export function plainMemberForm(): number {
	return box.plain;
}

export function unknownMemberForm(bag: any): unknown {
	return bag.whatever;
}

const mark = (target: unknown): void => {
	void target;
};

export function decoratorForm(): void {
	@mark
	class Decorated {}
	void Decorated;
}

export function spreadForm(values: Iterable<number>): number[] {
	return [...values];
}

export function forOfForm(values: Iterable<number>): number {
	let total = 0;
	for (const value of values) {
		total = total + value;
	}
	return total;
}

export function arrayPatternForm(values: Iterable<number>): number | undefined {
	const [first] = values;
	return first;
}

export function usingForm(resource: Disposable): void {
	using held = resource;
	void held;
}

export function instanceofForm(value: unknown): boolean {
	return value instanceof Error;
}

export async function awaitThenableForm(value: PromiseLike<number>): Promise<number> {
	return await value;
}

export async function awaitPromiseForm(value: Promise<number>): Promise<number> {
	return await value;
}

export function coercionForm(value: { toString(): string }): string {
	return ` + "`value: ${value}`" + `;
}

export function stringTemplateForm(value: string): string {
	return ` + "`value: ${value}`" + `;
}

export function objectSpreadForm(options: Record<string, unknown>): Record<string, unknown> {
	return { ...options };
}

export function capturedTaggedForm(
	tag: (parts: TemplateStringsArray) => string,
): () => string {
	return () => tag` + "`captured`" + `;
}

export function plainCallForm(callback: () => void): void {
	callback();
}

export function yieldStarForm(values: Iterable<number>): Generator<number> {
	function* inner(): Generator<number> {
		yield* values;
	}
	return inner();
}
`

// branchSource exercises the classifier arms markerSource does not reach: the
// two destructuring positions the compiler spells with the *same* node kinds it
// uses for values, the accessor asymmetries, element access with and without a
// statically known key, the coercion operators, the fail-closed `await`
// quantifier, and the premise that an accessor claim may only read
// declarations that are the bytes that run.
//
// Several exports below are deliberately TypeScript *errors* — writing a
// get-only accessor, reading a set-only one. They are here because the
// classifier has an arm for each, and because the checker still resolves the
// symbol: what the census answers about code `tsc` rejects is a fact about the
// producer, not a diagnostic this repository would ever report.
const branchSource = `class Widget {
	get value(): number {
		return 1;
	}
	set value(next: number) {
		void next;
	}
	get readOnly(): number {
		return 2;
	}
	set writeOnly(next: number) {
		void next;
	}
	readonly plain: number = 3;
}
const widget = new Widget();

interface Thenable {
	then(onFulfilled: (value: number) => void): void;
}

export function getterDestructureForm(): number {
	const { value } = widget;
	return value;
}

export function plainDestructureForm(): number {
	const { plain } = widget;
	return plain;
}

export function restDestructureForm(source: { first: number; rest: number }): unknown {
	const { first, ...rest } = source;
	void first;
	return rest;
}

export function computedDestructureForm(
	source: Record<string, number>,
	key: string,
): number | undefined {
	const { [key]: picked } = source;
	return picked;
}

export function elementAccessLiteralForm(): number {
	return widget["value"];
}

export function elementAccessComputedForm(key: "value"): number {
	return widget[key];
}

export function computedKeyForm(key: any): Record<string, number> {
	return { [key]: 1 };
}

export function getOnlyWrittenForm(): void {
	widget.readOnly = 4;
}

export function setOnlyReadForm(): unknown {
	return widget.writeOnly;
}

export function binaryCoercionForm(text: string, bag: { toString(): string }): string {
	return text + bag;
}

export function looseNullEqualityForm(bag: { valueOf(): number }): boolean {
	return bag != null;
}

export function looseEqualityCoercionForm(bag: { valueOf(): number }): boolean {
	return bag != 1;
}

export function unaryPrefixForm(bag: any): number {
	return -bag;
}

export function unaryPostfixForm(bag: { count: any }): void {
	bag.count++;
}

export function arrayAssignmentPatternForm(source: readonly number[]): number {
	let first = 0;
	let second = 0;
	[first, second] = source;
	return first + second;
}

export function singleArrayAssignmentPatternForm(source: Iterable<number>): number | undefined {
	let only: number | undefined;
	[only] = source;
	return only;
}

export function objectAssignmentPatternForm(): number {
	let value = 0;
	({ value } = widget);
	return value;
}

export function renamedObjectAssignmentPatternForm(): number {
	let local = 0;
	({ value: local } = widget);
	return local;
}

export function nestedObjectAssignmentPatternForm(source: { inner: Widget }): number {
	let value = 0;
	({ inner: { value } } = source);
	return value;
}

export function nestedArrayAssignmentPatternForm(source: { items: readonly number[] }): number {
	let first = 0;
	({ items: [first] } = source);
	return first;
}

export function objectValueForm(value: number): { value: number } {
	return { value };
}

export function propertyValueForm(value: number): { key: number } {
	return { key: value };
}

export function arrayValueForm(first: number, second: number): number[] {
	return [first, second];
}

export async function awaitUnionForm(value: Thenable | number): Promise<void> {
	void (await value);
}

export async function awaitTypeParameterForm<T>(value: T): Promise<void> {
	void (await value);
}

export async function awaitIndexSignatureForm(value: Record<string, unknown>): Promise<void> {
	void (await value);
}

export async function awaitPrimitiveForm(value: number): Promise<void> {
	void (await value);
}

export function declaredMemberForm(): number {
	return described.value;
}

export function standardLibraryMemberForm(values: readonly number[]): number {
	return values.length;
}

interface Countdown {
	[Symbol.iterator](): Iterator<number>;
}

export function iterateArrayForm(values: number[]): void {
	for (const value of values) {
		void value;
	}
}

export function iterateReadonlyArrayForm(values: readonly number[]): void {
	for (const value of values) {
		void value;
	}
}

export function iterateTupleForm(values: [number, string]): void {
	for (const value of values) {
		void value;
	}
}

export function iterateStringForm(text: string): void {
	for (const character of text) {
		void character;
	}
}

export function iterateSetForm(values: Set<number>): void {
	for (const value of values) {
		void value;
	}
}

export function iterateMapForm(values: Map<string, number>): void {
	for (const entry of values) {
		void entry;
	}
}

export function iterateTypedArrayForm(values: Uint8Array): void {
	for (const value of values) {
		void value;
	}
}

export function iterateUserIterableForm(values: Countdown): void {
	for (const value of values) {
		void value;
	}
}

export function iterateGeneratorForm(values: Generator<number>): void {
	for (const value of values) {
		void value;
	}
}

export function iterateUnionForm(values: number[] | Countdown): void {
	for (const value of values) {
		void value;
	}
}

export function iterateAnyForm(values: any): void {
	for (const value of values) {
		void value;
	}
}

export function iterateTypeParameterForm<T>(values: Iterable<T>): void {
	for (const value of values) {
		void value;
	}
}

export function iterateConstrainedTypeParameterForm<T extends number[]>(values: T): void {
	for (const value of values) {
		void value;
	}
}

export function spreadArrayForm(values: number[]): number[] {
	return [...values];
}

export function spreadStringForm(text: string): string[] {
	return [...text];
}

export function spreadArgumentForm(values: number[], sink: (...parts: number[]) => void): void {
	sink(...values);
}

export function arrayPatternArrayForm(values: number[]): number | undefined {
	const [first] = values;
	return first;
}

export function arrayPatternTupleForm(values: [number, string]): string {
	const [, second] = values;
	return second;
}

export function arrayPatternRestForm(values: number[]): number[] {
	const [, ...rest] = values;
	return rest;
}

export function yieldStarArrayForm(values: number[]): Generator<number> {
	function* inner(): Generator<number> {
		yield* values;
	}
	return inner();
}

export async function forAwaitArrayForm(values: number[]): Promise<void> {
	for await (const value of values) {
		void value;
	}
}

export async function forAwaitAsyncGeneratorForm(values: AsyncGenerator<number>): Promise<void> {
	for await (const value of values) {
		void value;
	}
}
`

// branchDeclarationSource is the whole point of declaredMemberForm: a
// hand-written declaration file describing a value whose runtime bytes this
// project never sees. `readonly value: number` is a *description*, and the
// thing it describes may perfectly well be a getter, so an accessor claim read
// off this declaration would be a claim about bytes nobody censused.
const branchDeclarationSource = `declare const described: { readonly value: number };
`

// withStatementSource is separate because `with` is the *only* node kind that
// can appear in a function body today and is neither classified into a named
// kind nor on the reviewed non-invoking list — that is what makes it the one
// available witness for the catch-all row. It is also a strict-mode error, so
// it cannot live in a checked-in fixture without manufacturing a TypeScript
// diagnostic there; see
// fixtures/package-contracts/uncensused-invoking-forms/README.md.
const withStatementSource = `export function withForm(bag: Record<string, unknown>): void {
	with (bag) {
		void 0;
	}
}
`

func markerProject(t *testing.T, files map[string]string) (typefacts.ExportValueAnalyzer, string) {
	t.Helper()
	dir := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(dir, "tsconfig.json"),
		[]byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","experimentalDecorators":true,"jsx":"preserve"},"include":["*.ts","*.tsx"]}`),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	for name, source := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
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

// implementationTranscriptFor asks for the transcript of the declaration the
// named identifier resolves to, through the ordinary export path: the demand's
// own location and its implementation location are the same identifier, which
// is what certification does for a declaration-rooted export.
func implementationTranscriptFor(
	t *testing.T,
	analyzer typefacts.ExportValueAnalyzer,
	path string,
	source string,
	name string,
) typefacts.ExportImplementationTranscript {
	t.Helper()
	start := strings.Index(source, name)
	if start < 0 {
		t.Fatalf("source does not contain %q", name)
	}
	location := typefacts.Location{Path: path, StartByte: start, EndByte: start + len(name)}
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &location}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
		t.Fatalf("transcripts for %q = %#v, want one carrying an implementation", name, answer.Transcripts)
	}
	return *answer.Transcripts[0].Implementation
}

func markerKinds(
	forms []typefacts.UncensusedInvokingForm,
) []typefacts.UncensusedInvokingFormKind {
	kinds := make([]typefacts.UncensusedInvokingFormKind, 0, len(forms))
	for _, form := range forms {
		kinds = append(kinds, form.Kind)
	}
	return kinds
}

func TestUncensusedInvokingFormsNameEveryClassifiedKind(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"markers.ts": markerSource})
	path := filepath.Join(dir, "markers.ts")

	for _, testCase := range []struct {
		export string
		want   []typefacts.UncensusedInvokingFormKind
	}{
		{"taggedForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedTaggedTemplate}},
		{"getAccessorForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor}},
		{"setAccessorForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedSetAccessor}},
		{"decoratorForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedDecorator}},
		{"spreadForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol}},
		{"forOfForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol}},
		{"arrayPatternForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol}},
		{"usingForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedUsingDispose}},
		{"instanceofForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedInstanceOf}},
		{"awaitThenableForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedAwaitThen}},
		{"coercionForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
		{
			"unknownMemberForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedPropertyAccessUnknownAccessor},
		},
		{
			"objectSpreadForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedPropertyAccessUnknownAccessor},
		},
		{
			"yieldStarForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol},
		},
		// The negative controls. Each contains the *syntax* of a classified
		// form and must still produce nothing, because the classification is
		// decided by the type rather than the spelling.
		{"plainMemberForm", nil},
		{"awaitPromiseForm", nil},
		{"stringTemplateForm", nil},
		{"plainCallForm", nil},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, markerSource, testCase.export)
		got := markerKinds(transcript.UncensusedInvokingForms)
		if len(got) != len(testCase.want) {
			t.Fatalf(
				"%s uncensused forms = %v, want %v",
				testCase.export, got, testCase.want,
			)
		}
		for index, kind := range testCase.want {
			if got[index] != kind {
				t.Fatalf(
					"%s uncensused form %d = %q, want %q (all: %v)",
					testCase.export, index, got[index], kind, got,
				)
			}
		}
		for _, form := range transcript.UncensusedInvokingForms {
			if form.NodeKind == "" {
				t.Fatalf("%s form %#v names no node kind", testCase.export, form)
			}
			if form.Captured != (form.EnclosingCallable != nil) {
				t.Fatalf("%s form %#v disagrees with itself about capture", testCase.export, form)
			}
		}
	}
}

func TestUncensusedInvokingFormClassifierBranches(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{
		"branches.ts":    branchSource,
		"described.d.ts": branchDeclarationSource,
	})
	path := filepath.Join(dir, "branches.ts")

	for _, testCase := range []struct {
		export string
		want   []typefacts.UncensusedInvokingFormKind
		why    string
	}{
		{
			export: "getterDestructureForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "an object-pattern binding element reads the property, and this one is a getter",
		},
		{
			export: "plainDestructureForm",
			want:   nil,
			why:    "the same syntax over a resolved data property invokes nothing",
		},
		{
			export: "restDestructureForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedPropertyAccessUnknownAccessor,
			},
			why: "a rest element reads every remaining own enumerable property and names none of them — its own identifier is the new object, not a key, even when the source happens to carry a property of that name",
		},
		{
			export: "computedDestructureForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedPropertyAccessUnknownAccessor,
			},
			why: "a computed binding key is not statically a property, and the string-typed key expression coerces nothing",
		},
		{
			export: "elementAccessLiteralForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "an element access with an exact literal key resolves the member like a property access",
		},
		{
			export: "elementAccessComputedForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedPropertyAccessUnknownAccessor,
			},
			why: "the checker resolves no member for a non-literal key, and a literal *type* is not a literal key — refusing is the fail-closed direction",
		},
		{
			export: "computedKeyForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion},
			why:    "a computed property key is coerced to a property key, which reaches Symbol.toPrimitive/toString on an object-typed key",
		},
		{
			export: "getOnlyWrittenForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "a get-only accessor written to refuses under the accessor that does resolve rather than modelling the asymmetry",
		},
		{
			export: "setOnlyReadForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedSetAccessor},
			why:    "and symmetrically for a set-only accessor read",
		},
		{
			export: "binaryCoercionForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion},
			why:    "`+` applies ToPrimitive to both operands; the object-typed one is enough",
		},
		{
			export: "looseNullEqualityForm",
			want:   nil,
			why:    "loose equality against an exact null literal never applies ToPrimitive to the other operand",
		},
		{
			export: "looseEqualityCoercionForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion},
			why:    "other loose equalities can apply ToPrimitive to an object operand",
		},
		{
			export: "unaryPrefixForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion},
			why:    "unary minus coerces, and `any` is not provably a non-object",
		},
		{
			export: "unaryPostfixForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion},
			why:    "`x++` coerces its operand exactly as `x + 1` does",
		},
		{
			export: "arrayAssignmentPatternForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "`[a, b] = src` is an assignment pattern: it drives src's Symbol.iterator and that iterator's next. `=` is not a coercing operator, so nothing else in the classifier would see it",
		},
		{
			export: "singleArrayAssignmentPatternForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "one element drives the protocol exactly as two do",
		},
		{
			export: "objectAssignmentPatternForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "`({ a } = src)` performs a Get on src for each member, which reaches that property's getter",
		},
		{
			export: "renamedObjectAssignmentPatternForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "the renaming form reads the same property",
		},
		{
			export: "nestedObjectAssignmentPatternForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor},
			why:    "a nested object pattern resolves its member through the enclosing pattern's source type, not through the pattern's own shape",
		},
		{
			export: "nestedArrayAssignmentPatternForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "an array pattern nested in an object pattern is still an assignment target",
		},
		// The value-position controls, and the reason the four kinds above are
		// classified by position rather than by kind: the same node kinds in a
		// value position invoke nothing at all.
		{export: "objectValueForm", want: nil, why: "`{ value }` builds an object and reads no other value"},
		{export: "propertyValueForm", want: nil, why: "and neither does `{ key: value }`"},
		{export: "arrayValueForm", want: nil, why: "nor `[first, second]`"},
		{
			export: "awaitUnionForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedAwaitThen},
			why:    "one constituent carries a user-owned `then`; that the other is a number proves nothing about this await",
		},
		{
			export: "awaitTypeParameterForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedAwaitThen},
			why:    "an unconstrained type parameter has no members the checker can enumerate, which is not the same as having no `then`",
		},
		{
			export: "awaitIndexSignatureForm",
			want:   []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedAwaitThen},
			why:    "an index-signature type declares no `then` while permitting one at runtime, whose Get would reach a getter and whose value await would call",
		},
		{
			export: "awaitPrimitiveForm",
			want:   nil,
			why:    "a primitive has no `then` to reach, and await on one resolves immediately",
		},
		{
			export: "declaredMemberForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedPropertyAccessUnknownAccessor,
			},
			why: "the premise \"no accessor declaration, therefore no invocation\" may only read declarations that are the bytes that run; `readonly value: number` in a .d.ts may describe a getter",
		},
		{
			export: "standardLibraryMemberForm",
			want:   nil,
			why:    "the default library is the one admissible exception: lib.d.ts describes the engine, which is not user code, so no lib-declared member can reach a user callable",
		},
		// The iteration protocol, whose classification is decided by the
		// operand's type the way `await`'s is. What the reviewed container
		// table claims is narrow and has two halves: the `[Symbol.iterator]`
		// named by the declaration is the engine's factory, *and* the value is
		// an object the engine created, so the iterator that factory returns —
		// and its `next` and `return` — is engine code too. Everything the
		// table does not name stays recorded.
		{
			export: "iterateArrayForm",
			want:   nil,
			why:    "Array's `[Symbol.iterator]` is the engine's, and the array iterator it returns is the engine's %ArrayIteratorPrototype% — nothing in the loop's own iteration reaches user code",
		},
		{
			export: "iterateReadonlyArrayForm",
			want:   nil,
			why:    "`readonly number[]` reaches the same method through ReadonlyArray",
		},
		{
			export: "iterateTupleForm",
			want:   nil,
			why:    "a tuple resolves the member through its Array base, so a fixed-slot type answers like the array it is",
		},
		{
			export: "iterateStringForm",
			want:   nil,
			why:    "a primitive string is not skipped as a non-object the way a coerced primitive is — it is iterable, and it clears because the member resolves through String's apparent type",
		},
		{export: "iterateSetForm", want: nil, why: "Set is a reviewed engine container"},
		{
			export: "iterateMapForm",
			want:   nil,
			why:    "and so is Map; the entry pair it yields is destructured nowhere here, so the loop adds no second row",
		},
		{
			export: "iterateTypedArrayForm",
			want:   nil,
			why:    "the typed arrays declare their own identically-shaped iteration in lib.es2015.iterable.d.ts and are reviewed here — unlike defaultLibraryMemberInvokers' array-iteration row, whose claim is about a callback slot rather than about the protocol",
		},
		{
			export: "iterateUserIterableForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "a user-declared `[Symbol.iterator]` is a user callable, and its declaration is not in a default-library file",
		},
		{
			export: "iterateGeneratorForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "the sharpest case in the table, and the one that proves the container allowlist does work rather than the file check alone: Generator declares `[Symbol.iterator]` *in* lib.es2015.generator.d.ts, and a generator's `next` runs a user function body",
		},
		{
			export: "iterateUnionForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "the quantifier is per constituent: that one of them is an array proves nothing about this iteration",
		},
		{
			export: "iterateAnyForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "`any` enumerates no members, and \"the checker could not find `[Symbol.iterator]`\" must never read as \"iterating this reaches no user code\"",
		},
		{
			export: "iterateTypeParameterForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "an `Iterable<T>` constraint is the structural protocol, so the factory named by the declaration is not the factory that runs",
		},
		{
			export: "iterateConstrainedTypeParameterForm",
			want:   nil,
			why:    "a type parameter constrained to an array clears through its constraint's apparent type, exactly as `await value` does when `T extends Promise<number>` — the same declaration-versus-runtime limit, recorded rather than closed",
		},
		{export: "spreadArrayForm", want: nil, why: "array spread drives the same protocol as for-of"},
		{export: "spreadStringForm", want: nil, why: "and so does spreading a string"},
		{
			export: "spreadArgumentForm",
			want:   nil,
			why:    "a spread argument is the same node kind in a call position; `sink(...)` itself is the call census's row, not this census's",
		},
		{
			export: "arrayPatternArrayForm",
			want:   nil,
			why:    "a binding pattern has no operand expression, so the iterated value comes from the declaration's own type — which is the initializer's",
		},
		{export: "arrayPatternTupleForm", want: nil, why: "an elision iterates no differently"},
		{
			export: "arrayPatternRestForm",
			want:   nil,
			why:    "an array rest element drains the same engine iterator; unlike an *object* rest element it names no property, so nothing here reads an unknown accessor",
		},
		{
			export: "yieldStarArrayForm",
			want:   nil,
			why:    "`yield*` drives the operand's iterator, so it narrows on the operand's type like for-of; that the enclosing generator's own body is user code is the transcript's question, not this row's",
		},
		{
			export: "forAwaitArrayForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "`for await…of` is the one arm that asks no type question: it resolves `Symbol.asyncIterator` first — declared in the default library only by structural contracts whose `next` is a user body — and falls back to the sync protocol while awaiting each result, invoking whatever `then` those values carry",
		},
		{
			export: "forAwaitAsyncGeneratorForm",
			want: []typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
			},
			why: "and an async generator's `next` is a user function body",
		},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, branchSource, testCase.export)
		got := markerKinds(transcript.UncensusedInvokingForms)
		if len(got) != len(testCase.want) {
			t.Errorf(
				"%s uncensused forms = %v, want %v (%s)",
				testCase.export, got, testCase.want, testCase.why,
			)
			continue
		}
		for index, kind := range testCase.want {
			if got[index] != kind {
				t.Errorf(
					"%s uncensused form %d = %q, want %q (all: %v; %s)",
					testCase.export, index, got[index], kind, got, testCase.why,
				)
			}
		}
	}
}

// A form inside a nested callable carries the innermost callable that contains
// it and Captured, exactly as the call census does. Lexical containment in a
// returned closure is not execution, and a consumer that could not tell the
// two apart would read a tagged template the export merely hands back as one
// the export performs.
func TestUncensusedInvokingFormInsideNestedCallableIsCaptured(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"markers.ts": markerSource})
	path := filepath.Join(dir, "markers.ts")
	transcript := implementationTranscriptFor(t, analyzer, path, markerSource, "capturedTaggedForm")
	if len(transcript.UncensusedInvokingForms) != 1 {
		t.Fatalf("uncensused forms = %#v, want exactly the captured tagged template", transcript.UncensusedInvokingForms)
	}
	form := transcript.UncensusedInvokingForms[0]
	if form.Kind != typefacts.UncensusedTaggedTemplate {
		t.Fatalf("form kind = %q, want tagged-template", form.Kind)
	}
	if !form.Captured || form.EnclosingCallable == nil {
		t.Fatalf("form = %#v, want captured with an enclosing callable", form)
	}
	if form.EnclosingCallable.StartByte >= form.Location.StartByte ||
		form.EnclosingCallable.EndByte < form.Location.EndByte {
		t.Fatalf("enclosing callable %#v does not contain the form at %#v", form.EnclosingCallable, form.Location)
	}
	// The nested arrow, not the export's own declaration.
	if form.EnclosingCallable.StartByte == transcript.Location.StartByte {
		t.Fatalf("enclosing callable = %#v, want the nested arrow rather than the implementation", form.EnclosingCallable)
	}
}

// The load-bearing row: a node kind the classifier neither names nor clears
// must arrive as unclassified-invoking-form carrying the compiler's own kind
// name, so that a form nobody has thought of refuses instead of passing.
func TestUnclassifiedInvokingFormRefusesByNodeKind(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"with.ts": withStatementSource})
	path := filepath.Join(dir, "with.ts")
	transcript := implementationTranscriptFor(t, analyzer, path, withStatementSource, "withForm")
	found := false
	for _, form := range transcript.UncensusedInvokingForms {
		if form.Kind != typefacts.UncensusedUnclassifiedInvokingForm {
			continue
		}
		found = true
		if form.NodeKind != "WithStatement" {
			t.Fatalf("unclassified form node kind = %q, want WithStatement", form.NodeKind)
		}
	}
	if !found {
		t.Fatalf("uncensused forms = %#v, want an unclassified row", transcript.UncensusedInvokingForms)
	}
}

const jsxMarkerSource = `declare global {
	namespace JSX {
		interface IntrinsicElements {
			div: Record<string, unknown>;
		}
		interface Element {
			readonly brand: unique symbol;
		}
	}
}

export function jsxForm(): JSX.Element {
	return <div />;
}
`

func TestJsxElementIsAnUncensusedInvokingForm(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"jsx.tsx": jsxMarkerSource})
	path := filepath.Join(dir, "jsx.tsx")
	transcript := implementationTranscriptFor(t, analyzer, path, jsxMarkerSource, "jsxForm")
	found := false
	for _, form := range transcript.UncensusedInvokingForms {
		if form.Kind == typefacts.UncensusedJSXElement {
			found = true
			if form.NodeKind != "JsxSelfClosingElement" {
				t.Fatalf("jsx form node kind = %q, want JsxSelfClosingElement", form.NodeKind)
			}
		}
	}
	if !found {
		t.Fatalf("uncensused forms = %#v, want a jsx-element row", transcript.UncensusedInvokingForms)
	}
}

const localHelperSource = `function localHelper(callback: () => void): void {
	callback();
}

function otherHelper(): void {
	void 0;
}

export function entry(callback: () => void): void {
	localHelper(callback);
}
`

// A census recurses into module-local helpers, and the export path cannot
// reach one: it starts from an identifier and resolves through the export's
// runtime binding, which a helper nothing exports does not have. The demand
// therefore names the declaration by its exact source range.
func TestLocalDeclarationTranscriptAnswersANonExportedHelper(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"local.ts": localHelperSource})
	path := filepath.Join(dir, "local.ts")
	entryStart := strings.Index(localHelperSource, "entry")
	entry := typefacts.Location{
		Path:      path,
		StartByte: entryStart,
		EndByte:   entryStart + len("entry"),
	}

	helperStart := strings.Index(localHelperSource, "function localHelper")
	helperEnd := strings.Index(localHelperSource, "}\n\nfunction otherHelper") + 1
	helper := typefacts.Location{Path: path, StartByte: helperStart, EndByte: helperEnd}

	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{Location: entry, LocalDeclarationLocation: &helper}},
	)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0].LocalDeclaration
	if transcript == nil {
		t.Fatal("demand asked for a local declaration and got none")
	}
	// Identity: the transcript states which bytes it describes, and they are
	// the demanded ones. Without this a transcript about otherHelper would be
	// indistinguishable from one about localHelper.
	if transcript.Location != helper {
		t.Fatalf("local declaration location = %#v, want the demanded %#v", transcript.Location, helper)
	}
	if !transcript.Complete || len(transcript.OpenReasons) != 0 {
		t.Fatalf("local declaration transcript = %#v, want complete", transcript)
	}
	if transcript.QueryName != "localHelper" || transcript.Target == "" || transcript.Declaration == nil {
		t.Fatalf("local declaration identity = %#v, want localHelper resolved", transcript)
	}
	// The binding that is *not* an echo. Location above is the demand copied
	// back, so it says nothing on its own; the resolved declaration's location
	// comes from the checker, and it must name the demanded file and sit inside
	// the demanded span. For a named function that is its identifier, which is
	// inside the declaration rather than equal to it.
	if transcript.Declaration.Location.Path != helper.Path ||
		transcript.Declaration.Location.StartByte < helper.StartByte ||
		transcript.Declaration.Location.EndByte > helper.EndByte {
		t.Fatalf(
			"resolved declaration at %#v is not inside the demanded %#v",
			transcript.Declaration.Location, helper,
		)
	}
	if transcript.Declaration.Name != transcript.QueryName {
		t.Fatalf(
			"resolved declaration name = %q, queried %q",
			transcript.Declaration.Name, transcript.QueryName,
		)
	}
	if transcript.Signature == nil {
		t.Fatalf("local declaration transcript = %#v, want a selected signature", transcript)
	}
	if len(transcript.Calls) != 1 || transcript.Calls[0].CalleeParameter == nil ||
		transcript.Calls[0].CalleeParameter.ParameterIndex != 0 {
		t.Fatalf("local declaration calls = %#v, want the parameter-0 call", transcript.Calls)
	}
	if len(transcript.UncensusedInvokingForms) != 0 {
		t.Fatalf(
			"local declaration uncensused forms = %#v, want none for a plain call",
			transcript.UncensusedInvokingForms,
		)
	}
}

const arrowHelperSource = `const localArrow = (callback: () => void): void => {
	callback();
};

export function entry(callback: () => void): void {
	localArrow(callback);
}
`

// A local declaration demand at an arrow's exact span answers through the
// declarator that holds it: the name and symbol come from the enclosing
// variable declaration, and the declaration identity binds because an anonymous
// callable resolves to the arrow itself, which is the demanded span. This is the
// producer half of the certifier's initializer-binding reading.
func TestLocalDeclarationTranscriptAnswersAnArrowBoundToAConst(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"local.ts": arrowHelperSource})
	path := filepath.Join(dir, "local.ts")
	entryStart := strings.Index(arrowHelperSource, "entry")
	entry := typefacts.Location{Path: path, StartByte: entryStart, EndByte: entryStart + len("entry")}
	arrowStart := strings.Index(arrowHelperSource, "(callback: () => void): void =>")
	arrowEnd := strings.Index(arrowHelperSource, "};\n") + 1
	arrow := typefacts.Location{Path: path, StartByte: arrowStart, EndByte: arrowEnd}

	// The implementation transcript of `entry` is demanded in the same request
	// and answered first: its `localArrow(callback)` call resolves the symbol
	// through the *declarator*, to the identifier. The local-declaration answer
	// that follows must still be the arrow — a cache keyed by the symbol alone
	// handed back the identifier here, and the client refused it.
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{
			Location:                 entry,
			ImplementationLocation:   &entry,
			LocalDeclarationLocation: &arrow,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	implementation := answer.Transcripts[0].Implementation
	if implementation == nil || len(implementation.Calls) != 1 || implementation.Calls[0].Declaration == nil {
		t.Fatalf("expected entry's one resolved call, got %+v", implementation)
	}
	if got := implementation.Calls[0].Declaration.Location; got.StartByte != strings.Index(arrowHelperSource, "localArrow") {
		t.Fatalf("expected the call to resolve to the declarator's identifier, got %+v", got)
	}
	transcript := answer.Transcripts[0].LocalDeclaration
	if transcript == nil {
		t.Fatal("expected a local declaration transcript")
	}
	if len(transcript.OpenReasons) != 0 {
		t.Fatalf("expected a closed transcript, got open reasons %v", transcript.OpenReasons)
	}
	if transcript.QueryName != "localArrow" {
		t.Fatalf("expected the declarator's name, got %q", transcript.QueryName)
	}
	if transcript.Declaration == nil || transcript.Declaration.Name != "localArrow" {
		t.Fatalf("expected the declaration to resolve to localArrow, got %+v", transcript.Declaration)
	}
	if transcript.Declaration.Location.StartByte != arrowStart || transcript.Declaration.Location.EndByte != arrowEnd {
		t.Fatalf("expected the resolved declaration to be the arrow itself, got %+v", transcript.Declaration.Location)
	}
	if len(transcript.Calls) != 1 {
		t.Fatalf("expected the arrow's one call, got %d", len(transcript.Calls))
	}
}

func TestLocalDeclarationTranscriptRefusesAWrongLocation(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"local.ts": localHelperSource})
	path := filepath.Join(dir, "local.ts")
	entryStart := strings.Index(localHelperSource, "entry")
	entry := typefacts.Location{
		Path:      path,
		StartByte: entryStart,
		EndByte:   entryStart + len("entry"),
	}

	nameStart := strings.Index(localHelperSource, "localHelper")
	for _, testCase := range []struct {
		name     string
		location typefacts.Location
		reason   string
	}{
		{
			// The *identifier* is inside the declaration but is not the
			// declaration. Containment is not a match; the span must be exact.
			name:     "identifier rather than declaration",
			location: typefacts.Location{Path: path, StartByte: nameStart, EndByte: nameStart + len("localHelper")},
			reason:   "declarationNotExact",
		},
		{
			name:     "span off by one byte",
			location: typefacts.Location{Path: path, StartByte: 0, EndByte: 1},
			reason:   "declarationNotExact",
		},
		{
			name:     "outside the snapshot",
			location: typefacts.Location{Path: filepath.Join(dir, "absent.ts"), StartByte: 0, EndByte: 4},
			reason:   "sourceUnavailable",
		},
	} {
		location := testCase.location
		answer, err := analyzer.ExportValueTranscripts(
			context.Background(),
			[]typefacts.ExportValueDemand{{Location: entry, LocalDeclarationLocation: &location}},
		)
		if err != nil {
			t.Fatal(err)
		}
		transcript := answer.Transcripts[0].LocalDeclaration
		if transcript == nil {
			t.Fatalf("%s: demand asked for a local declaration and got none", testCase.name)
		}
		if transcript.Complete {
			t.Fatalf("%s: transcript = %#v, want a refusal", testCase.name, transcript)
		}
		if transcript.Location != location {
			t.Fatalf(
				"%s: refused transcript location = %#v, want the demanded %#v",
				testCase.name, transcript.Location, location,
			)
		}
		if !stringListContains(transcript.OpenReasons, testCase.reason) {
			t.Fatalf(
				"%s: open reasons = %v, want %q",
				testCase.name, transcript.OpenReasons, testCase.reason,
			)
		}
	}
}

// A declaration file is a *program* file, so it clears sourceFileFor — and it
// is not snapshot source, because it has no runtime bytes to census. Every
// `lib.*.d.ts` and every dependency `.d.ts` is in the program on every run, so
// without the separate check a demand naming one of them would be answered
// with `declarationNotExact` (or, for an ambient declaration whose span
// matched, `implementationUnavailable`) — reasons that describe the *span*,
// when the honest refusal is about the file.
func TestLocalDeclarationTranscriptRefusesADeclarationFile(t *testing.T) {
	const describedSource = `export declare function described(): void;
`
	analyzer, dir := markerProject(t, map[string]string{
		"local.ts":       localHelperSource,
		"described.d.ts": describedSource,
	})
	entryStart := strings.Index(localHelperSource, "entry")
	entry := typefacts.Location{
		Path:      filepath.Join(dir, "local.ts"),
		StartByte: entryStart,
		EndByte:   entryStart + len("entry"),
	}
	declared := typefacts.Location{
		Path:      filepath.Join(dir, "described.d.ts"),
		StartByte: 0,
		EndByte:   len(describedSource) - 1,
	}

	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{Location: entry, LocalDeclarationLocation: &declared}},
	)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0].LocalDeclaration
	if transcript == nil {
		t.Fatal("demand asked for a local declaration and got none")
	}
	if transcript.Complete {
		t.Fatalf("transcript = %#v, want a refusal", transcript)
	}
	if !stringListContains(transcript.OpenReasons, "declarationOutsideSnapshot") {
		t.Fatalf(
			"open reasons = %v, want declarationOutsideSnapshot",
			transcript.OpenReasons,
		)
	}
}

// The contract fixture's own published bytes, classified.
//
// fixtures/package-contracts/uncensused-invoking-forms exists to run the
// classifier over a real published ES module through ordinary contract
// generation, but `uncensusedInvokingForms` is a *transcript* field and not a
// contract field, so that fixture's generated `expected.json` and
// `expected-proposal.json` carry no marker rows at all. Left there, the
// fixture would pin only that the classifier does not crash — its README table
// of "export → marker kind" would be prose nothing checks. This test is what
// makes that table load-bearing: it reads the fixture's `index.js` off disk and
// pins the kinds, so editing the fixture moves a test rather than only a
// comment.
//
// The project here is a plain `allowJs` program rather than the generator's,
// because what is being pinned is the classification of these bytes; the
// generator's own consumption of the fixture is pinned by the contract corpus.
func TestUncensusedInvokingFormsClassifyThePublishedFixtureBytes(t *testing.T) {
	fixture := filepath.Join(
		"..", "..", "..", "..", "..",
		"fixtures", "package-contracts", "uncensused-invoking-forms", "index.js",
	)
	published, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	source := string(published)

	dir := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(dir, "tsconfig.json"),
		[]byte(`{"compilerOptions":{"allowJs":true,"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.js"]}`),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "index.js"), published, 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = opened.Close() })
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}
	path := filepath.Join(dir, "index.js")

	for _, testCase := range []struct {
		export string
		want   []typefacts.UncensusedInvokingFormKind
	}{
		{"taggedForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedTaggedTemplate}},
		{"getAccessorForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedGetAccessor}},
		{"setAccessorForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedSetAccessor}},
		{
			"unknownMemberForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedPropertyAccessUnknownAccessor},
		},
		{
			"objectSpreadForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedPropertyAccessUnknownAccessor},
		},
		{"spreadForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedIterationProtocol}},
		// Two rows, and the second is the one the README's table does not
		// promise: the published `.js` annotates nothing, so `total + value`
		// adds an operand that is not provably a non-object. It is the same
		// fail-closed direction the `.ts` marker source pins for `any`.
		{
			"forOfForm",
			[]typefacts.UncensusedInvokingFormKind{
				typefacts.UncensusedIterationProtocol,
				typefacts.UncensusedCoercion,
			},
		},
		{"instanceofForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedInstanceOf}},
		{"awaitThenableForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedAwaitThen}},
		{"coercionForm", []typefacts.UncensusedInvokingFormKind{typefacts.UncensusedCoercion}},
		{
			"capturedTaggedForm",
			[]typefacts.UncensusedInvokingFormKind{typefacts.UncensusedTaggedTemplate},
		},
		// The controls. `box.plain` resolves, in the *runtime* bytes, to the
		// constructor's own `this.plain = 2` — which is why this fixture's
		// silence is a claim about code and not about a declaration file.
		{"plainMemberForm", nil},
		{"plainCallForm", nil},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, testCase.export)
		got := markerKinds(transcript.UncensusedInvokingForms)
		if len(got) != len(testCase.want) {
			t.Errorf("%s uncensused forms = %v, want %v", testCase.export, got, testCase.want)
			continue
		}
		for index, kind := range testCase.want {
			if got[index] != kind {
				t.Errorf(
					"%s uncensused form %d = %q, want %q (all: %v)",
					testCase.export, index, got[index], kind, got,
				)
			}
		}
	}
}

// The demand digest covers the local-declaration location, so two demands that
// differ only in which helper they name are two different demands. Without
// this a producer answer for one could be replayed as the answer for the
// other under the same envelope.
func TestExportValueDemandDigestSeparatesLocalDeclarationLocations(t *testing.T) {
	base := typefacts.Location{Path: "/p/a.ts", StartByte: 1, EndByte: 6}
	first := typefacts.Location{Path: "/p/a.ts", StartByte: 10, EndByte: 40}
	second := typefacts.Location{Path: "/p/a.ts", StartByte: 50, EndByte: 80}
	none := exportValueDemandDigest([]typefacts.ExportValueDemand{{Location: base}})
	one := exportValueDemandDigest(
		[]typefacts.ExportValueDemand{{Location: base, LocalDeclarationLocation: &first}},
	)
	two := exportValueDemandDigest(
		[]typefacts.ExportValueDemand{{Location: base, LocalDeclarationLocation: &second}},
	)
	if none == one || one == two {
		t.Fatalf("digests collide: none=%s one=%s two=%s", none, one, two)
	}
}

// subjectSource pins ADR 0034's subject-parameter premise, positive and negative:
// only a read accessor whose receiver chain roots at a plain, uninitialized,
// non-rest parameter that is written nowhere — in a declaration mentioning
// neither `arguments` nor `eval` — states a subject parameter.
const subjectSource = `const registry: any = { value: 1, inner: [{ value: 2 }] };

export function parameterRead(source: any): unknown {
	return source.value;
}

export function parameterChainRead(source: any): unknown {
	return source.inner[0].value;
}

export function writtenParameter(source: any): unknown {
	source = registry;
	return source.value;
}

export function writtenAfterRead(source: any): unknown {
	const seen = source.value;
	source = registry;
	return seen;
}

export function moduleRead(): unknown {
	return registry.value;
}

export function nestedParameterRead(items: any[]): unknown[] {
	return items.map((item: any) => item.value);
}

export function defaultedParameter(source: any = registry): unknown {
	return source.value;
}

export function destructuredParameter({ inner }: any): unknown {
	return inner.value;
}

export function setterOnParameter(source: any): void {
	source.value = 1;
}

export function deletedOnParameter(source: any): void {
	delete source.value;
}

export function argumentsMention(source: any): unknown {
	void arguments.length;
	return source.value;
}

export function toStringTagViaCall(value: unknown): boolean {
	return Object.prototype.toString.call(value) === "[object String]";
}

export function sliceViaCall(value: unknown): unknown {
	return Array.prototype.slice.call(value);
}

function helper(this: unknown): unknown {
	return this;
}

export function localViaCall(value: unknown): unknown {
	return helper.call(value);
}
`

func TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"subjects.ts": subjectSource})
	path := filepath.Join(dir, "subjects.ts")
	zero := 0
	for _, testCase := range []struct {
		export string
		want   []*int
	}{
		{"parameterRead", []*int{&zero}},
		{"parameterChainRead", []*int{&zero, &zero, &zero}},
		{"writtenParameter", []*int{nil}},
		{"writtenAfterRead", []*int{nil}},
		{"moduleRead", []*int{nil}},
		{"nestedParameterRead", []*int{nil}},
		{"defaultedParameter", []*int{nil}},
		{"destructuredParameter", []*int{nil}},
		{"setterOnParameter", []*int{nil}},
		{"deletedOnParameter", []*int{nil}},
		{"argumentsMention", []*int{nil}},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, subjectSource, testCase.export)
		forms := transcript.UncensusedInvokingForms
		if len(forms) != len(testCase.want) {
			t.Fatalf("%s: %d forms (%v), want %d", testCase.export, len(forms), markerKinds(forms), len(testCase.want))
		}
		for index, want := range testCase.want {
			got := forms[index].SubjectParameter
			switch {
			case want == nil && got != nil:
				t.Fatalf("%s form %d (%s) states subject parameter %d, want none", testCase.export, index, forms[index].Kind, *got)
			case want != nil && (got == nil || *got != *want):
				t.Fatalf("%s form %d (%s) subject parameter = %v, want %d", testCase.export, index, forms[index].Kind, got, *want)
			}
		}
	}
}

func TestCallAndApplyStateTheirReceiverAndThisParameter(t *testing.T) {
	analyzer, dir := markerProject(t, map[string]string{"subjects.ts": subjectSource})
	path := filepath.Join(dir, "subjects.ts")
	for _, testCase := range []struct {
		export        string
		receiver      string
		library       bool
		thisParameter bool
	}{
		{"toStringTagViaCall", "Object.toString", true, true},
		{"sliceViaCall", "Array.slice", true, true},
		{"localViaCall", "helper", false, true},
	} {
		transcript := implementationTranscriptFor(t, analyzer, path, subjectSource, testCase.export)
		var found *typefacts.ImplementationCall
		for index := range transcript.Calls {
			if transcript.Calls[index].Declaration != nil && transcript.Calls[index].Declaration.Name == "call" {
				found = &transcript.Calls[index]
			}
		}
		if found == nil {
			t.Fatalf("%s: no `.call` row among %d calls", testCase.export, len(transcript.Calls))
		}
		if found.CallReceiver == nil {
			t.Fatalf("%s: the `.call` row states no receiver", testCase.export)
		}
		if found.CallReceiver.QualifiedName != testCase.receiver || found.CallReceiver.StandardLibrary != testCase.library {
			t.Fatalf(
				"%s: receiver = %q (library %v), want %q (library %v)",
				testCase.export, found.CallReceiver.QualifiedName, found.CallReceiver.StandardLibrary,
				testCase.receiver, testCase.library,
			)
		}
		if (found.ThisParameter != nil) != testCase.thisParameter || (found.ThisParameter != nil && *found.ThisParameter != 0) {
			t.Fatalf("%s: this parameter = %v, want stated=%v index 0", testCase.export, found.ThisParameter, testCase.thisParameter)
		}
		for _, form := range transcript.UncensusedInvokingForms {
			t.Fatalf("%s: unexpected uncensused form %s at %d", testCase.export, form.Kind, form.Location.StartByte)
		}
	}
}
