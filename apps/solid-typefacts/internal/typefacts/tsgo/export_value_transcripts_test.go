package tsgo

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

func TestExportValueTranscriptResolvesImportedAliasWithoutInventingCall(t *testing.T) {
	dir := t.TempDir()
	packageDir := filepath.Join(dir, "node_modules", "fixture-package")
	if err := os.MkdirAll(packageDir, 0o755); err != nil {
		t.Fatal(err)
	}
	write := func(path, source string) {
		t.Helper()
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(packageDir, "package.json"), `{"name":"fixture-package","version":"1.0.0","types":"index.d.ts"}`)
	write(filepath.Join(packageDir, "index.d.ts"), `export declare const nestedValue: { nested: { callback: (value: number) => void } };`)
	write(filepath.Join(dir, "tsconfig.json"), `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["harness.ts"]}`)
	source := `import { nestedValue as __solid_checker_export_0 } from "fixture-package";
void __solid_checker_export_0;
`
	harness := filepath.Join(dir, "harness.ts")
	write(harness, source)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}
	needle := "__solid_checker_export_0"
	start := strings.LastIndex(source, needle)
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location:      typefacts.Location{Path: harness, StartByte: start, EndByte: start + len(needle)},
		CallableDepth: 3,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 {
		t.Fatalf("transcripts = %d, want 1", len(answer.Transcripts))
	}
	transcript := answer.Transcripts[0]
	if !transcript.Complete || len(transcript.OpenReasons) != 0 {
		t.Fatalf("transcript = %#v, want exact closed identity", transcript)
	}
	if transcript.QueryName != needle || transcript.Declaration == nil || transcript.Target == "" {
		t.Fatalf("transcript identity = %#v, want query, target, and declaration", transcript)
	}
	if got := filepath.Clean(transcript.Declaration.Location.Path); !strings.HasSuffix(got, filepath.Join("node_modules", "fixture-package", "index.d.ts")) {
		t.Fatalf("declaration path = %q, want package declaration", got)
	}
	found := false
	for _, path := range transcript.CallablePaths {
		if len(path.Path) == 2 && path.Path[0].Property == "nested" && path.Path[1].Property == "callback" {
			found = path.Complete && path.Callability == typefacts.CallabilityCallable
		}
	}
	if !found {
		t.Fatalf("callable paths = %#v, want exact nested.callback", transcript.CallablePaths)
	}
}

func TestExportValueTranscriptKeepsUnknownOpen(t *testing.T) {
	dir := t.TempDir()
	source := "declare const value: unknown;\nvoid value;\n"
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	start := strings.LastIndex(source, "value")
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: start, EndByte: start + len("value")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := answer.Transcripts[0].Value.Callability; got != typefacts.CallabilityUnknown {
		t.Fatalf("callability = %q, want unknown rather than a positive or negative", got)
	}
}

func TestExportValueTranscriptRetainsCallableGenericAlias(t *testing.T) {
	dir := t.TempDir()
	solidDir := filepath.Join(dir, "node_modules", "solid-js")
	fixtureDir := filepath.Join(dir, "node_modules", "fixture-package")
	if err := os.MkdirAll(solidDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(fixtureDir, 0o755); err != nil {
		t.Fatal(err)
	}
	write := func(path, contents string) {
		t.Helper()
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(solidDir, "package.json"), `{"name":"solid-js","version":"1.0.0","types":"index.d.ts"}`)
	write(filepath.Join(solidDir, "index.d.ts"), `export type Accessor<T> = () => T;`)
	write(filepath.Join(fixtureDir, "package.json"), `{"name":"fixture-package","version":"1.0.0","types":"index.d.ts"}`)
	write(filepath.Join(fixtureDir, "index.d.ts"), `import type { Accessor } from "solid-js";
export declare function generic<T>(callback: Accessor<T>): [Accessor<T>, T];`)
	write(filepath.Join(dir, "tsconfig.json"), `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["harness.ts"]}`)
	source := `import { generic } from "fixture-package";
void generic;
`
	write(filepath.Join(dir, "harness.ts"), source)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	start := strings.LastIndex(source, "generic")
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location:      typefacts.Location{Path: filepath.Join(dir, "harness.ts"), StartByte: start, EndByte: start + len("generic")},
		CallableDepth: 1,
	}})
	if err != nil {
		t.Fatal(err)
	}
	signature := answer.Transcripts[0].CallSignature
	if signature == nil || len(signature.Parameters) != 1 {
		t.Fatalf("signature = %#v, want one exact generic signature", signature)
	}
	if got := signature.Parameters[0].Value.Callability; got != typefacts.CallabilityCallable {
		t.Fatalf("constrained callback callability = %q, want callable", got)
	}
	if got := signature.Parameters[0].Value.Constructability; got != typefacts.InvocationNonConstructable {
		t.Fatalf("constrained callback constructability = %q, want non-constructable", got)
	}
	if declared := signature.Parameters[0].DeclaredType; declared == nil || declared.Module != "solid-js" || declared.Name != "Accessor" {
		t.Fatalf("declared callback type = %#v, want solid-js Accessor", declared)
	}
	var first *typefacts.CallablePathFact
	for index := range signature.ResultCallablePaths {
		path := &signature.ResultCallablePaths[index]
		if len(path.Path) != 1 || path.Path[0].Index == nil {
			continue
		}
		if *path.Path[0].Index == 0 {
			first = path
		}
	}
	if first == nil || first.Callability != typefacts.CallabilityCallable || !first.Complete {
		t.Fatalf("callable constrained tuple item = %#v, want closed callable", first)
	}
}

