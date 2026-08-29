package tsgo

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestInvocationTranscriptSelectsGenericSignatureAndExpandsExactTupleSpread(t *testing.T) {
	dir := t.TempDir()
	source := `
type Options = {
  mode: "read" | "write";
  nested?: { callback: (value: number) => void };
};

function execute<T extends Options>(options: T, ...steps: [boolean, (value: number) => void]): Promise<T> {
  options.nested?.callback(1);
  steps[1](1);
  return Promise.resolve(options);
}

func TestInvocationDemandDigestBindsOrderAndEveryDemandOption(t *testing.T) {
	demands := []typefacts.InvocationDemand{
		{Location: typefacts.Location{Path: "/p/a.ts", StartByte: 4, EndByte: 12}, CallableDepth: 3, Census: true},
		{Location: typefacts.Location{Path: "/p/b.ts", StartByte: 0, EndByte: 7}},
	}
	const expected = "sha256:68a28ae5071bb694387c8d372c3a3febd48636f783d4a5881ece0a15656fb88c"
	if got := invocationDemandDigest(demands); got != expected {
		t.Fatalf("invocation demand digest = %q, want cross-language fixture %q", got, expected)
	}
	demands[0], demands[1] = demands[1], demands[0]
	if got := invocationDemandDigest(demands); got == expected {
		t.Fatal("invocation demand digest ignored demand order")
	}
}

const callback = (value: number) => void value;
const steps = [true, callback] as const;
execute({ mode: "read", nested: { callback } }, ...steps);
`
	configPath := filepath.Join(dir, "tsconfig.json")
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(configPath, []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), configPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer, ok := opened.(typefacts.InvocationAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement InvocationAnalyzer")
	}
	needle := `execute({ mode: "read", nested: { callback } }, ...steps)`
	start := strings.LastIndex(source, needle)
	answer, err := analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location:      typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
		CallableDepth: 3,
		Census:        true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 {
		t.Fatalf("transcripts = %d, want 1", len(answer.Transcripts))
	}
	transcript := answer.Transcripts[0]
	if transcript.SelectedSignature == nil || transcript.SelectedSignature.Identity == "" {
		t.Fatalf("selected signature = %#v, want durable identity", transcript.SelectedSignature)
	}
	if got := transcript.SelectedSignature.OverloadCount; got != 1 {
		t.Fatalf("overload count = %d, want exact single-signature census", got)
	}
	if got := transcript.SelectedSignature.MinimumArgumentCount; got != 3 {
		t.Fatalf("minimum argument count = %d, want 3", got)
	}
	if !transcript.SelectedSignature.HasRest || len(transcript.SelectedSignature.Parameters) != 2 {
		t.Fatalf("selected parameters = %#v, want final rest formal", transcript.SelectedSignature.Parameters)
	}
	if len(transcript.Bindings) != 2 {
		t.Fatalf("bindings = %#v, want two written arguments", transcript.Bindings)
	}
	if got := transcript.Bindings[1].Disposition; got != typefacts.ArgumentBindingExactTupleSpread {
		t.Fatalf("spread disposition = %q, want exact tuple", got)
	}
	if got := len(transcript.Bindings[1].Slots); got != 2 {
		t.Fatalf("spread slots = %d, want 2", got)
	}
	if transcript.Bindings[1].Slots[0].ParameterIndex != 1 ||
		transcript.Bindings[1].Slots[1].ParameterIndex != 1 ||
		!transcript.Bindings[1].Slots[1].Rest {
		t.Fatalf("spread mappings = %#v, want both slots mapped to rest formal", transcript.Bindings[1].Slots)
	}
	if !transcript.Completeness.Contains(typefacts.InvocationDomainBindings) ||
		!transcript.Completeness.Contains(typefacts.InvocationDomainOmissions) {
		t.Fatalf("completeness = %#v, want bindings and omissions closed", transcript.Completeness)
	}
	seenCompleteness := make(map[typefacts.InvocationDomain]bool, len(transcript.Completeness))
	for _, domain := range transcript.Completeness {
		if seenCompleteness[domain] {
			t.Fatalf("completeness contains duplicate domain %q: %#v", domain, transcript.Completeness)
		}
		seenCompleteness[domain] = true
	}
	parameter := transcript.SelectedSignature.Parameters[0]
	if !hasCallablePath(parameter.CallablePaths, "nested", "callback") {
		t.Fatalf("callable paths = %#v, want nested.callback", parameter.CallablePaths)
	}
	if !hasFinitePartition(parameter.Value.Partitions, typefacts.FinitePartitionDiscriminant) {
		t.Fatalf("parameter partitions = %#v, want mode discriminant", parameter.Value.Partitions)
	}
	if !hasFinitePartition(transcript.SelectedSignature.Result.Partitions, typefacts.FinitePartitionProtocol) {
		t.Fatalf("result partitions = %#v, want Promise protocol", transcript.SelectedSignature.Result.Partitions)
	}
	for _, fact := range transcript.SelectedSignature.ResultCallablePaths {
		for _, segment := range fact.Path {
			if !utf8.ValidString(segment.Property) {
				t.Fatalf("compiler-internal escaped property crossed the wire: %q", segment.Property)
			}
		}
	}
	if transcript.ParameterUses == nil || transcript.ControlFlow == nil {
		t.Fatalf("census missing: uses=%#v flow=%#v", transcript.ParameterUses, transcript.ControlFlow)
	}
	if answer.Envelope.Generation != 1 || answer.Envelope.DemandSHA256 == "" ||
		answer.Envelope.ModuleGraphSHA256 == "" || len(answer.Envelope.Sources) < 1 {
		t.Fatalf("envelope = %#v, want generation and all proof identities", answer.Envelope)
	}
}

