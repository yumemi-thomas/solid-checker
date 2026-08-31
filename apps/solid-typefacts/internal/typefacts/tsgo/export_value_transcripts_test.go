package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
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
	if returned.ControlFlow == nil || len(returned.ControlFlow.Returns) != 1 || len(returned.ControlFlow.Returns[0].Captures) != 1 || returned.ControlFlow.Returns[0].Captures[0] != 0 {
		t.Fatalf("returned control flow = %#v, want returned closure capture of parameter 0", returned.ControlFlow)
	}
	retained := answer.Transcripts[2].Implementation
	if retained == nil || len(retained.Calls) != 1 || !retained.Calls[0].Captured {
		t.Fatalf("retained implementation = %#v, want one captured call", retained)
	}
	if retained.ControlFlow == nil || len(retained.ControlFlow.Returns) != 1 || len(retained.ControlFlow.Returns[0].Captures) != 0 {
		t.Fatalf("retained control flow = %#v, want no returned capture authority", retained.ControlFlow)
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
	if captures := returned.ControlFlow.Returns[0].Captures; len(captures) != 0 {
		t.Fatalf("returned captures = %#v, want no proven captures for a nil closure", captures)
	}
}