func TestExportValueTranscriptRetainsUnresolvedImportedTypeIdentity(t *testing.T) {
	dir := t.TempDir()
	fixtureDir := filepath.Join(dir, "node_modules", "fixture-package")
	if err := os.MkdirAll(fixtureDir, 0o755); err != nil {
		t.Fatal(err)
	}
	write := func(path, contents string) {
		t.Helper()
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(fixtureDir, "package.json"), `{"name":"fixture-package","version":"1.0.0","types":"index.d.ts"}`)
	write(filepath.Join(fixtureDir, "index.d.ts"), `import type { Accessor } from "solid-js";
export declare function until<T>(condition: Accessor<T>): Promise<T>;`)
	write(filepath.Join(dir, "tsconfig.json"), `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["harness.ts"]}`)
	source := `import { until } from "fixture-package";
void until;
`
	write(filepath.Join(dir, "harness.ts"), source)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	start := strings.LastIndex(source, "until")
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location: typefacts.Location{Path: filepath.Join(dir, "harness.ts"), StartByte: start, EndByte: start + len("until")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	parameter := answer.Transcripts[0].CallSignature.Parameters[0]
	if parameter.Value.Callability != typefacts.CallabilityUnknown {
		t.Fatalf("unresolved Accessor callability = %q, want unknown", parameter.Value.Callability)
	}
	if declared := parameter.DeclaredType; declared == nil || declared.Module != "solid-js" || declared.Name != "Accessor" {
		t.Fatalf("declared callback type = %#v, want exact unresolved import identity", declared)
	}
}

func TestExportValueTranscriptSelectionFailureHasWireValidUnknownValue(t *testing.T) {
	dir := t.TempDir()
	source := "const value = 1;\n"
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: 0, EndByte: len("const")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	if transcript.Complete || len(transcript.OpenReasons) == 0 {
		t.Fatalf("transcript = %#v, want an explicit selection refusal", transcript)
	}
	if transcript.Value.Callability != typefacts.CallabilityUnknown ||
		transcript.Value.Constructability != typefacts.InvocationConstructUnknown ||
		!transcript.Value.Primitive.Unknown {
		t.Fatalf("value = %#v, want a wire-valid unknown domain", transcript.Value)
	}
	if len(transcript.Value.OpenReasons) != 1 || transcript.Value.OpenReasons[0] != "valueUnavailable" {
		t.Fatalf("value open reasons = %#v", transcript.Value.OpenReasons)
	}
}

func TestExportImplementationTranscriptDistinguishesDirectReturnedAndRetainedCalls(t *testing.T) {
	dir := t.TempDir()
	source := `export function direct(callback: () => void) {
  try { callback(); } finally { void 0; }
}
export function returned(callback: () => void) {
  function invoke() { callback(); }
  return invoke;
}
export function retained(callback: () => void) {
  function invoke() { callback(); }
  void invoke;
  return () => {};
}
void direct;
void returned;
void retained;
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	demands := make([]typefacts.ExportValueDemand, 0, 3)
	for _, name := range []string{"direct", "returned", "retained"} {
		queryStart := strings.LastIndex(source, "void "+name) + len("void ")
		implementationStart := strings.Index(source, "function "+name) + len("function ")
		location := typefacts.Location{Path: path, StartByte: queryStart, EndByte: queryStart + len(name)}
		implementation := typefacts.Location{Path: path, StartByte: implementationStart, EndByte: implementationStart + len(name)}
		demands = append(demands, typefacts.ExportValueDemand{
			Location: location, ImplementationLocation: &implementation,
		})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 3 {
		t.Fatalf("transcripts = %d, want 3", len(answer.Transcripts))
	}
	direct := answer.Transcripts[0].Implementation
	if direct == nil || len(direct.Calls) != 1 || direct.Calls[0].Reach != typefacts.Reachable || direct.Calls[0].Captured {
		t.Fatalf("direct implementation = %#v, want one reachable direct call", direct)
	}
	if direct.Complete || len(direct.OpenReasons) != 1 || direct.OpenReasons[0] != "controlFlowUnsupported" {
		t.Fatalf("direct completeness = %#v, want only the try/finally frontier open", direct)
	}
	returned := answer.Transcripts[1].Implementation
	if returned == nil || len(returned.Calls) != 1 || !returned.Calls[0].Captured {
		t.Fatalf("returned implementation = %#v, want one captured call", returned)
	}
	if returned.ControlFlow == nil || len(returned.ControlFlow.Returns) != 1 ||
		len(returned.ControlFlow.Returns[0].CarriedCallables) != 1 {
		t.Fatalf("returned control flow = %#v, want one carried callable", returned.ControlFlow)
	}
	if !locationWithinAny(returned.Calls[0].Location, returned.ControlFlow.Returns[0].CarriedCallables) {
		t.Fatalf(
			"returned captured call %#v is not inside the carried callable %#v",
			returned.Calls[0].Location, returned.ControlFlow.Returns[0].CarriedCallables,
		)
	}
	retained := answer.Transcripts[2].Implementation
	if retained == nil || len(retained.Calls) != 1 || !retained.Calls[0].Captured {
		t.Fatalf("retained implementation = %#v, want one captured call", retained)
	}
	// The retained closure is never returned. Its call sits outside every
	// carried range, which is exactly the authority a consumer must not find.
	if retained.ControlFlow == nil || len(retained.ControlFlow.Returns) != 1 ||
		len(retained.ControlFlow.Returns[0].CarriedCallables) != 1 {
		t.Fatalf("retained control flow = %#v, want the returned arrow carried", retained.ControlFlow)
	}
	if locationWithinAny(retained.Calls[0].Location, retained.ControlFlow.Returns[0].CarriedCallables) {
		t.Fatalf(
			"retained call %#v was placed inside a carried callable %#v",
			retained.Calls[0].Location, retained.ControlFlow.Returns[0].CarriedCallables,
		)
	}
}

// TestReportedOverloadSetIsAllOrNothing pins the producer half of the overload
// guard: a set that describes fewer signatures than the type has is not a
// smaller answer to the same question, it is a different one. "Every overload"
// silently becoming "every overload we could describe" is the soundness loss,
// so the gate answers nothing at all.
func TestReportedOverloadSetIsAllOrNothing(t *testing.T) {
	complete := []typefacts.SelectedSignature{
		{OverloadOrdinal: 0, OverloadCount: 2},
		{OverloadOrdinal: 1, OverloadCount: 2},
	}
	if got := completeOverloadSet(complete, 2); len(got) != 2 {
		t.Fatalf("complete set = %#v, want both signatures", got)
	}
	for _, missing := range [][]typefacts.SelectedSignature{
		nil,
		complete[:1],
		complete[1:],
	} {
		if got := completeOverloadSet(missing, 2); got != nil {
			t.Fatalf("partial set %#v was reported as %#v, want nothing", missing, got)
		}
	}
}

func TestExportImplementationTranscriptBindsReturnedCallResultProvenance(t *testing.T) {
	dir := t.TempDir()
	source := `import { createSignal as signal } from "solid-js";
function createSignal(value: unknown) { return [() => value, () => {}]; }
export function imported(initial: unknown) {
  const [value] = signal(initial);
  return [value, () => {}];
}
export function local(initial: unknown) {
  const [value] = createSignal(initial);
  return [value, () => {}];
}
void imported;
void local;
`
	writeInvocationProject(t, dir, map[string]string{
		"facts.ts": source,
		"solid-js.d.ts": `declare module "solid-js" {
  export function createSignal(value: unknown): [() => unknown, () => void];
}`,
	})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	var demands []typefacts.ExportValueDemand
	for _, name := range []string{"imported", "local"} {
		queryStart := strings.LastIndex(source, "void "+name) + len("void ")
		implementationStart := strings.Index(source, "function "+name) + len("function ")
		location := typefacts.Location{Path: path, StartByte: queryStart, EndByte: queryStart + len(name)}
		implementation := typefacts.Location{Path: path, StartByte: implementationStart, EndByte: implementationStart + len(name)}
		demands = append(demands, typefacts.ExportValueDemand{Location: location, ImplementationLocation: &implementation, CallableDepth: 1})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, wantModule := range []string{"solid-js", ""} {
		flow := answer.Transcripts[index].Implementation.ControlFlow
		if flow == nil || len(flow.Returns) != 1 || len(flow.Returns[0].Sources) != 2 {
			t.Fatalf("transcript %d return sources = %#v, want call result and direct callable", index, flow)
		}
		callResult := flow.Returns[0].Sources[0]
		if callResult.Kind != typefacts.ImplementationValueCallResult || callResult.TargetModule != wantModule ||
			len(callResult.Path) != 1 || callResult.Path[0].Index == nil || *callResult.Path[0].Index != 0 ||
			len(callResult.TargetPath) != 1 || callResult.TargetPath[0].Index == nil || *callResult.TargetPath[0].Index != 0 {
			t.Fatalf("transcript %d call-result source = %#v", index, callResult)
		}
		if direct := flow.Returns[0].Sources[1]; direct.Kind != typefacts.ImplementationValueDirectCallable || len(direct.Path) != 1 || direct.Path[0].Index == nil || *direct.Path[0].Index != 1 {
			t.Fatalf("transcript %d direct-callable source = %#v", index, direct)
		}
	}
}

// A returned identifier whose symbol has no value declaration and several
// declarations — here a namespace import, whose module symbol resolves to its
// source declarations with no single value declaration — leaves the resolved
// closure node nil. This mirrors @solid-primitives/media@2.3.6, whose
// `return noop`-style returned bindings crashed the producer. The
// returned-closure capture census must treat a nil closure as a
// missing-evidence frontier — no proven captures — rather than dereferencing
// the nil AST node. Before the nil guard this panicked in ast.IsArrowFunction
// (isCallableDeclaration(nil)) and took the whole producer session down.
func TestExportImplementationTranscriptKeepsReturnedNilClosureOpen(t *testing.T) {
	dir := t.TempDir()
	// The namespace binding resolves to an alias with no value declaration and
	// not exactly one declaration, so the returned-closure resolution yields a
	// nil closure node — the exact shape that crashed the producer on
	// @solid-primitives/media@2.3.6.
	source := `import * as ns from "./unresolved-peer";
export function returnsNamespace(callback: () => void): unknown {
  callback();
  return ns;
}
void returnsNamespace;
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	queryStart := strings.LastIndex(source, "void returnsNamespace") + len("void ")
	implementationStart := strings.Index(source, "function returnsNamespace") + len("function ")
	location := typefacts.Location{Path: path, StartByte: queryStart, EndByte: queryStart + len("returnsNamespace")}
	implementation := typefacts.Location{Path: path, StartByte: implementationStart, EndByte: implementationStart + len("returnsNamespace")}
	demands := []typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &implementation}}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 {
		t.Fatalf("transcripts = %d, want 1", len(answer.Transcripts))
	}
	returned := answer.Transcripts[0].Implementation
	if returned == nil || returned.ControlFlow == nil || len(returned.ControlFlow.Returns) != 1 {
		t.Fatalf("implementation = %#v, want one return site", returned)
	}
	if carried := returned.ControlFlow.Returns[0].CarriedCallables; len(carried) != 0 {
		t.Fatalf("carried callables = %#v, want none proven for a nil closure", carried)
	}
}