func TestInvocationTranscriptCountsOverloadsAndKeepsUnresolvedGenericLocal(t *testing.T) {
	dir := t.TempDir()
	source := `
function choose(value: string): string;
function choose(value: number): number;
function choose(value: string | number): string | number { return value; }
declare function forward<T>(value: T): T;
function generic<T>(value: T) { return forward(value); }
choose(1);
`
	configPath := filepath.Join(dir, "tsconfig.json")
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(configPath, []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), configPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.InvocationAnalyzer)
	demands := invocationDemandsForNeedles(
		sourcePath,
		source,
		[]string{"forward(value)", "choose(1)"},
		false,
	)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	generic := answer.Transcripts[0].SelectedSignature
	if generic == nil || len(generic.Parameters) != 1 {
		t.Fatalf("generic signature = %#v", generic)
	}
	if !slices.Contains(generic.Parameters[0].Value.OpenReasons, "unresolvedGeneric") ||
		!slices.Contains(generic.Result.OpenReasons, "unresolvedGeneric") {
		t.Fatalf("unresolved generic did not stay locally open: %#v", generic)
	}
	overloaded := answer.Transcripts[1].SelectedSignature
	if overloaded == nil || overloaded.OverloadCount != 3 || overloaded.OverloadOrdinal >= overloaded.OverloadCount {
		t.Fatalf("overload census = %#v, want selected ordinal within three declarations", overloaded)
	}
}

func TestInvocationTranscriptDistinguishesOverloadsThroughAnImportAlias(t *testing.T) {
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"origin.ts": `export function select(value: string): "string";
export function select(value: number): "number";
export function select(value: string | number) { return typeof value === "string" ? "string" : "number"; }
`,
		"use.ts": `import { select as choose } from "./origin";
choose("value");
choose(1);
`,
	})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "use.ts")
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	demands := invocationDemandsForNeedles(path, string(source), []string{`choose("value")`, `choose(1)`}, false)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	stringSignature := answer.Transcripts[0].SelectedSignature
	numberSignature := answer.Transcripts[1].SelectedSignature
	if stringSignature == nil || numberSignature == nil {
		t.Fatalf("selected signatures = %#v", answer.Transcripts)
	}
	if stringSignature.Identity == numberSignature.Identity ||
		stringSignature.OverloadOrdinal == numberSignature.OverloadOrdinal {
		t.Fatalf("overload identities collapsed: string=%#v number=%#v", stringSignature, numberSignature)
	}
	if stringSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") ||
		numberSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") {
		t.Fatalf("alias did not resolve to origin declarations: %#v %#v", stringSignature.Declaration, numberSignature.Declaration)
	}
}

