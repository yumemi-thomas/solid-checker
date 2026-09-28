package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// The `solid-js` stub declares exactly the three bindings ADR 0153's premise
// names, with the signatures `solid-js@2.0.0-rc.9` publishes
// (`types/client/core.d.ts:81`, `:37` of `types/server/core.d.ts` for the
// shared `useContext` shape, `types/client/component.d.ts:73`). The support
// types are reduced: the producer's premise reads only that each is a
// declaration-file binding of that name, never its type.
const contextSolidStub = `export type EffectOptions = { name?: string };
export type SolidElement = unknown;
export type Component<P = {}> = (props: P) => SolidElement;
export type FlowComponent<P = {}> = Component<P & { children?: unknown }>;
export type ContextProviderComponent<T> = FlowComponent<{
    value: T;
}>;
export interface Context<T> extends ContextProviderComponent<T> {
    id: symbol;
    defaultValue: T | undefined;
}
export declare function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;
export declare function useContext<T>(context: Context<T>): T;
export declare function createComponent<T extends Record<string, any>>(Comp: Component<T>, props: T, name?: string): SolidElement;
`

const contextStateTypes = `export interface Location { path: string }
export interface State { location: Location; readonly pending: number; navigate: () => void }
`

func contextProject(t *testing.T, files map[string]string) (typefacts.ExportValueAnalyzer, string) {
	t.Helper()
	dir := t.TempDir()
	all := map[string]string{
		"tsconfig.json":                      `{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}`,
		"node_modules/solid-js/package.json": `{"name":"solid-js","version":"2.0.0-rc.9","types":"index.d.ts"}`,
		"node_modules/solid-js/index.d.ts":   contextSolidStub,
		"state.d.ts":                         contextStateTypes,
	}
	for name, source := range files {
		all[name] = source
	}
	for name, source := range all {
		path := filepath.Join(dir, name)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
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

func contextMemberForms(t *testing.T, analyzer typefacts.ExportValueAnalyzer, path, source, name string) []typefacts.UncensusedInvokingForm {
	t.Helper()
	transcript := implementationTranscriptFor(t, analyzer, path, source, name)
	var forms []typefacts.UncensusedInvokingForm
	for _, form := range transcript.UncensusedInvokingForms {
		if form.Kind == typefacts.UncensusedPropertyAccessUnknownAccessor || form.Kind == typefacts.UncensusedGetAccessor {
			forms = append(forms, form)
		}
	}
	if len(forms) == 0 {
		t.Fatalf("%s: recorded no accessor form (non-vacuity)", name)
	}
	return forms
}

const contextPositive = `import { createContext, useContext, createComponent, type Context } from "solid-js";
import type { State } from "./state";
function invariant<T>(value: T, message: string): T {
  if (value == null) throw new Error(message);
  return value;
}
function createState(): State {
  return { location: { path: "/" }, get pending() { return 1; }, navigate() {} };
}
const Ctx = createContext<State>();
function useOptional(context: Context<State>) {
  try {
    return useContext(context);
  } catch {
    return undefined;
  }
}
const useThing = () => invariant(useContext(Ctx), "missing");
export const useLocation = () => useThing().location;
export function useAlias() { const thing = useThing(); return thing.location; }
export const usePending = () => useThing().pending;
export function Provider(props: { children?: unknown }) {
  const state = createState();
  return createComponent(Ctx, { value: state, get children() { return props.children; } });
}
export function peek() { return useOptional(Ctx); }
`

func TestContextMemberPremiseNamesTheChainTheProvidersAndTheReads(t *testing.T) {
	analyzer, dir := contextProject(t, map[string]string{"positive.ts": contextPositive})
	path := filepath.Join(dir, "positive.ts")
	for _, name := range []string{"useLocation", "useAlias"} {
		forms := contextMemberForms(t, analyzer, path, contextPositive, name)
		if len(forms) != 1 || forms[0].ContextMember == nil {
			t.Fatalf("%s: forms %+v, want one carrying a context-member premise", name, forms)
		}
		premise := forms[0].ContextMember
		if forms[0].SubjectRoot != "" || premise.Member != "location" {
			t.Fatalf("%s: premise %+v beside subject root %q", name, premise, forms[0].SubjectRoot)
		}
		if len(premise.Chain) != 2 || premise.Chain[0].Kind != typefacts.ContextChainResult ||
			premise.Chain[1].Kind != typefacts.ContextChainIdentity || len(premise.Chain[1].Returns) != 1 {
			t.Fatalf("%s: chain %+v, want result (useThing) then identity (invariant)", name, premise.Chain)
		}
		provision := premise.Context
		if provision.Initializer.TargetName != "createContext" || len(provision.Exports) != 0 {
			t.Fatalf("%s: provision %+v", name, provision)
		}
		// The direct read in useThing and the helper's read in useOptional.
		if len(provision.Reads) != 2 || len(provision.Helpers) != 1 {
			t.Fatalf("%s: reads %+v helpers %+v, want two reads and one helper", name, provision.Reads, provision.Helpers)
		}
		if len(provision.Providers) != 1 || provision.Providers[0].Factory == nil ||
			len(provision.Providers[0].Literals) != 1 || provision.Providers[0].Render.TargetName != "createComponent" {
			t.Fatalf("%s: providers %+v", name, provision.Providers)
		}
	}
	// `pending` is a getter of the provided literal: nothing is stated.
	for _, form := range contextMemberForms(t, analyzer, path, contextPositive, "usePending") {
		if form.ContextMember != nil {
			t.Fatalf("usePending: an accessor member was stated: %+v", form.ContextMember)
		}
	}
}

func TestContextMemberPremiseStatesAnEscapeAndRefusesEveryOtherReference(t *testing.T) {
	header := `import { createContext, useContext, createComponent } from "solid-js";
import type { State } from "./state";
function createState(): State { return { location: { path: "/" }, pending: 0, navigate() {} }; }
`
	cases := map[string]struct {
		body   string
		stated bool
		escape string
	}{
		"escaped.ts": {`const Ctx = createContext<State>();
export { Ctx as EscapedContext };
export function Provider() { const state = createState(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, true, "EscapedContext"},
		"exportedDeclaration.ts": {`export const Ctx = createContext<State>();
export function Provider() { const state = createState(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, true, "Ctx"},
		// The router's `createRouterContext` shape: the literal is returned
		// before hoisted helper declarations. Stated, and not exported.
		"hoistedHelpers.ts": {`const Ctx = createContext<State>();
function createHoisted(): State {
  return { location: { path: "/" }, pending: 0, navigate };
  function navigate() {}
}
export function Provider() { const state = createHoisted(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, true, ""},
		"callValue.ts": {`const Ctx = createContext<State>();
export function Provider() { return createComponent(Ctx, { value: createState() }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"directCall.ts": {`const Ctx = createContext<State>();
export function Provider() { const state = createState(); return Ctx({ value: state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"aliased.ts": {`const Ctx = createContext<State>();
const Other = Ctx;
export function Provider() { const state = createState(); return createComponent(Other, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"shorthand.ts": {`const Ctx = createContext<State>();
export const holder = { Ctx };
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"defaulted.ts": {`const fallback: State = { location: { path: "/" }, pending: 0, navigate() {} };
const Ctx = createContext<State>(fallback);
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"written.ts": {`let Ctx = createContext<State>();
export function reset() { Ctx = createContext<State>(); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"parameterValue.ts": {`const Ctx = createContext<State>();
export function Provider(props: { state: State }) { return createComponent(Ctx, { value: props.state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"getterValue.ts": {`const Ctx = createContext<State>();
function createOther(): State { return { get location() { return { path: "/" }; }, pending: 0, navigate() {} }; }
export function Provider() { const state = createOther(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"protoLiteral.ts": {`const Ctx = createContext<State>();
function createProto(): State { return { __proto__: null, location: { path: "/" }, pending: 0, navigate() {} } as State; }
export function Provider() { const state = createProto(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"spreadProps.ts": {`const Ctx = createContext<State>();
export function Provider(extra: object) { const state = createState(); return createComponent(Ctx, { ...extra, value: state }); }
export const reader = () => useContext(Ctx).location;
`, false, ""},
		"shadowed.ts": {`const Ctx = createContext<State>();
export const reader = () => {
  const useContext = (context: unknown) => context as State;
  return useContext(Ctx).location;
};
`, false, ""},
		"joined.ts": {`const Ctx = createContext<State>();
declare const other: State;
export function Provider() { const state = createState(); return createComponent(Ctx, { value: state }); }
const pick = () => useContext(Ctx) || other;
export const reader = () => pick().location;
`, false, ""},
	}
	files := map[string]string{}
	for name, entry := range cases {
		files[name] = header + entry.body
	}
	analyzer, dir := contextProject(t, files)
	for name, entry := range cases {
		source := files[name]
		forms := contextMemberForms(t, analyzer, filepath.Join(dir, name), source, "reader")
		premise := forms[0].ContextMember
		if (premise != nil) != entry.stated {
			t.Fatalf("%s: premise %+v, want stated=%v", name, premise, entry.stated)
		}
		if premise != nil && entry.escape == "" && len(premise.Context.Exports) != 0 {
			t.Fatalf("%s: exports %+v, want none", name, premise.Context.Exports)
		}
		if premise != nil && entry.escape != "" && (len(premise.Context.Exports) != 1 || premise.Context.Exports[0].Name != entry.escape) {
			t.Fatalf("%s: exports %+v, want %q", name, premise.Context.Exports, entry.escape)
		}
	}
}

func TestContextMemberPremiseRefusesAnInstallationThatCouldReplaceTheMember(t *testing.T) {
	base := `import { createContext, useContext, createComponent } from "solid-js";
import type { State } from "./state";
function createState(): State { return { location: { path: "/" }, pending: 0, navigate() {} }; }
const Ctx = createContext<State>();
export function Provider() { const state = createState(); return createComponent(Ctx, { value: state }); }
export const reader = () => useContext(Ctx).location;
`
	cases := map[string]struct {
		extra  string
		stated bool
	}{
		"other key":      {`export function name(f: object) { Object.defineProperty(f, "name", { value: "x" }); }`, true},
		"same key":       {`export function hide(f: object) { Object.defineProperty(f, "location", { get() { return 1; } }); }`, false},
		"computed key":   {`export function put(f: object, k: string) { Object.defineProperty(f, k, { value: 1 }); }`, false},
		"aliased":        {`const define = Object.defineProperty; export function put(f: object) { define(f, "name", { value: 1 }); }`, false},
		"descriptor map": {`export function put(f: object) { Object.defineProperties(f, { location: { value: 1 } }); }`, false},
		// A delete alone leaves the read on Object.prototype; a change of
		// prototype beside it is what could reach code.
		"literal delete":  {`export function drop(f: { location?: unknown }) { delete f.location; }`, true},
		"computed delete": {`export function drop(f: Record<string, unknown>, k: string) { delete f[k]; }`, true},
		"setPrototypeOf":  {`export function swap(f: object, p: object) { Object.setPrototypeOf(f, p); }`, false},
		"reflected":       {`export function swap(f: object, p: object) { Reflect.setPrototypeOf(f, p); }`, false},
		"proto write":     {`export function swap(f: { __proto__?: unknown }, p: object) { f.__proto__ = p; }`, false},
		"proto read":      {`export function peek(f: { __proto__?: unknown }) { return f.__proto__; }`, true},
	}
	for label, entry := range cases {
		analyzer, dir := contextProject(t, map[string]string{"context.ts": base, "extra.ts": entry.extra})
		forms := contextMemberForms(t, analyzer, filepath.Join(dir, "context.ts"), base, "reader")
		if (forms[0].ContextMember != nil) != entry.stated {
			t.Fatalf("%s: premise %+v, want stated=%v", label, forms[0].ContextMember, entry.stated)
		}
	}
}