// TestImplementationCallCensusKeepsCallsAfterAFallThroughConstructReachable pins
// the call census's half of the fall-through reachability rule. The control-flow
// census already answered this question for return sites; the call census
// answered `unknown` for every statement after any loop or switch, so a single
// `for (const [key, value] of Object.entries(props))` made every later
// `createRenderEffect` unusable as evidence. The two censuses read the same
// constructs and must give the same answer.
func TestImplementationCallCensusKeepsCallsAfterAFallThroughConstructReachable(t *testing.T) {
	cases := []struct {
		name string
		body string
		want typefacts.Reachability
	}{{
		name: "forOfWithoutJumpsFallsThrough",
		body: `for (const key of keys) { sink(key); }`,
		want: typefacts.Reachable,
	}, {
		name: "forOfWithContinueFallsThrough",
		body: `for (const key of keys) { if (key) continue; sink(key); }`,
		want: typefacts.Reachable,
	}, {
		name: "nestedLoopBreakFallsThrough",
		body: `for (const key of keys) { for (const other of keys) { if (other) break; } }`,
		want: typefacts.Reachable,
	}, {
		name: "whileTrueWithBreakFallsThrough",
		body: `while (true) { if (keys.length) break; }`,
		want: typefacts.Reachable,
	}, {
		name: "forEverWithBreakFallsThrough",
		body: `for (;;) { if (keys.length) break; }`,
		want: typefacts.Reachable,
	}, {
		name: "doWhileTrueWithBreakFallsThrough",
		body: `do { if (keys.length) break; } while (true);`,
		want: typefacts.Reachable,
	}, {
		name: "switchWithBreaksFallsThrough",
		body: `switch (keys.length) { case 0: sink(0); break; default: sink(1); }`,
		want: typefacts.Reachable,
	}, {
		name: "labeledBreakTargetingTheConstructFallsThrough",
		body: `outer: for (const key of keys) { for (const other of keys) { if (other) break outer; } }`,
		want: typefacts.Reachable,
	}, {
		name: "nestedCallableJumpsAreNotOurControlFlow",
		body: `for (const key of keys) { sink(() => { if (key) return 1; throw new Error("x"); }); }`,
		want: typefacts.Reachable,
	}, {
		name: "forOfWithReturnStaysUnknown",
		body: `for (const key of keys) { if (key) return; }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileTrueWithoutBreakStaysUnknown",
		body: `while (true) { if (keys.length) sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "forEverWithoutBreakStaysUnknown",
		body: `for (;;) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "whileOneStaysUnknown",
		body: `while (1) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "forOfWrappingInfiniteLoopStaysUnknown",
		body: `for (const key of keys) { while (true) { sink(key); } }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "switchDefaultInfiniteLoopStaysUnknown",
		body: `switch (keys.length) { default: while (true) { sink(1); } }`,
		want: typefacts.ReachUnknown,
	}, {
		// A `const` that uniquely names `true` is `true`; reading the header as
		// exitable is what promoted the dead code after it.
		name: "constBoundInfiniteLoopStaysUnknown",
		body: `const ALWAYS = true;
  while (ALWAYS) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "literalComparisonInfiniteLoopStaysUnknown",
		body: `while (1 === 1) { sink(1); }`,
		want: typefacts.ReachUnknown,
	}, {
		// The comparison is read for its value, not merely for being a
		// comparison: an always-false header exits immediately.
		name: "alwaysFalseComparisonLoopFallsThrough",
		body: `while (1 === 2) { sink(1); }`,
		want: typefacts.Reachable,
	}, {
		// Retained approximation, pinned so it is a decision rather than an
		// accident: a condition no literal reading decides is treated as having
		// an exit edge, so the statements after it keep the reachability they
		// had. `keys.length` may be permanently non-zero and this still says
		// reachable. See docs/precision-backlog.md.
		name: "nonLiteralConditionStaysOptimistic",
		body: `while (keys.length) { sink(1); }`,
		want: typefacts.Reachable,
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `declare function sink(value: unknown): void;
declare const keys: string[];
export function make(callback: () => void) {
  ` + testCase.body + `
  callback();
}
void make;
`
			implementation := exportImplementationForMake(t, source)
			trailing := strings.LastIndex(source, "callback()")
			if trailing < 0 {
				t.Fatal("probe source lost its trailing call")
			}
			found := false
			for _, call := range implementation.Calls {
				if call.Location.StartByte == trailing {
					found = true
					if call.Reach != testCase.want {
						t.Fatalf(
							"call after the construct = %q, want %q (calls=%#v)",
							call.Reach, testCase.want, implementation.Calls,
						)
					}
					continue
				}
				// Reachability *inside* the construct is a separate claim and
				// stays unknown: a loop body may run zero times, and nothing
				// here promotes it.
				if call.Reach != typefacts.ReachUnknown {
					t.Fatalf(
						"call inside the construct = %q, want unknown (call=%#v)",
						call.Reach, call,
					)
				}
			}
			if !found {
				t.Fatalf("trailing call at %d is absent from the census: %#v", trailing, implementation.Calls)
			}
		})
	}
}