func TestInvocationTranscriptResolvesNamespaceReexportsAndConstructSignatures(t *testing.T) {
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"origin.ts": `export function select(value: string): string { return value; }
export class Box<T> { constructor(readonly value: T) {} }
`,
		"barrel.ts": `export { select, Box } from "./origin";`,
		"use.ts": `import * as api from "./barrel";
api.select("value");
new api.Box(1);
`,
	})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "use.ts")
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, string(source), []string{`api.select("value")`, `new api.Box(1)`}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	call, construct := answer.Transcripts[0], answer.Transcripts[1]
	if call.Kind != typefacts.CallKindCall || construct.Kind != typefacts.CallKindConstruct ||
		call.SelectedSignature == nil || construct.SelectedSignature == nil {
		t.Fatalf("namespace call/construct facts = %#v", answer.Transcripts)
	}
	for _, transcript := range []typefacts.InvocationTranscript{call, construct} {
		if transcript.SelectedSignature.Declaration.Location.Path != filepath.Join(dir, "origin.ts") {
			t.Fatalf("reexport did not resolve to origin: %#v", transcript.SelectedSignature.Declaration)
		}
	}
}

func TestInvocationDemandMustNameAnExactCallAndStayWithinDepthLimit(t *testing.T) {
	dir := t.TempDir()
	source := `declare function invoke(value: string): void;
invoke("value");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	start := strings.LastIndex(source, `invoke("value")`)
	answer, err := analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location: typefacts.Location{Path: path, StartByte: start, EndByte: start + len("invoke")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	if answer.Transcripts[0].SelectedSignature != nil ||
		!slices.Contains(answer.Transcripts[0].OpenReasons, "callNotExact") {
		t.Fatalf("non-exact demand widened to a call: %#v", answer.Transcripts[0])
	}
	_, err = analyzer.InvocationTranscripts(context.Background(), []typefacts.InvocationDemand{{
		Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(`invoke("value")`)},
		CallableDepth: typefacts.MaxInvocationCallableDepth + 1,
	}})
	if err == nil {
		t.Fatal("over-limit callable depth was silently truncated")
	}
}

func TestInvocationBindingsKeepOptionalRestAndUnknownSpreadsDistinct(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function variadic(required: string, optional?: number, ...callbacks: Array<() => void>): void;
declare function tupled(...values: [string] | [string, number]): void;
const callback = () => {};
const exact = ["value", 1, callback] as const;
declare const open: Array<string | number | (() => void)>;
declare const choice: [string] | [string, number];
variadic("value");
variadic(...exact);
variadic(...open);
tupled(...choice);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	needles := []string{`variadic("value")`, `variadic(...exact)`, `variadic(...open)`, `tupled(...choice)`}
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, needles, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	omitted := answer.Transcripts[0]
	if !omitted.Completeness.Contains(typefacts.InvocationDomainOmissions) ||
		len(omitted.OmittedParameters) != 1 || omitted.OmittedParameters[0] != 1 {
		t.Fatalf("optional omission = %#v", omitted)
	}
	exact := answer.Transcripts[1]
	if got := exact.Bindings[0].Disposition; got != typefacts.ArgumentBindingExactTupleSpread {
		t.Fatalf("exact spread = %q", got)
	}
	if got := []int{
		exact.Bindings[0].Slots[0].ParameterIndex,
		exact.Bindings[0].Slots[1].ParameterIndex,
		exact.Bindings[0].Slots[2].ParameterIndex,
	}; got[0] != 0 || got[1] != 1 || got[2] != 2 || !exact.Bindings[0].Slots[2].Rest {
		t.Fatalf("exact spread mappings = %#v", exact.Bindings[0].Slots)
	}
	for _, index := range []int{2, 3} {
		transcript := answer.Transcripts[index]
		if transcript.Bindings[0].Disposition != typefacts.ArgumentBindingUnknownLengthSpread ||
			transcript.Completeness.Contains(typefacts.InvocationDomainBindings) ||
			transcript.Completeness.Contains(typefacts.InvocationDomainOmissions) {
			t.Fatalf("open spread %d certified binding: %#v", index, transcript)
		}
	}
}

func TestCallablePathsProveUnionSiblingAbsenceLocally(t *testing.T) {
	dir := t.TempDir()
	source := `
type Variant =
  | { kind: "callback"; nested: { run(): void } }
  | { kind: "value"; nested: { value: number } };
declare function consume(variant: Variant): void;
declare const variant: Variant;
consume(variant);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"consume(variant)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	paths := answer.Transcripts[0].SelectedSignature.Parameters[0].CallablePaths
	var runPresent, runAbsent bool
	for _, path := range paths {
		if !pathNamesEqual(path.Path, "nested", "run") {
			continue
		}
		runPresent = runPresent || path.Presence != typefacts.PathAbsent && path.Callability == typefacts.CallabilityCallable
		runAbsent = runAbsent || path.Presence == typefacts.PathAbsent && path.Complete
	}
	if !runPresent || !runAbsent {
		t.Fatalf("union-local run facts = %#v", paths)
	}
}