// TestImplementationCallCensusNeverPromotesAnUnreachableConstruct pins the
// one-way direction: a construct entered from an already-unreachable point never
// makes the call after it reachable.
func TestImplementationCallCensusNeverPromotesAnUnreachableConstruct(t *testing.T) {
	source := `declare function sink(value: unknown): void;
declare const keys: string[];
export function make(callback: () => void) {
  throw new Error("stop");
  for (const key of keys) { sink(key); }
  callback();
}
void make;
`
	implementation := exportImplementationForMake(t, source)
	trailing := strings.LastIndex(source, "callback()")
	for _, call := range implementation.Calls {
		// Not merely "not reachable": dead code stays dead across a loop, and
		// `unknown` would let it witness a zero-lower-bound claim.
		if call.Location.StartByte == trailing && call.Reach != typefacts.Unreachable {
			t.Fatalf(
				"call after an unreachable construct = %q, want unreachable: %#v",
				call.Reach, implementation.Calls,
			)
		}
	}
}

func exportImplementationForMake(
	t *testing.T,
	source string,
) *typefacts.ExportImplementationTranscript {
	t.Helper()
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	queryStart := strings.LastIndex(source, "void make") + len("void ")
	implementationStart := strings.Index(source, "function make") + len("function ")
	location := typefacts.Location{Path: path, StartByte: queryStart, EndByte: queryStart + len("make")}
	implementation := typefacts.Location{
		Path: path, StartByte: implementationStart, EndByte: implementationStart + len("make"),
	}
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{Location: location, ImplementationLocation: &implementation}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
		t.Fatalf("transcripts = %#v, want one implementation transcript", answer.Transcripts)
	}
	return answer.Transcripts[0].Implementation
}

// TestParameterUseCensusCarriesTheSameReachabilityAsTheCallCensus pins the fact
// that closed the read-use hole: a parameter use now records where it sits in
// the body, and it records it from the same walk the call census reads, so a
// `props.children` in dead code can never witness what a `sink(...)` in the same
// position could not.
//
// Every row asserts both censuses at once. A row where they diverge is the
// defect, whatever the individual answers are.
func TestParameterUseCensusCarriesTheSameReachabilityAsTheCallCensus(t *testing.T) {
	cases := []struct {
		name string
		body string
		want typefacts.Reachability
	}{{
		name: "plainStatementIsReachable",
		body: `sink(props.children);`,
		want: typefacts.Reachable,
	}, {
		name: "afterAReturnIsUnreachable",
		body: `if (keys.length) { return; }
  return;
  sink(props.children);`,
		want: typefacts.Unreachable,
	}, {
		name: "afterAThrowIsUnreachable",
		body: `throw new Error("stop");
  sink(props.children);`,
		want: typefacts.Unreachable,
	}, {
		name: "inALiteralFalseBranchIsUnreachable",
		body: `if (false) { sink(props.children); }`,
		want: typefacts.Unreachable,
	}, {
		name: "inALoopBodyIsUnknown",
		body: `for (const key of keys) { sink(props.children); }`,
		want: typefacts.ReachUnknown,
	}, {
		name: "afterAConstBoundInfiniteLoopIsUnknown",
		body: `const ALWAYS = true;
  while (ALWAYS) { sink(1); }
  sink(props.children);`,
		want: typefacts.ReachUnknown,
	}, {
		name: "afterALiteralComparisonInfiniteLoopIsUnknown",
		body: `while (1 === 1) { sink(1); }
  sink(props.children);`,
		want: typefacts.ReachUnknown,
	}, {
		name: "afterATryWhoseFinallyReturnsIsUnreachable",
		body: `try { sink(1); } finally { return; }
  sink(props.children);`,
		want: typefacts.Unreachable,
	}, {
		name: "insideANestedCallableIsStillWalked",
		body: `sink(() => { sink(props.children); });`,
		want: typefacts.Reachable,
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `declare function sink(value: unknown): void;
declare const keys: string[];
export function make(props: { children?: unknown }) {
  ` + testCase.body + `
}
void make;
`
			implementation := exportImplementationForMake(t, source)
			useStart := strings.LastIndex(source, "props.children")
			if useStart < 0 {
				t.Fatal("probe source lost its parameter use")
			}
			var use *typefacts.ParameterUse
			for index, candidate := range implementation.ParameterUses {
				if candidate.Location.StartByte == useStart {
					use = &implementation.ParameterUses[index]
				}
			}
			if use == nil {
				t.Fatalf("use at %d is absent from the census: %#v", useStart, implementation.ParameterUses)
			}
			if use.Reach != testCase.want {
				t.Fatalf("use reach = %q, want %q (use=%#v)", use.Reach, testCase.want, *use)
			}
			// The call wrapping that very use is the same position, so the two
			// censuses have to answer it identically.
			var call *typefacts.ImplementationCall
			for index, candidate := range implementation.Calls {
				if candidate.Location.StartByte <= useStart &&
					useStart < candidate.Location.EndByte &&
					(call == nil || candidate.Location.StartByte > call.Location.StartByte) {
					call = &implementation.Calls[index]
				}
			}
			if call == nil {
				t.Fatalf("no call contains the use at %d: %#v", useStart, implementation.Calls)
			}
			if call.Reach != use.Reach {
				t.Fatalf(
					"call reach = %q but use reach = %q: the two censuses disagree",
					call.Reach, use.Reach,
				)
			}
		})
	}
}

// TestCallCensusStopsAtAFinallyThatReturns pins S3: a `finally` runs on every
// path out of the `try`, and a jump inside it overrides the one it interrupted,
// so nothing after the `try` is reached. Merging the finally block's completion
// into the arms is what says so; discarding it let the statement after the
// construct claim it runs.
func TestCallCensusStopsAtAFinallyThatReturns(t *testing.T) {
	cases := []struct {
		name string
		body string
		want typefacts.Reachability
	}{{
		name: "finallyReturnStopsTheFollowingCall",
		body: `try { sink(0); } finally { return; }`,
		want: typefacts.Unreachable,
	}, {
		name: "finallyReturnStopsTheFollowingCallAcrossALoop",
		body: `try { sink(0); } finally { return; }
  for (const key of keys) { sink(key); }`,
		want: typefacts.Unreachable,
	}, {
		name: "finallyThatCompletesLeavesTheFollowingCallReachable",
		body: `try { sink(0); } finally { sink(1); }`,
		want: typefacts.Reachable,
	}, {
		name: "aReturnInTheTryBlockAloneStillRunsTheFinally",
		body: `try { return; } finally { sink(1); }`,
		want: typefacts.Unreachable,
	}, {
		name: "aFinallyWhoseLoopMayNotCompleteWeakensTheFollowingCall",
		body: `try { sink(0); } finally { while (true) { sink(1); } }`,
		want: typefacts.ReachUnknown,
	}}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := `declare function sink(value: unknown): void;
declare const keys: string[];
export function make(callback: () => void) {
  ` + testCase.body + `
  callback();
}
void make;
`
			implementation := exportImplementationForMake(t, source)
			trailing := strings.LastIndex(source, "callback()")
			for _, call := range implementation.Calls {
				if call.Location.StartByte != trailing {
					continue
				}
				if call.Reach != testCase.want {
					t.Fatalf(
						"call after the try = %q, want %q (calls=%#v)",
						call.Reach, testCase.want, implementation.Calls,
					)
				}
				return
			}
			t.Fatalf("trailing call at %d is absent: %#v", trailing, implementation.Calls)
		})
	}
}