func TestProtocolPartitionUsesTheCompilerForStructuralAsyncIterables(t *testing.T) {
	dir := t.TempDir()
	source := `
interface StructuralStream {
  [Symbol.asyncIterator](): AsyncIterator<number>;
}
declare function choose(): Promise<number> | StructuralStream | number;
choose(/*invoke*/);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"choose(/*invoke*/)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	partitions := answer.Transcripts[0].SelectedSignature.Result.Partitions
	var protocol *typefacts.FinitePartition
	for index := range partitions {
		if partitions[index].Axis == typefacts.FinitePartitionProtocol {
			protocol = &partitions[index]
			break
		}
	}
	if protocol == nil || !protocol.Complete || len(protocol.Cases) != 3 {
		t.Fatalf("protocol partition = %#v, want exact plain/Promise/AsyncIterable", protocol)
	}
	got := make(map[typefacts.ValueProtocol]bool)
	for _, candidate := range protocol.Cases {
		got[candidate.Protocol] = true
	}
	for _, expected := range []typefacts.ValueProtocol{
		typefacts.ValueProtocolPlain,
		typefacts.ValueProtocolPromise,
		typefacts.ValueProtocolAsyncIterable,
	} {
		if !got[expected] {
			t.Fatalf("protocol partition lacks %q: %#v", expected, protocol.Cases)
		}
	}
}

func TestFinitePartitionsDoNotInventCallabilityOrDiscriminantCases(t *testing.T) {
	dir := t.TempDir()
	source := `
type Overlap =
  | { kind: "same"; left: "a" }
  | { kind: "same"; right: "b" };
declare function classify(value: (() => void) | Function | number, overlap: Overlap): void;
declare const value: (() => void) | Function | number;
declare const overlap: Overlap;
classify(value, overlap);
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"classify(value, overlap)"}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	parameters := answer.Transcripts[0].SelectedSignature.Parameters
	var callability *typefacts.FinitePartition
	for index := range parameters[0].Value.Partitions {
		candidate := &parameters[0].Value.Partitions[index]
		if candidate.Axis == typefacts.FinitePartitionCallability {
			callability = candidate
		}
	}
	if callability == nil || len(callability.Cases) != 3 {
		t.Fatalf("callability partition = %#v, want all three proven categories", callability)
	}
	want := map[string]bool{"callable": true, "untypedCallable": true, "nonCallable": true}
	for _, candidate := range callability.Cases {
		delete(want, candidate.Kind)
	}
	if len(want) != 0 {
		t.Fatalf("callability partition omitted categories: %#v", callability.Cases)
	}
	if hasFinitePartition(parameters[1].Value.Partitions, typefacts.FinitePartitionDiscriminant) {
		t.Fatalf("overlapping object union was called discriminated: %#v", parameters[1].Value.Partitions)
	}
}

func TestInvocationCensusesClassifyAliasesCapturesReturnsThrowsAndUnreachableSites(t *testing.T) {
	dir := t.TempDir()
	source := `
declare function accept(callback: () => void): void;
function inspect(callback: () => void, mode: "return" | "throw") {
  const alias = callback;
  alias();
  accept(callback);
  if (mode === "return") return () => callback();
  throw new Error("stop");
  callback();
}
inspect(() => {}, "return");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	demand := invocationDemandsForNeedles(path, source, []string{`inspect(() => {}, "return")`}, true)
	answer, err := analyzer.InvocationTranscripts(context.Background(), demand)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	kinds := make(map[typefacts.ParameterUseKind]bool)
	for _, use := range transcript.ParameterUses {
		kinds[use.Kind] = true
	}
	for _, kind := range []typefacts.ParameterUseKind{
		typefacts.ParameterUseStorage,
		typefacts.ParameterUseAliasCall,
		typefacts.ParameterUseArgumentKnown,
		typefacts.ParameterUseCapture,
	} {
		if !kinds[kind] {
			t.Fatalf("parameter uses lack %q: %#v", kind, transcript.ParameterUses)
		}
	}
	flow := transcript.ControlFlow
	if flow == nil || len(flow.Returns) != 1 || len(flow.Throws) != 1 || len(flow.Branches) != 1 {
		t.Fatalf("control-flow census = %#v", flow)
	}
	if len(flow.Returns[0].Captures) != 1 || flow.Returns[0].Captures[0] != 0 {
		t.Fatalf("returned closure captures = %#v", flow.Returns[0].Captures)
	}
	var unreachableCall bool
	for _, use := range transcript.ParameterUses {
		if strings.Contains(source[use.Location.StartByte:use.Location.EndByte], "callback") &&
			use.Location.StartByte > flow.Throws[0].Location.StartByte {
			unreachableCall = true
		}
	}
	if !unreachableCall {
		t.Fatalf("parameter-use census dropped the use after throw: %#v", transcript.ParameterUses)
	}
}

func TestUnsupportedFlowKeepsOnlyControlFlowOpen(t *testing.T) {
	dir := t.TempDir()
	source := `
function repeat(callback: () => void) { while (Math.random()) callback(); }
repeat(() => {});
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{"repeat(() => {})"}, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	if !transcript.Completeness.Contains(typefacts.InvocationDomainSignature) ||
		!transcript.Completeness.Contains(typefacts.InvocationDomainUses) ||
		transcript.Completeness.Contains(typefacts.InvocationDomainControlFlow) ||
		transcript.ControlFlow == nil || len(transcript.ControlFlow.Unsupported) == 0 {
		t.Fatalf("unsupported flow completeness leaked: %#v", transcript)
	}
}

func TestFunctionPrototypeCallApplyAndBindRefuseTheWrapperSignature(t *testing.T) {
	dir := t.TempDir()
	source := `
function target(value: string, count: number): boolean { return value.length === count; }
target.call(undefined, "x", 1);
target.apply(undefined, ["x", 1]);
target.bind(undefined, "x");
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{
			`target.call(undefined, "x", 1)`,
			`target.apply(undefined, ["x", 1])`,
			`target.bind(undefined, "x")`,
		}, false),
	)
	if err != nil {
		t.Fatal(err)
	}
	for index, transcript := range answer.Transcripts {
		if transcript.SelectedSignature != nil || len(transcript.Completeness) != 0 ||
			!slices.Contains(transcript.OpenReasons, "indirectFunctionInvocation") {
			t.Fatalf("indirect invocation %d certified the wrapper signature: %#v", index, transcript)
		}
	}
}

func TestInvocationCensusPreservesDestructuredBindingPaths(t *testing.T) {
	dir := t.TempDir()
	source := `
function inspect({ nested: { callback }, pair: [, second] }: {
  nested: { callback: () => void };
  pair: [number, () => void];
}) {
  callback();
  second();
}
inspect({ nested: { callback() {} }, pair: [1, () => {}] });
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	analyzer, closeProject := openInvocationAnalyzer(t, dir)
	defer closeProject()
	path := filepath.Join(dir, "facts.ts")
	answer, err := analyzer.InvocationTranscripts(
		context.Background(),
		invocationDemandsForNeedles(path, source, []string{
			`inspect({ nested: { callback() {} }, pair: [1, () => {}] })`,
		}, true),
	)
	if err != nil {
		t.Fatal(err)
	}
	uses := answer.Transcripts[0].ParameterUses
	var callbackPath, secondPath bool
	for _, use := range uses {
		callbackPath = callbackPath || pathNamesEqual(use.BindingPath, "nested", "callback")
		secondPath = secondPath || len(use.BindingPath) == 2 &&
			use.BindingPath[0].Property == "pair" && use.BindingPath[1].Index != nil &&
			*use.BindingPath[1].Index == 1
	}
	if !callbackPath || !secondPath {
		t.Fatalf("destructured binding paths = %#v", uses)
	}
}