// TestCallCensusRunsAFinallyThatFollowsAReturn pins the other half of the same
// arm: the finally block's *contents* are reached whenever the `try` statement
// was, even when the try block returns. Gating them on the merged answer would
// call a cleanup that provably runs unreachable.
func TestCallCensusRunsAFinallyThatFollowsAReturn(t *testing.T) {
	source := `declare function sink(value: unknown): void;
export function make(callback: () => void) {
  try { return; } finally { sink(1); }
}
void make;
`
	implementation := exportImplementationForMake(t, source)
	cleanup := strings.Index(source, "sink(1)")
	for _, call := range implementation.Calls {
		if call.Location.StartByte == cleanup {
			if call.Reach != typefacts.Reachable {
				t.Fatalf("finally-block call = %q, want reachable (%#v)", call.Reach, call)
			}
			return
		}
	}
	t.Fatalf("finally-block call at %d is absent: %#v", cleanup, implementation.Calls)
}

// The argument half of the same tracer that answers a return site. One entry
// per written argument slot, and every entry a positive fact or nothing: the
// three shapes that trace to nothing here — a slot a spread displaced, a
// non-identifier callee, and a reassignable binding — are refusals to state a
// provenance, never a claim that the slot holds a plain value.
func TestImplementationCallCensusTracesArgumentValueProvenance(t *testing.T) {
	source := `import { createSignal } from "solid-js";
function localSignal(value: unknown): [() => unknown, (next: unknown) => void] {
  return [() => value, () => {}];
}
export function make(
  cb: (...values: unknown[]) => void,
  extra: unknown[],
  options: { storage?: typeof createSignal },
) {
  const [text, setText] = createSignal(1);
  cb(text, ...extra, setText);
  cb(setText);
  const [own] = localSignal(1);
  cb(own);
  const [picked] = (options.storage || createSignal)(1);
  cb(picked);
  let [mutable] = createSignal(1);
  if (extra.length) { [mutable] = createSignal(2); }
  cb(mutable);
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	byStart := map[int]typefacts.ImplementationCall{}
	for _, call := range implementation.Calls {
		byStart[call.Location.StartByte] = call
	}
	call := func(needle string) typefacts.ImplementationCall {
		t.Helper()
		start := strings.Index(source, needle)
		if start < 0 {
			t.Fatalf("needle %q is absent from the fixture", needle)
		}
		found, ok := byStart[start]
		if !ok {
			t.Fatalf("call at %q (byte %d) is absent from the census: %#v", needle, start, implementation.Calls)
		}
		return found
	}
	slot := func(needle string, index int) []typefacts.ImplementationValueSource {
		t.Helper()
		found := call(needle)
		if len(found.ArgumentSources) != len(found.ArgumentParameters) {
			t.Fatalf(
				"call %q has %d traced slots and %d parameter slots, want one entry per written argument",
				needle, len(found.ArgumentSources), len(found.ArgumentParameters),
			)
		}
		if index >= len(found.ArgumentSources) {
			t.Fatalf("call %q has no slot %d: %#v", needle, index, found.ArgumentSources)
		}
		return found.ArgumentSources[index]
	}
	tupleResult := func(sources []typefacts.ImplementationValueSource) typefacts.ImplementationValueSource {
		t.Helper()
		if len(sources) != 1 {
			t.Fatalf("traced sources = %#v, want exactly one", sources)
		}
		source := sources[0]
		if source.Kind != typefacts.ImplementationValueCallResult || len(source.Path) != 0 ||
			len(source.TargetPath) != 1 || source.TargetPath[0].Kind != typefacts.PathSegmentTuple ||
			source.TargetPath[0].Index == nil {
			t.Fatalf("traced source = %#v, want a root-path call result at one tuple slot", source)
		}
		return source
	}

	// An accessor bound by `const [text] = createSignal(...)` and handed to a
	// callback: the exact provenance mechanism A proves a reactive operation
	// input with.
	accessor := tupleResult(slot("cb(text, ...extra, setText)", 0))
	if accessor.TargetName != "createSignal" || accessor.TargetModule != "solid-js" ||
		*accessor.TargetPath[0].Index != 0 || accessor.Target == "" {
		t.Fatalf("accessor argument source = %#v", accessor)
	}

	// Slot identity is the whole point: the setter is slot 1 of the same call,
	// and a consumer comparing roles must be able to tell them apart.
	setter := tupleResult(slot("cb(setText)", 0))
	if setter.TargetName != "createSignal" || *setter.TargetPath[0].Index != 1 {
		t.Fatalf("setter argument source = %#v", setter)
	}

	// The spread displaces slots 1 and 2, so neither is traced even though
	// slot 2 is written as an accessor: the runtime value there is not the one
	// written there. Slot 0 above keeps its exact meaning.
	for _, index := range []int{1, 2} {
		if traced := slot("cb(text, ...extra, setText)", index); len(traced) != 0 {
			t.Fatalf("spread-displaced slot %d = %#v, want nothing traced", index, traced)
		}
	}

	// A locally declared factory is traced, with the empty module that says so.
	// The producer states the syntax; whether `localSignal` is a dialect
	// primitive is not its question, and the empty module is what a consumer
	// refuses on.
	own := tupleResult(slot("cb(own)", 0))
	if own.TargetName != "localSignal" || own.TargetModule != "" || *own.TargetPath[0].Index != 0 {
		t.Fatalf("locally declared factory source = %#v", own)
	}

	// `(options.storage || createSignal)(1)` resolves to no callee symbol, so
	// the binding traces to nothing at all.
	if traced := slot("cb(picked)", 0); len(traced) != 0 {
		t.Fatalf("non-identifier creator callee = %#v, want nothing traced", traced)
	}

	// The tightening: one declaration and never assigned. A `let` binding
	// reassigned in a branch has one declaration and would otherwise trace to
	// the initializer that is not the value.
	if traced := slot("cb(mutable)", 0); len(traced) != 0 {
		t.Fatalf("reassigned binding = %#v, want nothing traced", traced)
	}
}

// Every premise of the identifier arm's binding gate, one row each, so a
// mutation that drops one turns exactly one row green.
//
// The gate is single-assignment, not `const`. A `const` requirement was tried
// and reverted: bundler output across the measured corpus — solid-js 1.9.14's
// own `dist/solid.js` among it — destructures `createSignal` with `let` or
// `var` and never reassigns, and refusing those rows buys no soundness the
// assignment census does not already provide.
func TestArgumentSourcesBindingGateIsSingleAssignmentAndSlotExact(t *testing.T) {
	source := `import { createSignal } from "solid-js";
export function make(cb: (value: unknown) => void, flag: boolean) {
  const [fixed] = createSignal(1);
  cb(fixed);
  const [, setter] = createSignal(2);
  cb(setter);
  let [mutable] = createSignal(3);
  cb(mutable);
  let [reassigned] = createSignal(4);
  if (flag) { [reassigned] = createSignal(5); }
  cb(reassigned);
  var [redeclared] = createSignal(6);
  var [redeclared] = createSignal(7);
  cb(redeclared);
  const [...rest] = createSignal(8);
  cb(rest);
  const [defaulted = 9] = createSignal(10);
  cb(defaulted);
  const [chosen] = flag ? createSignal(11) : createSignal(12);
  cb(chosen);
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	tracedSlot := func(needle string) []typefacts.ImplementationValueSource {
		t.Helper()
		start := strings.Index(source, needle)
		if start < 0 {
			t.Fatalf("needle %q is absent from the fixture", needle)
		}
		for _, call := range implementation.Calls {
			if call.Location.StartByte != start {
				continue
			}
			if len(call.ArgumentSources) != 1 {
				t.Fatalf("call %q has %d traced slots, want one", needle, len(call.ArgumentSources))
			}
			return call.ArgumentSources[0]
		}
		t.Fatalf("call at %q is absent from the census", needle)
		return nil
	}
	tupleSlot := func(needle string) int {
		t.Helper()
		sources := tracedSlot(needle)
		if len(sources) != 1 || len(sources[0].TargetPath) != 1 ||
			sources[0].TargetPath[0].Kind != typefacts.PathSegmentTuple ||
			sources[0].TargetPath[0].Index == nil || sources[0].TargetName != "createSignal" {
			t.Fatalf("call %q traced %#v, want one createSignal tuple slot", needle, sources)
		}
		return *sources[0].TargetPath[0].Index
	}

	// `const`, and `let` that is never assigned, both trace. An omitted
	// element still holds its position, so the setter is slot 1 and not slot 0.
	if got := tupleSlot("cb(fixed)"); got != 0 {
		t.Fatalf("const binding = slot %d, want 0", got)
	}
	if got := tupleSlot("cb(mutable)"); got != 0 {
		t.Fatalf("never-assigned let binding = slot %d, want 0", got)
	}
	if got := tupleSlot("cb(setter)"); got != 1 {
		t.Fatalf("binding after an omitted element = slot %d, want 1", got)
	}

	for _, row := range []struct{ needle, why string }{
		// Assigned somewhere other than its declaration: the initializer is
		// not the value.
		{"cb(reassigned)", "a binding reassigned in a branch"},
		// Two declarations leave no single initializer.
		{"cb(redeclared)", "a redeclared var binding"},
		// `[...rest]` binds the tail array, not slot 0. The arm traced it to
		// slot 0 before this premise existed.
		{"cb(rest)", "a rest element"},
		// `[a = fallback]` is the fallback exactly when slot 0 is undefined,
		// and nothing here observes which value won.
		{"cb(defaulted)", "a defaulted element"},
		// The initializer has to be the call itself.
		{"cb(chosen)", "a conditional initializer"},
	} {
		if traced := tracedSlot(row.needle); len(traced) != 0 {
			t.Fatalf("%s traced %#v, want nothing", row.why, traced)
		}
	}
}

// A traced reference has to be positioned after the declaration it reads.
// `var` hoists, so `cb(hoisted); var [hoisted] = createSignal(1);` reads
// `undefined` at the call -- and `tsc` says nothing about it, which is what
// made the single-assignment gate alone insufficient. The bound is the whole
// `VariableDeclaration`, so a self-reference inside the initializer is refused
// too.
func TestArgumentSourcesRefuseAReferenceBeforeItsDeclaration(t *testing.T) {
	source := `import { createSignal } from "solid-js";
export function make(cb: (value: unknown) => void) {
  cb(hoisted);
  var [hoisted] = createSignal(1);
  const [ordered] = createSignal(2);
  cb(ordered);
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	traced := func(needle string) []typefacts.ImplementationValueSource {
		t.Helper()
		start := strings.Index(source, needle)
		for _, call := range implementation.Calls {
			if call.Location.StartByte == start && len(call.ArgumentSources) == 1 {
				return call.ArgumentSources[0]
			}
		}
		t.Fatalf("call at %q is absent from the census", needle)
		return nil
	}
	if sources := traced("cb(hoisted)"); len(sources) != 0 {
		t.Fatalf("a hoisted reference traced %#v, want nothing", sources)
	}
	if sources := traced("cb(ordered)"); len(sources) != 1 || sources[0].TargetName != "createSignal" {
		t.Fatalf("an ordered reference traced %#v, want the createSignal slot", sources)
	}
}

// The same two shapes on the surface the tracer already served. `[...rest]`
// stated slot 0 for the tail array here too, and a defaulted element stated a
// value the default may have replaced.
func TestReturnValueSourcesRefuseRestAndDefaultedElements(t *testing.T) {
	source := `import { createSignal } from "solid-js";
export function make() {
  const [...rest] = createSignal(1);
  const [defaulted = 2] = createSignal(3);
  const [fixed] = createSignal(4);
  return [rest, defaulted, fixed];
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	flow := implementation.ControlFlow
	if flow == nil || len(flow.Returns) != 1 {
		t.Fatalf("control flow = %#v, want one return site", flow)
	}
	sources := flow.Returns[0].Sources
	if len(sources) != 1 {
		t.Fatalf("return sources = %#v, want only the plain const-bound slot", sources)
	}
	if len(sources[0].Path) != 1 || sources[0].Path[0].Index == nil || *sources[0].Path[0].Index != 2 {
		t.Fatalf("surviving return source = %#v, want the third returned slot", sources[0])
	}
}

// A slot the producer traced nothing for is an empty CBOR array (major type 4,
// length 0 -- `0x80`), never `null` (`0xf6`). The Rust client's field is
// `Vec<Vec<ImplementationValueSource>>`, so a null slot fails the whole
// transcript with "invalid type: null, expected array" -- which is exactly what
// a nil Go slice encoded to before this was pinned, and it refused every
// certification that reached a census.
func TestArgumentSourcesEncodeEmptySlotsAsArrays(t *testing.T) {
	source := `export function make(cb: (value: unknown) => void) {
  cb(1);
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	var call *typefacts.ImplementationCall
	for index := range implementation.Calls {
		if len(implementation.Calls[index].ArgumentSources) == 1 {
			call = &implementation.Calls[index]
			break
		}
	}
	if call == nil {
		t.Fatalf("census = %#v, want one call with one written argument", implementation.Calls)
	}
	if len(call.ArgumentSources[0]) != 0 {
		t.Fatalf("slot 0 traced %#v, want nothing", call.ArgumentSources[0])
	}
	encoded, err := wirecbor.Marshal(call)
	if err != nil {
		t.Fatal(err)
	}
	var decoded map[string]any
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	slots, ok := decoded["argumentSources"].([]any)
	if !ok || len(slots) != 1 {
		t.Fatalf("argumentSources decoded as %#v, want one slot", decoded["argumentSources"])
	}
	if slots[0] == nil {
		t.Fatalf("slot 0 decoded as null; the Rust client refuses the transcript for it")
	}
	if empty, ok := slots[0].([]any); !ok || len(empty) != 0 {
		t.Fatalf("slot 0 decoded as %#v, want an empty array", slots[0])
	}
	if bytes.Contains(encoded, []byte("argumentSources")) {
		index := bytes.Index(encoded, []byte("argumentSources")) + len("argumentSources")
		if encoded[index] != 0x81 || encoded[index+1] != 0x80 {
			t.Fatalf(
				"argumentSources encodes as %#x %#x, want one-element array (0x81) of empty array (0x80)",
				encoded[index], encoded[index+1],
			)
		}
	}

	// A round trip through the wire form keeps the slot list, so a consumer
	// that indexes by written argument sees the same shape the producer built.
	var roundTripped typefacts.ImplementationCall
	if err := wirecbor.Unmarshal(encoded, &roundTripped); err != nil {
		t.Fatal(err)
	}
	if len(roundTripped.ArgumentSources) != 1 || len(roundTripped.ArgumentSources[0]) != 0 {
		t.Fatalf("round-tripped slots = %#v, want one empty slot", roundTripped.ArgumentSources)
	}
}

// The same tightening, on the surface it already served. `ReturnSite.Sources`
// has followed a symbol's first declaration since it existed, so a reassigned
// binding stated a provenance there too; narrowing the arm can only withdraw a
// source, never add one.
func TestReturnValueSourcesRefuseAReassignedBinding(t *testing.T) {
	source := `import { createSignal } from "solid-js";
export function make(flag: boolean) {
  let [mutable] = createSignal(1);
  const [fixed] = createSignal(2);
  if (flag) { [mutable] = createSignal(3); }
  return [mutable, fixed];
}
void make;
`
	implementation := exportImplementationForSolidMake(t, source)
	flow := implementation.ControlFlow
	if flow == nil || len(flow.Returns) != 1 {
		t.Fatalf("control flow = %#v, want one return site", flow)
	}
	sources := flow.Returns[0].Sources
	if len(sources) != 1 {
		t.Fatalf("return sources = %#v, want only the const-bound slot", sources)
	}
	if len(sources[0].Path) != 1 || sources[0].Path[0].Index == nil || *sources[0].Path[0].Index != 1 ||
		sources[0].TargetName != "createSignal" {
		t.Fatalf("surviving return source = %#v, want the slot-1 const binding", sources[0])
	}
}

func exportImplementationForSolidMake(
	t *testing.T,
	source string,
) *typefacts.ExportImplementationTranscript {
	t.Helper()
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{
		"facts.ts": source,
		"solid-js.d.ts": `declare module "solid-js" {
  export function createSignal(value: unknown): [() => unknown, (next: unknown) => void];
}`,
	})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	path := filepath.Join(dir, "facts.ts")
	queryStart := strings.LastIndex(source, "void make") + len("void ")
	implementationStart := strings.Index(source, "function make") + len("function ")
	location := typefacts.Location{Path: path, StartByte: queryStart, EndByte: queryStart + len("make")}
	implementation := typefacts.Location{
		Path: path, StartByte: implementationStart, EndByte: implementationStart + len("make"),
	}
	answer, err := analyzer.ExportValueTranscripts(
		context.Background(),
		[]typefacts.ExportValueDemand{{
			Location:               location,
			ImplementationLocation: &implementation,
			CallableDepth:          1,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 || answer.Transcripts[0].Implementation == nil {
		t.Fatalf("transcripts = %#v, want one implementation transcript", answer.Transcripts)
	}
	return answer.Transcripts[0].Implementation
}