func TestInvocationValidityMatchesPublishedTypeScriptForOverloadAndSpreadCases(t *testing.T) {
	repositoryRoot, err := filepath.Abs(filepath.Join("..", "..", "..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	tsc := filepath.Join(repositoryRoot, "packages", "cli", "node_modules", ".bin", "tsc")
	if _, err := os.Stat(tsc); err != nil {
		t.Fatalf("published TypeScript oracle is unavailable at %s: %v", tsc, err)
	}
	cases := []struct {
		name  string
		call  string
		valid bool
	}{
		{"selected string overload", `choose("x")`, true},
		{"selected number overload", `choose(1)`, true},
		{"exact tuple spread", `rest(...(["x", 1] as const))`, true},
		{"unknown array to rest", `rest(...openValues)`, true},
		{"invalid overload", `choose(true)`, false},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			dir := t.TempDir()
			source := `
declare function choose(value: string): string;
declare function choose(value: number): number;
declare function rest(...values: Array<string | number>): void;
declare const openValues: Array<string | number>;
` + testCase.call + `;
`
			writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
			command := exec.Command(tsc, "--noEmit", "--strict", "--skipLibCheck", "--target", "esnext", filepath.Join(dir, "facts.ts"))
			output, tscErr := command.CombinedOutput()
			if (tscErr == nil) != testCase.valid {
				t.Fatalf("published tsc validity = %v, want %v: %s", tscErr == nil, testCase.valid, output)
			}
			analyzer, closeProject := openInvocationAnalyzer(t, dir)
			defer closeProject()
			path := filepath.Join(dir, "facts.ts")
			answer, err := analyzer.InvocationTranscripts(
				context.Background(),
				invocationDemandsForNeedles(path, source, []string{testCase.call}, false),
			)
			if err != nil {
				t.Fatal(err)
			}
			got := answer.Transcripts[0].Validity == typefacts.ResolvedCallValid
			if got != testCase.valid {
				t.Fatalf("Type Facts validity = %v, published tsc = %v; transcript=%#v", got, testCase.valid, answer.Transcripts[0])
			}
		})
	}
}

func writeInvocationProject(t *testing.T, dir string, files map[string]string) {
	t.Helper()
	if err := os.WriteFile(
		filepath.Join(dir, "tsconfig.json"),
		[]byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}`),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	for name, source := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func openInvocationAnalyzer(t *testing.T, dir string) (typefacts.InvocationAnalyzer, func()) {
	t.Helper()
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	analyzer, ok := opened.(typefacts.InvocationAnalyzer)
	if !ok {
		_ = opened.Close()
		t.Fatal("TypeScript-Go project does not implement InvocationAnalyzer")
	}
	return analyzer, func() { _ = opened.Close() }
}

func invocationDemandsForNeedles(path, source string, needles []string, census bool) []typefacts.InvocationDemand {
	demands := make([]typefacts.InvocationDemand, len(needles))
	for index, needle := range needles {
		start := strings.Index(source, needle)
		demands[index] = typefacts.InvocationDemand{
			Location:      typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)},
			CallableDepth: 3,
			Census:        census,
		}
	}
	return demands
}

func pathNamesEqual(path []typefacts.PathSegment, names ...string) bool {
	if len(path) != len(names) {
		return false
	}
	for index := range names {
		if path[index].Property != names[index] {
			return false
		}
	}
	return true
}

func hasCallablePath(paths []typefacts.CallablePathFact, names ...string) bool {
	for _, path := range paths {
		if len(path.Path) != len(names) {
			continue
		}
		matches := true
		for index := range names {
			if path.Path[index].Property != names[index] {
				matches = false
				break
			}
		}
		if matches && path.Callability == typefacts.CallabilityCallable {
			return true
		}
	}
	return false
}

func hasFinitePartition(partitions []typefacts.FinitePartition, axis typefacts.FinitePartitionAxis) bool {
	for _, partition := range partitions {
		if partition.Axis == axis && partition.Complete {
			return true
		}
	}
	return false
}
