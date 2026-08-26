package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

func TestDemandedCallabilityUsesCompilerCallSignatures(t *testing.T) {
	dir := t.TempDir()
	source := `export function plain() {}
export const value = 1;
export const mixed = null as (() => void) | number;
export function overloaded(value: string): string;
export function overloaded(value: number): number;
export function overloaded(value: string | number) { return value; }
export const generic = <T>(value: T) => value;
export class ConstructorOnly {}
export const anyValue: any = plain;
export const unknownValue: unknown = plain;
export const neverValue = null as never;
`
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		name string
		want typefacts.Callability
	}{
		{"plain", typefacts.CallabilityCallable},
		{"value", typefacts.CallabilityNonCallable},
		{"mixed", typefacts.CallabilityMixed},
		{"overloaded", typefacts.CallabilityCallable},
		{"generic", typefacts.CallabilityCallable},
		{"ConstructorOnly", typefacts.CallabilityNonCallable},
		{"anyValue", typefacts.CallabilityUnknown},
		{"unknownValue", typefacts.CallabilityUnknown},
		{"neverValue", typefacts.CallabilityUnknown},
	}
	demands := make([]typefacts.EntityDemand, 0, len(cases))
	for _, testCase := range cases {
		start := strings.Index(source, testCase.name)
		if start < 0 {
			t.Fatalf("%q not found", testCase.name)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(testCase.name),
			},
			Callability: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != len(cases) {
		t.Fatalf("entities = %d, want %d", len(entities), len(cases))
	}
	for index, testCase := range cases {
		if got := entities[index].Callability; got != testCase.want {
			t.Errorf("%s callability = %q, want %q", testCase.name, got, testCase.want)
		}
	}
}

// TestDemandedConstructabilityUsesCompilerConstructSignatures is the exact
// counterpart of TestDemandedCallabilityUsesCompilerCallSignatures, and the
// class rows are why the fact exists: the same declaration is nonCallable
// there and constructable here.
func TestDemandedConstructabilityUsesCompilerConstructSignatures(t *testing.T) {
	dir := t.TempDir()
	origin := `export class Widget {}
export function make() {}
`
	source := `import { Widget, make } from "./origin";
import * as originNamespace from "./origin";
class Local {}
abstract class Abstract { abstract render(): void }
function plain() {}
const value = 1;
interface Factory { (): Widget; new (): Widget }
declare const factory: Factory;
declare const constructorOnly: new () => Widget;
declare const mixedConstruct: (new () => Widget) | number;
declare const anyValue: any;
declare const unknownValue: unknown;
declare const neverValue: never;
declare const middleware: Function;
const aliasedWidget = Widget;
const aliasedMake = make;
export { Local, Abstract, plain, value, Widget as ReWidget, originNamespace, factory, constructorOnly, mixedConstruct, anyValue, unknownValue, neverValue, middleware, aliasedWidget, aliasedMake };
`
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "origin.ts"), []byte(origin), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		// expression is matched at its LAST occurrence, which is the export
		// clause for every re-exported name.
		expression       string
		callability      typefacts.Callability
		constructability typefacts.Constructability
	}{
		// A class: the pair the type system cannot answer alone. The same
		// declaration is nonCallable and constructable.
		{"Local", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		// Abstract construct signatures are not filtered out: an abstract
		// class is still a function object at runtime.
		{"Abstract", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		{"plain", typefacts.CallabilityCallable, typefacts.ConstructabilityNonConstructable},
		{"value", typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		// An interface carrying both signature kinds answers both positively.
		{"factory", typefacts.CallabilityCallable, typefacts.ConstructabilityConstructable},
		{"constructorOnly", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		{"mixedConstruct", typefacts.CallabilityNonCallable, typefacts.ConstructabilityMixed},
		// A namespace object has neither signature kind: an honest negative.
		{"originNamespace", typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		// Aliases and re-exports are transparent, exactly as for callability.
		{"ReWidget", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		{"aliasedWidget", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		{"aliasedMake", typefacts.CallabilityCallable, typefacts.ConstructabilityNonConstructable},
		// No closed type: the fact is the absence of an answer, never a
		// negative one.
		{"anyValue", typefacts.CallabilityUnknown, typefacts.ConstructabilityUnknown},
		{"unknownValue", typefacts.CallabilityUnknown, typefacts.ConstructabilityUnknown},
		{"neverValue", typefacts.CallabilityUnknown, typefacts.ConstructabilityUnknown},
		// lib.es5.d.ts's `Function` interface declares apply/call/bind but no
		// call or construct signature of its own. Constructability answers by
		// the signature it does not have, and that answer is the compiler's:
		// `new` on a Function-typed value is a compile error. Callability does
		// not, because calling one is legal — see
		// TestCallabilityAnswersTheSignatureLessFunctionSupertypeFamily, which
		// owns this family, and TestTheFunctionSupertypeFamilyIsCallableButNot-
		// Constructable, which pins the asymmetry against the compiler's own
		// diagnostics. Kept here so the pair's two halves are read together:
		// this is the one row where they disagree about a single type.
		{"middleware", typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
	}
	demands := make([]typefacts.EntityDemand, 0, len(cases))
	for _, testCase := range cases {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(testCase.expression),
			},
			Callability:      true,
			Constructability: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != len(cases) {
		t.Fatalf("entities = %d, want %d", len(entities), len(cases))
	}
	for index, testCase := range cases {
		if got := entities[index].Constructability; got != testCase.constructability {
			t.Errorf("%s constructability = %s, want %s", testCase.expression, got, testCase.constructability)
		}
		if got := entities[index].Callability; got != testCase.callability {
			t.Errorf("%s callability = %q, want %q", testCase.expression, got, testCase.callability)
		}
	}

	// A class *declaration name* is not the same span as an export
	// specifier: the compiler's type there is the class's instance type,
	// which has no construct signature. Pinned because it is the shape a
	// consumer gets wrong by demanding at the wrong span, and because
	// callability answers nonCallable there too and so hides the difference.
	declarationName := strings.Index(source, "Local")
	instance, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path:      sourcePath,
			StartByte: declarationName,
			EndByte:   declarationName + len("Local"),
		},
		Callability:      true,
		Constructability: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := instance[0].Constructability; got != typefacts.ConstructabilityNonConstructable {
		t.Errorf("class declaration-name constructability = %s, want %s", got, typefacts.ConstructabilityNonConstructable)
	}

	// An undemanded span carries no constructability at all, which is what
	// separates "never asked" from "asked and got no closed answer".
	undemanded, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:    demands[0].Location,
		Callability: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := undemanded[0].Constructability; got.IsPresent() {
		t.Errorf("undemanded constructability = %s, want absent", got)
	}
}

// The measured shapes the fact was written for, reproduced from the published
// artifacts named in solid-checker's precision backlog: a class a bundler
// hides behind an IIFE, a class reached only as a cross-file tuple element
// type, and the two destructuring patterns. None has a class expression a
// syntactic search could find at the exported binding.
func TestConstructabilityAnswersBundlerLoweredClasses(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","moduleResolution":"bundler","allowJs":true},"include":["*.ts","*.js"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	// A cross-package tuple element type, as @tanstack/devtools-utils declares it.
	if err := os.WriteFile(filepath.Join(dir, "utils.ts"), []byte(
		"export declare class DevtoolsCore { mount(): void }\n"+
			"export declare function constructCoreClass(): [typeof DevtoolsCore];\n"+
			"export declare const pair: [typeof DevtoolsCore, number];\n"+
			"export declare const Container: { Inner: typeof DevtoolsCore; label: string };\n",
	), 0o644); err != nil {
		t.Fatal(err)
	}
	// @solidjs/web@2.0.0-rc.1 ships ResponseEnvelope as plain JavaScript, not
	// TypeScript. A .ts transcription of the same shape (a `class` expression
	// cast `as any` to satisfy the compiler) never exercises .js inference,
	// which is the exact path a consumer's real project takes: allowJs is on
	// specifically so this file is checked as authored, without a TypeScript
	// cast smoothing over anything. The file is named distinctly from the
	// "artifact.ts" test source below it on purpose: giving a .js file the
	// same basename as a sibling .ts file makes bundler-mode module
	// resolution conflate "./envelope.js" with "./envelope.ts" (a file that
	// does not even exist here) and, when it does exist as in an earlier
	// version of this fixture, resolve the specifier back to the .ts
	// importer itself, producing a self-referential TS2303 "circular
	// definition of import alias" and answering unknown/unknown for reasons
	// that have nothing to do with .js inference.
	if err := os.WriteFile(filepath.Join(dir, "envelope.js"), []byte(
		"const ENVELOPE = Symbol(\"envelope\");\n"+
			"export const ResponseEnvelope = /* @__PURE__ */ (() => {\n"+
			"  class ResponseEnvelope {}\n"+
			"  ResponseEnvelope.prototype[ENVELOPE] = true;\n"+
			"  return ResponseEnvelope;\n"+
			"})();\n",
	), 0o644); err != nil {
		t.Fatal(err)
	}
	source := `import { constructCoreClass, pair, Container } from "./utils";
import { ResponseEnvelope } from "./envelope.js";
const coreClasses = constructCoreClass();
const TableDevtoolsCore = coreClasses[0];
const { Inner } = Container;
const { label } = Container;
const [Core] = pair;
var Downleveled: any;
(function (Downleveled) { Downleveled[Downleveled["A"] = 0] = "A"; })(Downleveled || (Downleveled = {}));
const AnonymousClassExpr = class {};
export { ResponseEnvelope, TableDevtoolsCore, Inner, label, Core, Downleveled, AnonymousClassExpr };
`
	sourcePath := filepath.Join(dir, "artifact.ts")
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		name             string
		callability      typefacts.Callability
		constructability typefacts.Constructability
	}{
		// @solidjs/web@2.0.0-rc.1 ResponseEnvelope, sourced from a real
		// artifact.js above rather than a .ts transcription: the type must
		// come from checking authored JavaScript, not a cast TypeScript file.
		{"ResponseEnvelope", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		// @tanstack/*-devtools *DevtoolsCore.
		{"TableDevtoolsCore", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		// const { Inner } = Container, a static class member.
		{"Inner", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		// const { label } = Container, the primitive the refusal also cost.
		{"label", typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		// const [Core] = pair, a tuple element whose element type is a class.
		{"Core", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
		// A downleveled enum object stays unanswerable: `any` closes no domain
		// for either fact, so this fact does not rescue it.
		{"Downleveled", typefacts.CallabilityUnknown, typefacts.ConstructabilityUnknown},
		// A bare class EXPRESSION (not a bundler-hidden one): the export's own
		// initializer is a `class {}` expression a syntactic search could in
		// fact find. Pinned as a control alongside the harder shapes above.
		{"AnonymousClassExpr", typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},
	}
	demands := make([]typefacts.EntityDemand, 0, len(cases))
	for _, testCase := range cases {
		start := strings.LastIndex(source, testCase.name)
		if start < 0 {
			t.Fatalf("%q not found", testCase.name)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location: typefacts.Location{
				Path: sourcePath, StartByte: start, EndByte: start + len(testCase.name),
			},
			Callability:      true,
			Constructability: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range cases {
		if got := entities[index].Constructability; got != testCase.constructability {
			t.Errorf("%s constructability = %s, want %s", testCase.name, got, testCase.constructability)
		}
		if got := entities[index].Callability; got != testCase.callability {
			t.Errorf("%s callability = %s, want %s", testCase.name, got, testCase.callability)
		}
	}
}

func TestDemandedRuntimeValueDomainUsesCheckerSemantics(t *testing.T) {
	dir := t.TempDir()
	source := `
interface CallableInterface { (): void }
type CleanupAlias = (() => void) | undefined;
declare const functionValue: () => void;
declare const undefinedValue: undefined;
declare const cleanupUnion: (() => void) | undefined;
declare const callableInterfaceValue: CallableInterface;
declare const overloadedFunction: { (): void; (value: string): number };
declare const aliasedCleanup: CleanupAlias;
declare const callableIntersection: CallableInterface & { tag: string };
declare const optionalHolder: { optionalCleanup?: () => void };
optionalHolder.optionalCleanup;
function constrained<T extends (() => void) | undefined>(boundedCleanup: T) { return boundedCleanup; }
declare const numberValue: number;
declare const nullValue: null;
declare const objectValue: object;
declare const promiseValue: Promise<void>;
declare const callableNumber: (() => void) | number;
declare const undefinedString: undefined | string;
declare const cleanupNull: (() => void) | undefined | null;
declare const objectIntersection: { left: true } & { right: true };
declare const anyValue: any;
declare const unknownValue: unknown;
declare const neverValue: never;
function unconstrained<T>(genericValue: T) { return genericValue; }
declare const recoveryValue: MissingType;
declare function voidCall(): void;
const voidResult = voidCall();
`
	sourcePath := filepath.Join(dir, "domains.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	callable := typefacts.RuntimeValueDomain{MayBeCallable: true}
	undefined := typefacts.RuntimeValueDomain{MayBeUndefined: true}
	cleanup := typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true}
	other := typefacts.RuntimeValueDomain{MayBeOther: true}
	callableOther := typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeOther: true}
	undefinedOther := typefacts.RuntimeValueDomain{MayBeUndefined: true, MayBeOther: true}
	cleanupOther := typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true, MayBeOther: true}
	unknown := typefacts.RuntimeValueDomain{
		MayBeCallable: true, MayBeUndefined: true, MayBeOther: true, Unknown: true,
	}
	cases := []struct {
		name string
		want typefacts.RuntimeValueDomain
	}{
		{"functionValue", callable},
		{"undefinedValue", undefined},
		{"cleanupUnion", cleanup},
		{"callableInterfaceValue", callable},
		{"overloadedFunction", callable},
		{"aliasedCleanup", cleanup},
		{"callableIntersection", callable},
		{"optionalCleanup", cleanup},
		{"boundedCleanup", cleanup},
		// A void result is undefined at runtime and is trusted like any other
		// declared return type; the bivariance hole is documented, not modelled.
		{"voidResult", undefined},
		{"numberValue", other},
		{"nullValue", other},
		{"objectValue", other},
		{"promiseValue", other},
		{"callableNumber", callableOther},
		{"undefinedString", undefinedOther},
		{"cleanupNull", cleanupOther},
		{"objectIntersection", other},
		{"anyValue", unknown},
		{"unknownValue", unknown},
		{"neverValue", typefacts.RuntimeValueDomain{}},
		{"genericValue", unknown},
		{"recoveryValue", unknown},
	}
	demands := make([]typefacts.EntityDemand, len(cases))
	for index, testCase := range cases {
		start := strings.LastIndex(source, testCase.name)
		if start < 0 {
			t.Fatalf("%q not found", testCase.name)
		}
		demands[index] = typefacts.EntityDemand{
			Location: typefacts.Location{
				Path: sourcePath, StartByte: start, EndByte: start + len(testCase.name),
			},
			RuntimeValueDomain: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != len(cases) {
		t.Fatalf("entities = %d, want %d", len(entities), len(cases))
	}
	for index, testCase := range cases {
		if entities[index].RuntimeValueDomain == nil {
			t.Errorf("%s runtime value domain is absent", testCase.name)
			continue
		}
		if got := *entities[index].RuntimeValueDomain; got != testCase.want {
			t.Errorf("%s runtime value domain = %+v, want %+v", testCase.name, got, testCase.want)
		}
	}
}

func TestDemandedPrimitiveValueDomainUsesCheckerSemanticsAtExactSpans(t *testing.T) {
	dir := t.TempDir()
	source := `
type TextAlias = string;
type BrandedText = string & { readonly __brand: unique symbol };
declare const textValue: TextAlias;
declare const brandedText: BrandedText;
declare const numberValue: number;
declare const finiteValue: 42;
declare const finiteUnion: 1 | 2 | "three";
declare const overflowValue: number;
declare const booleanValue: boolean;
declare const bigintValue: bigint;
declare const symbolValue: symbol;
declare const nullValue: null;
declare const undefinedValue: undefined;
declare const objectValue: { value: string };
declare const functionValue: () => void;
declare const safeUnion: string | boolean | null | undefined;
declare const unsafeUnion: string | bigint | object;
function constrained<T extends string | boolean>(bounded: T) { return bounded; }
function unconstrained<T>(generic: T) { return generic; }
declare const anyValue: any;
declare const unknownValue: unknown;
declare const neverValue: never;
declare const recoveryValue: MissingType;
declare function voidCall(): void;
const voidResult = voidCall();
textValue;
brandedText;
numberValue;
finiteValue;
finiteUnion;
overflowValue;
booleanValue;
bigintValue;
symbolValue;
nullValue;
undefinedValue;
objectValue;
functionValue;
safeUnion;
unsafeUnion;
anyValue;
unknownValue;
neverValue;
recoveryValue;
voidResult;
`
	sourcePath := filepath.Join(dir, "primitive-domains.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	domain := typefacts.NewPrimitiveValueDomain
	stringDomain := domain(true, false, false, false, false, false, false, false, false)
	booleanDomain := domain(false, false, true, false, false, false, false, false, false)
	objectDomain := domain(false, false, false, false, false, false, false, true, false)
	unknownDomain := unknownPrimitiveValueDomain()
	cases := []struct {
		name string
		want typefacts.PrimitiveValueDomain
	}{
		{"textValue", stringDomain},
		{"brandedText", stringDomain},
		{"numberValue", domain(false, true, false, false, false, false, false, false, false)},
		{"finiteValue", domain(false, true, false, false, false, false, false, false, true)},
		{"finiteUnion", domain(true, true, false, false, false, false, false, false, true)},
		{"overflowValue", domain(false, true, false, false, false, false, false, false, false)},
		{"booleanValue", booleanDomain},
		{"bigintValue", domain(false, false, false, true, false, false, false, false, false)},
		{"symbolValue", domain(false, false, false, false, true, false, false, false, false)},
		{"nullValue", domain(false, false, false, false, false, true, false, false, false)},
		{"undefinedValue", domain(false, false, false, false, false, false, true, false, false)},
		{"objectValue", objectDomain},
		{"functionValue", objectDomain},
		{"safeUnion", domain(true, false, true, false, false, true, true, false, false)},
		{"unsafeUnion", domain(true, false, false, true, false, false, false, true, false)},
		{"bounded", domain(true, false, true, false, false, false, false, false, false)},
		{"generic", unknownDomain},
		{"anyValue", unknownDomain},
		{"unknownValue", unknownDomain},
		{"neverValue", domain(false, false, false, false, false, false, false, false, false)},
		{"recoveryValue", unknownDomain},
		{"voidResult", domain(false, false, false, false, false, false, true, false, false)},
	}
	demands := make([]typefacts.EntityDemand, 0, len(cases)+1)
	for _, testCase := range cases {
		start := strings.LastIndex(source, "\n"+testCase.name+";")
		if start >= 0 {
			start++
		} else {
			start = strings.LastIndex(source, testCase.name+";")
		}
		if start < 0 {
			t.Fatalf("%q not found", testCase.name)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location:             typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.name)},
			PrimitiveValueDomain: true,
		})
	}
	textStart := strings.LastIndex(source, "textValue;")
	demands = append(demands, typefacts.EntityDemand{
		Location:             typefacts.Location{Path: sourcePath, StartByte: textStart, EndByte: textStart + len("textValue") - 1},
		PrimitiveValueDomain: true,
	})
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range cases {
		if !entities[index].PrimitiveValueDomain.IsPresent() {
			t.Errorf("%s primitive value domain is absent", testCase.name)
			continue
		}
		if got := entities[index].PrimitiveValueDomain; got != testCase.want {
			t.Errorf("%s primitive value domain = %+v, want %+v", testCase.name, got, testCase.want)
		}
	}
	if entities[len(cases)].PrimitiveValueDomain.IsPresent() {
		t.Fatal("non-exact primitive-domain demand unexpectedly answered")
	}
}

func TestPrimitiveLiteralCandidatesAreExactCompilerInhabitants(t *testing.T) {
	dir := t.TempDir()
	source := `
declare const literalUnion: "alpha" | "beta" | -1 | 2 | false | true;
declare const broadUnion: string | 3;
declare const branded: "tag" & { readonly brand: unique symbol };
declare enum Choice { A = "a", B = 2 }
declare const enumValue: Choice;
function constrained<T extends "left" | "right">(value: T) { value; }
function generic<T>(value: T) { value; }
literalUnion;
broadUnion;
branded;
enumValue;
`
	sourcePath := filepath.Join(dir, "literal-candidates.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)
	names := []string{"literalUnion", "broadUnion", "branded", "enumValue", "value"}
	demands := make([]typefacts.EntityDemand, 0, len(names)+1)
	for _, name := range names {
		start := strings.LastIndex(source, name+";")
		if start < 0 {
			t.Fatalf("%q not found", name)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location:                   typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(name)},
			PrimitiveLiteralCandidates: true,
		})
	}
	// The earlier constrained parameter, not the unconstrained one selected by LastIndex.
	constrainedStart := strings.Index(source, "value: T")
	demands = append(demands, typefacts.EntityDemand{
		Location:                   typefacts.Location{Path: sourcePath, StartByte: constrainedStart, EndByte: constrainedStart + len("value")},
		PrimitiveLiteralCandidates: true,
	})
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	literal := func(kind typefacts.PrimitiveLiteralKind, text string, number float64, boolean bool) typefacts.PrimitiveLiteralCandidate {
		return typefacts.PrimitiveLiteralCandidate{Kind: kind, String: text, Number: number, Boolean: boolean}
	}
	if got, want := entities[0].PrimitiveLiteralCandidates, []typefacts.PrimitiveLiteralCandidate{
		literal(typefacts.PrimitiveLiteralBoolean, "", 0, false),
		literal(typefacts.PrimitiveLiteralBoolean, "", 0, true),
		literal(typefacts.PrimitiveLiteralNumber, "", -1, false),
		literal(typefacts.PrimitiveLiteralNumber, "", 2, false),
		literal(typefacts.PrimitiveLiteralString, "alpha", 0, false),
		literal(typefacts.PrimitiveLiteralString, "beta", 0, false),
	}; !slices.Equal(got, want) {
		t.Fatalf("literalUnion candidates = %#v, want %#v", got, want)
	}
	if got, want := entities[1].PrimitiveLiteralCandidates, []typefacts.PrimitiveLiteralCandidate{
		literal(typefacts.PrimitiveLiteralNumber, "", 3, false),
	}; !slices.Equal(got, want) {
		t.Fatalf("broadUnion candidates = %#v, want %#v", got, want)
	}
	for index, name := range []string{"branded", "enumValue", "generic"} {
		if len(entities[index+2].PrimitiveLiteralCandidates) != 0 {
			t.Fatalf("%s unexpectedly has candidates %#v", name, entities[index+2].PrimitiveLiteralCandidates)
		}
	}
	if got := entities[5].PrimitiveLiteralCandidates; len(got) != 2 || got[0].String != "left" || got[1].String != "right" {
		t.Fatalf("constrained candidates = %#v", got)
	}
}

func TestDemandedCallResultDomainUsesExactCallSpans(t *testing.T) {
	dir := t.TempDir()
	source := `declare function makeCount(): number;
declare function makeThunk(): () => void;
declare function make(): (() => void) | undefined;
declare const handlers: Array<() => number>;
declare const maybe: (() => void) | undefined;
declare const anyFactory: any;
declare function takesNumber(value: number): number;
declare function onCleanup(callback: () => void): void;
declare function makeNested(): (value: number) => number;
declare function makeBuilder(): { build(): number };
declare class Foo { bar(): number; }

onCleanup(() => { return makeCount(); });
const thunkResult = makeThunk();
const unionResult = make();
const indexedResult = handlers[0]();
const optionalResult = maybe?.();
const anyResult = anyFactory();
const recoveryResult = takesNumber("wrong");
const nestedResult = makeNested()(2);
const builderResult = makeBuilder().build();
const newResult = new Foo().bar();
`
	sourcePath := filepath.Join(dir, "calls.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	want := []struct {
		name string
		want typefacts.RuntimeValueDomain
	}{
		{"makeCount()", typefacts.RuntimeValueDomain{MayBeOther: true}},
		{"makeThunk()", typefacts.RuntimeValueDomain{MayBeCallable: true}},
		{"make()", typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true}},
		{"handlers[0]()", typefacts.RuntimeValueDomain{MayBeOther: true}},
		{"maybe?.()", typefacts.RuntimeValueDomain{MayBeUndefined: true}},
		{"anyFactory()", typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true, MayBeOther: true, Unknown: true}},
		{"takesNumber(" + `"wrong"` + ")", typefacts.RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true, MayBeOther: true, Unknown: true}},
		{"makeNested()(2)", typefacts.RuntimeValueDomain{MayBeOther: true}},
		{"makeBuilder().build()", typefacts.RuntimeValueDomain{MayBeOther: true}},
		{"new Foo().bar()", typefacts.RuntimeValueDomain{MayBeOther: true}},
	}
	demands := make([]typefacts.EntityDemand, len(want))
	for index, testCase := range want {
		start := strings.LastIndex(source, testCase.name)
		if start < 0 {
			t.Fatalf("%q not found", testCase.name)
		}
		demands[index] = typefacts.EntityDemand{
			Location: typefacts.Location{
				Path: sourcePath, StartByte: start, EndByte: start + len(testCase.name),
			},
			CallResultDomain: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range want {
		if entities[index].CallResultDomain == nil {
			t.Errorf("%s call-result domain is absent", testCase.name)
			continue
		}
		if got := *entities[index].CallResultDomain; got != testCase.want {
			t.Errorf("%s call-result domain = %+v, want %+v", testCase.name, got, testCase.want)
		}
	}

	callStart := strings.LastIndex(source, "makeCount()")
	callEnd := callStart + len("makeCount()")
	identifierStart := strings.Index(source, "thunkResult")
	absence := []typefacts.EntityDemand{
		{Location: typefacts.Location{Path: sourcePath, StartByte: callStart, EndByte: callStart + len("makeCount")}, CallResultDomain: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: callStart, EndByte: callEnd + 1}, CallResultDomain: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: callStart + 1, EndByte: callEnd}, CallResultDomain: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: identifierStart, EndByte: identifierStart + len("thunkResult")}, CallResultDomain: true},
	}
	absenceEntities, err := semantic.SemanticEntities(context.Background(), absence)
	if err != nil {
		t.Fatal(err)
	}
	for index, entity := range absenceEntities {
		if entity.CallResultDomain != nil {
			t.Errorf("absence demand %d unexpectedly returned %+v", index, entity.CallResultDomain)
		}
	}

	// Keep the entity span identical while explicitly pinning the legacy domain
	// to the callee query span. The new field must remain tied to the full exact
	// call span and describe the produced number instead.
	calleeLocation := typefacts.Location{Path: sourcePath, StartByte: callStart, EndByte: callStart + len("makeCount")}
	distinction := []typefacts.EntityDemand{
		{Location: typefacts.Location{Path: sourcePath, StartByte: callStart, EndByte: callEnd}, QueryLocation: &calleeLocation, RuntimeValueDomain: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: callStart, EndByte: callEnd}, CallResultDomain: true},
	}
	distinctEntities, err := semantic.SemanticEntities(context.Background(), distinction)
	if err != nil {
		t.Fatal(err)
	}
	if got := distinctEntities[0].RuntimeValueDomain; got == nil || !got.MayBeCallable || got.MayBeOther {
		t.Fatalf("callee runtime domain = %+v, want callable only", got)
	}
	if got := distinctEntities[1].CallResultDomain; got == nil || !got.MayBeOther || got.MayBeCallable {
		t.Fatalf("call-result runtime domain = %+v, want other only", got)
	}
}

func TestDemandedConstantValueUsesExactSpansAndBoundedFolding(t *testing.T) {
	dir := t.TempDir()
	source := `import { IMPORTED } from "./dep";
const CONST_A = "a";
let LET_A = "a";
enum Choice { Value = 7 }
class Holder { static readonly Value = "held"; }
function f(): string { return "dynamic"; }
function parameterValue(parameter: string) { return parameter; }
const SELF = SELF;

"<p>Hello</p>" + "<p>world!</p>";
"plain";
42;
1 + 2;
-3;
+4;
` + "`foo`;\n`foo${CONST_A}`;\n" + `
("cast") as string;
("satisfied") satisfies string;
("nonnull")!;
CONST_A + "b";
IMPORTED + "d";
LET_A + "b";
Choice.Value;
Holder.Value;
1 + "a";
f() + "a";
SELF;
`
	sourcePath := filepath.Join(dir, "constants.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "dep.ts"), []byte(`export const IMPORTED = "importe";`), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	tests := []struct {
		expression string
		want       *typefacts.ConstantValue
	}{
		{`"<p>Hello</p>" + "<p>world!</p>"`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "<p>Hello</p><p>world!</p>"}},
		{`"plain"`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "plain"}},
		{`42`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: 42}},
		{`1 + 2`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: 3}},
		{`-3`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: -3}},
		{`+4`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: 4}},
		{"`foo`", &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "foo"}},
		{"`foo${CONST_A}`", nil},
		{`("cast") as string`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "cast"}},
		{`("satisfied") satisfies string`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "satisfied"}},
		{`("nonnull")!`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "nonnull"}},
		{`CONST_A + "b"`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "ab"}},
		{`IMPORTED + "d"`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "imported"}},
		{`LET_A + "b"`, nil},
		{`Choice.Value`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueNumber, Number: 7}},
		{`Holder.Value`, &typefacts.ConstantValue{Kind: typefacts.ConstantValueString, String: "held"}},
		{`1 + "a"`, nil},
		{`f() + "a"`, nil},
		{`parameter`, nil},
		{`SELF`, nil},
	}
	demands := make([]typefacts.EntityDemand, len(tests))
	for index, testCase := range tests {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands[index] = typefacts.EntityDemand{
			Location:      typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.expression)},
			ConstantValue: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range tests {
		got := entities[index].ConstantValue
		if testCase.want == nil {
			if got != nil {
				t.Errorf("%s constant value = %+v, want absent", testCase.expression, got)
			}
			continue
		}
		if got == nil || *got != *testCase.want {
			t.Errorf("%s constant value = %+v, want %+v", testCase.expression, got, testCase.want)
		}
	}

	concatenation := tests[0].expression
	outerStart := strings.LastIndex(source, concatenation)
	leading := `"<p>Hello</p>"`
	subNodeEntities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{
		{Location: typefacts.Location{Path: sourcePath, StartByte: outerStart, EndByte: outerStart + len(leading)}, ConstantValue: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: outerStart, EndByte: outerStart + len(concatenation) - 1}, ConstantValue: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := subNodeEntities[0].ConstantValue; got == nil || got.Kind != typefacts.ConstantValueString || got.String != "<p>Hello</p>" {
		t.Fatalf("leading literal constant value = %+v", got)
	}
	if got := subNodeEntities[1].ConstantValue; got != nil {
		t.Fatalf("non-exact concatenation span returned %+v", got)
	}
	undemanded, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:       demands[0].Location,
		TypeDescriptor: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := undemanded[0].ConstantValue; got != nil {
		t.Fatalf("undemanded constant value returned %+v", got)
	}
}

func TestResolvedCallDistinguishesValidRecoveryAndUnresolved(t *testing.T) {
	dir := t.TempDir()
	source := `function takesNumber(value: number): string { return String(value); }
const valid = takesNumber(1);
const recovery = takesNumber("wrong");
const unresolved = takesNumber;
`
	sourcePath := filepath.Join(dir, "calls.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	demandAt := func(needle string) typefacts.EntityDemand {
		start := strings.Index(source, needle)
		if start < 0 {
			t.Fatalf("%q not found", needle)
		}
		return typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
			ResolvedCall: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{
		demandAt("takesNumber(1)"),
		demandAt(`takesNumber("wrong")`),
		demandAt("unresolved = takesNumber"),
	})
	if err != nil {
		t.Fatal(err)
	}
	want := []typefacts.ResolvedCallValidity{
		typefacts.ResolvedCallValid,
		typefacts.ResolvedCallRecovery,
		typefacts.ResolvedCallUnresolved,
	}
	for index, validity := range want {
		if entities[index].ResolvedCall == nil {
			t.Fatalf("entity %d has no resolved-call fact", index)
		}
		if got := entities[index].ResolvedCall.Validity; got != validity {
			t.Errorf("entity %d validity = %q, want %q", index, got, validity)
		}
	}
	if mapping := entities[0].ResolvedCall.Arguments; len(mapping) != 1 ||
		mapping[0].Status != typefacts.ArgumentMappingResolved {
		t.Errorf("valid call mappings = %+v", mapping)
	}
	if mapping := entities[1].ResolvedCall.Arguments; len(mapping) != 1 ||
		mapping[0].Status != typefacts.ArgumentMappingUnresolved ||
		mapping[0].Unresolved != typefacts.ArgumentMappingRecoverySignature {
		t.Errorf("recovery call mappings = %+v", mapping)
	}
	if mapping := entities[2].ResolvedCall.Arguments; len(mapping) != 0 {
		t.Errorf("unresolved non-call mappings = %+v, want empty", mapping)
	}
}

func TestResolvedCallUsesOutermostChainedCall(t *testing.T) {
	dir := t.TempDir()
	source := `declare function factory(): (value: number) => string;
const result = factory()(1);
`
	sourcePath := filepath.Join(dir, "calls.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)
	start := strings.Index(source, "factory()(1)")
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len("factory()(1)")},
		ResolvedCall: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != 1 || entities[0].ResolvedCall == nil {
		t.Fatalf("resolved chained call entities = %+v", entities)
	}
	if got := len(entities[0].ResolvedCall.Arguments); got != 1 {
		t.Fatalf("resolved chained call arguments = %d, want outer call's 1 argument", got)
	}
}

func TestResolvedCallValidityPreservesDiagnosticFallbacks(t *testing.T) {
	dir := t.TempDir()
	source := `function takesNumber(value: number): string { return String(value); }
function generic<T extends string>(value: T): T { return value; }
declare const maybe: ((value: number) => void) | undefined;
declare const either: ((value: string) => void) | ((value: number) => void);
const notCallable = 1;
takesNumber(1);
takesNumber("wrong");
takesNumber();
generic<number>(1);
maybe(1);
either(true);
notCallable();
`
	sourcePath := filepath.Join(dir, "calls.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		needle string
		want   typefacts.ResolvedCallValidity
	}{
		{needle: "takesNumber(1);", want: typefacts.ResolvedCallValid},
		{needle: `takesNumber("wrong")`, want: typefacts.ResolvedCallRecovery},
		{needle: "takesNumber();", want: typefacts.ResolvedCallRecovery},
		{needle: "generic<number>(1)", want: typefacts.ResolvedCallRecovery},
		{needle: "maybe(1)", want: typefacts.ResolvedCallRecovery},
		{needle: "either(true)", want: typefacts.ResolvedCallRecovery},
		{needle: "notCallable()", want: typefacts.ResolvedCallRecovery},
	}
	demands := make([]typefacts.EntityDemand, len(cases))
	for index, testCase := range cases {
		start := strings.LastIndex(source, testCase.needle)
		if start < 0 {
			t.Fatalf("%q not found", testCase.needle)
		}
		spanNeedle := strings.TrimSuffix(testCase.needle, ";")
		demands[index] = typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(spanNeedle),
			},
			ResolvedCall:     true,
			CallResultDomain: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range cases {
		call := entities[index].ResolvedCall
		if call == nil || call.Validity != testCase.want {
			t.Errorf("%q resolved call = %+v, want validity %q", testCase.needle, call, testCase.want)
		}
		domain := entities[index].CallResultDomain
		if index == 0 {
			if domain == nil || !domain.MayBeOther || domain.Unknown {
				t.Errorf("%q call-result domain = %+v, want known non-function", testCase.needle, domain)
			}
		} else if domain == nil || !domain.Unknown {
			t.Errorf("%q call-result domain = %+v, want unknown for recovery", testCase.needle, domain)
		}
	}
}

// A package-contract consumer can validate a candidate runtime witness by
// adding one synthetic call to the configured project and demanding the
// existing resolvedCall fact. This deliberately exercises contextual generic
// inference: reconstructing TableOptions from rendered type text would lose
// both the inferred feature set and the compiler's assignability rules.
func TestResolvedCallValidatesTableShapedSyntheticWitnesses(t *testing.T) {
	dir := t.TempDir()
	source := `type RowData = unknown;
type FeatureMap = Record<string, { createTable(table: unknown): void }>;
type ValidateFeatureSlots<TFeatures extends FeatureMap> = "missing" extends keyof TFeatures ? { missing: never } : {};
interface TableOptions<TFeatures extends FeatureMap, TData extends RowData> {
	features: TFeatures & ValidateFeatureSlots<TFeatures>;
	data: TData[];
	columns: Array<{ accessorKey: keyof TData }>;
}
declare function createTable<TFeatures extends FeatureMap, TData extends RowData>(options: TableOptions<TFeatures, TData>): unknown;
declare const stockFeatures: {
	core: { createTable(table: unknown): void };
};
createTable({ features: stockFeatures, data: [], columns: [] });
createTable({ features: { core: {} }, data: [], columns: [] });
createTable({ features: stockFeatures, data: {} });
createTable(null as never);
`
	sourcePath := filepath.Join(dir, "witness.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		needle string
		want   typefacts.ResolvedCallValidity
	}{
		{`createTable({ features: stockFeatures, data: [], columns: [] })`, typefacts.ResolvedCallValid},
		{`createTable({ features: { core: {} }, data: [], columns: [] })`, typefacts.ResolvedCallRecovery},
		{`createTable({ features: stockFeatures, data: {} })`, typefacts.ResolvedCallRecovery},
	}
	demands := make([]typefacts.EntityDemand, len(cases))
	for index, testCase := range cases {
		start := strings.Index(source, testCase.needle)
		if start < 0 {
			t.Fatalf("%q not found", testCase.needle)
		}
		demands[index] = typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.needle)},
			ResolvedCall: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range cases {
		call := entities[index].ResolvedCall
		if call == nil || call.Validity != testCase.want {
			t.Errorf("%q resolved call = %+v, want validity %q", testCase.needle, call, testCase.want)
		}
		if call != nil && call.Validity == typefacts.ResolvedCallValid && call.Arguments[0].Parameter.ObjectShape != nil {
			t.Errorf("%q returned an undemanded parameter object shape", testCase.needle)
		}
	}
	shapeNeedle := `createTable(null as never)`
	shapeStart := strings.Index(source, shapeNeedle)
	shapes, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:             typefacts.Location{Path: sourcePath, StartByte: shapeStart, EndByte: shapeStart + len(shapeNeedle)},
		ResolvedCall:         true,
		ParameterObjectShape: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	parameter := shapes[0].ResolvedCall.Arguments[0].Parameter
	if parameter == nil || parameter.ObjectShape == nil {
		t.Fatalf("TableOptions parameter object shape = %+v", parameter)
	}
	wantProperties := []typefacts.ObjectConstructionProperty{
		{Name: "columns", Witness: typefacts.ConstructionWitnessEmptyArray},
		{Name: "data", Witness: typefacts.ConstructionWitnessEmptyArray},
		{Name: "features", Witness: typefacts.ConstructionWitnessEmptyObject},
	}
	if !slices.Equal(parameter.ObjectShape.RequiredProperties, wantProperties) {
		t.Fatalf("TableOptions required properties = %+v, want %+v", parameter.ObjectShape.RequiredProperties, wantProperties)
	}
}

func TestParameterObjectShapeLeavesUnprovenWitnessesUnknown(t *testing.T) {
	dir := t.TempDir()
	source := `declare function unsafe(options: {
	callback: () => void;
	tuple: [string];
	primitive: string;
	optional?: number;
}): void;
unsafe(null as never);
`
	sourcePath := filepath.Join(dir, "witness.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	needle := `unsafe(null as never)`
	start := strings.Index(source, needle)
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(needle)},
		ResolvedCall: true, ParameterObjectShape: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	properties := entities[0].ResolvedCall.Arguments[0].Parameter.ObjectShape.RequiredProperties
	want := []typefacts.ObjectConstructionProperty{
		{Name: "callback", Witness: typefacts.ConstructionWitnessUnknown},
		{Name: "primitive", Witness: typefacts.ConstructionWitnessUnknown},
		{Name: "tuple", Witness: typefacts.ConstructionWitnessUnknown},
	}
	if !slices.Equal(properties, want) {
		t.Fatalf("unsafe required properties = %+v, want %+v", properties, want)
	}
}

func TestResolvedCallIdentifiesSelectedOverloadAndMapsArguments(t *testing.T) {
	dir := t.TempDir()
	source := `function select(value: string, callback: (value: string) => void): void;
function select(value: number, callback: (value: number) => number): number;
function select(value: string | number, callback: (value: never) => unknown) {
	return callback(value as never);
}
const selected = select(1, value => value);
`
	sourcePath := filepath.Join(dir, "calls.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	callStart := strings.LastIndex(source, "select(")
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path:      sourcePath,
			StartByte: callStart,
			EndByte:   callStart + len("select(1, value => value)"),
		},
		ResolvedCall: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	call := entities[0].ResolvedCall
	if call == nil {
		t.Fatal("resolved call fact is missing")
	}
	if call.Kind != typefacts.CallKindCall {
		t.Fatalf("call kind = %q, want call", call.Kind)
	}
	if call.Validity != typefacts.ResolvedCallValid {
		t.Fatalf("validity = %q, want valid", call.Validity)
	}
	if call.Declaration == nil {
		t.Fatal("selected overload declaration is missing")
	}
	secondOverload := strings.Index(source[strings.Index(source, "\n")+1:], "select") + strings.Index(source, "\n") + 1
	if got := call.Declaration.Location.StartByte; got != secondOverload {
		t.Fatalf("selected declaration starts at %d, want second overload at %d", got, secondOverload)
	}
	if call.Declaration.Symbol == "" || call.Declaration.Name != "select" ||
		call.Declaration.Kind != "FunctionDeclaration" {
		t.Fatalf("selected declaration identity = %+v", call.Declaration)
	}
	if len(call.Arguments) != 2 {
		t.Fatalf("argument mappings = %d, want 2", len(call.Arguments))
	}
	for index, mapping := range call.Arguments {
		if mapping.ArgumentIndex != index || mapping.Status != typefacts.ArgumentMappingResolved ||
			mapping.Parameter == nil || mapping.Parameter.Index != index {
			t.Fatalf("argument %d mapping = %+v", index, mapping)
		}
		if mapping.Parameter.Symbol == "" || mapping.Parameter.Declaration == nil ||
			mapping.Parameter.TypeDescriptor == nil {
			t.Fatalf("argument %d parameter facts = %+v", index, mapping.Parameter)
		}
	}
	if got := call.Arguments[0].Parameter.Callability; got != typefacts.CallabilityNonCallable {
		t.Errorf("value parameter callability = %q, want nonCallable", got)
	}
	if got := call.Arguments[1].Parameter.Callability; got != typefacts.CallabilityCallable {
		t.Errorf("callback parameter callability = %q, want callable", got)
	}
}

func TestResolvedCallOwnerIdentityDistinguishesSameNamedMethods(t *testing.T) {
	dir := t.TempDir()
	source := `interface CustomStorage { getItem(key: string): string }
interface CustomEventTarget { removeEventListener(type: string, listener: () => void): void }
interface CustomArray { push(value: number): number }
interface CustomFunction { bind(thisArg: unknown): void }
declare const customStorage: CustomStorage;
declare const customTarget: CustomEventTarget;
declare const customArray: CustomArray;
declare const customFunction: CustomFunction;
declare const storage: Storage;
declare const target: EventTarget;
declare const array: number[];
declare const fn: Function;
storage.getItem("key");
customStorage.getItem("key");
target.removeEventListener("event", () => {});
customTarget.removeEventListener("event", () => {});
array.push(1);
customArray.push(1);
fn.bind(undefined);
customFunction.bind(undefined);
`
	sourcePath := filepath.Join(dir, "owners.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	calls := []string{
		`storage.getItem("key")`,
		`customStorage.getItem("key")`,
		`target.removeEventListener("event", () => {})`,
		`customTarget.removeEventListener("event", () => {})`,
		`array.push(1)`,
		`customArray.push(1)`,
		`fn.bind(undefined)`,
		`customFunction.bind(undefined)`,
	}
	demands := make([]typefacts.EntityDemand, 0, len(calls))
	for _, call := range calls {
		start := strings.Index(source, call)
		demands = append(demands, typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(call)},
			ResolvedCall: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}

	wantQualified := []string{
		"Storage.getItem", "CustomStorage.getItem",
		"EventTarget.removeEventListener", "CustomEventTarget.removeEventListener",
		"Array.push", "CustomArray.push",
		"Function.bind", "CustomFunction.bind",
	}
	for index, want := range wantQualified {
		call := entities[index].ResolvedCall
		if call == nil || call.Declaration == nil {
			t.Fatalf("%s declaration is missing", calls[index])
		}
		if got := call.Declaration.QualifiedName; got != want {
			t.Errorf("%s qualified identity = %q, want %q", calls[index], got, want)
		}
		wantStandardLibrary := index%2 == 0
		if got := call.Declaration.StandardLibrary; got != wantStandardLibrary {
			t.Errorf("%s standardLibrary = %t, want %t", calls[index], got, wantStandardLibrary)
		}
		if index%2 == 0 && call.Declaration.Symbol == entities[index+1].ResolvedCall.Declaration.Symbol {
			t.Errorf("%s and %s share declaration identity %q", calls[index], calls[index+1], call.Declaration.Symbol)
		}
	}
}

func TestResolvedConstructionMapsRestArguments(t *testing.T) {
	dir := t.TempDir()
	source := `class Box {
	constructor(callback: (value: string) => void, ...labels: string[]) {}
}
const box = new Box(value => {}, "first", "second");
`
	sourcePath := filepath.Join(dir, "construct.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	start := strings.Index(source, "new Box")
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path: sourcePath, StartByte: start, EndByte: start + len(`new Box(value => {}, "first", "second")`),
		},
		ResolvedCall: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	call := entities[0].ResolvedCall
	if call == nil || call.Validity != typefacts.ResolvedCallValid {
		t.Fatalf("construction = %+v", call)
	}
	if call.Kind != typefacts.CallKindConstruct {
		t.Fatalf("construction kind = %q, want construct", call.Kind)
	}
	if call.Declaration == nil || call.Declaration.QualifiedName != "Box.constructor" {
		t.Fatalf("constructor declaration = %+v", call.Declaration)
	}
	if len(call.Arguments) != 3 {
		t.Fatalf("argument mappings = %d, want 3", len(call.Arguments))
	}
	if call.Arguments[0].Parameter == nil ||
		call.Arguments[0].Parameter.Callability != typefacts.CallabilityCallable ||
		call.Arguments[0].Parameter.Rest {
		t.Errorf("callback mapping = %+v", call.Arguments[0])
	}
	for _, index := range []int{1, 2} {
		mapping := call.Arguments[index]
		if mapping.Parameter == nil || mapping.Parameter.Index != 1 || !mapping.Parameter.Rest {
			t.Errorf("rest argument %d mapping = %+v", index, mapping)
		}
	}
}

func TestArgumentMappingsUseGenericSubstitutionAndRejectAmbiguousSpread(t *testing.T) {
	dir := t.TempDir()
	source := `function generic<T>(value: T, callback?: (value: T) => T): T {
	return callback ? callback(value) : value;
}
function pair(first: number, second: string): void {}
const pairArguments: [number, string] = [1, "two"];
generic(1, value => value);
pair(...pairArguments);
`
	sourcePath := filepath.Join(dir, "mapping.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	demandCall := func(text string) typefacts.EntityDemand {
		start := strings.LastIndex(source, text)
		return typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(text)},
			ResolvedCall: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{
		demandCall("generic(1, value => value)"),
		demandCall("pair(...pairArguments)"),
	})
	if err != nil {
		t.Fatal(err)
	}

	generic := entities[0].ResolvedCall
	if generic == nil || len(generic.Arguments) != 2 {
		t.Fatalf("generic call = %+v", generic)
	}
	valueParameter := generic.Arguments[0].Parameter
	callbackParameter := generic.Arguments[1].Parameter
	if valueParameter == nil || valueParameter.TypeDescriptor == nil ||
		valueParameter.TypeDescriptor.Text != "number" {
		t.Errorf("instantiated value parameter = %+v", valueParameter)
	}
	if callbackParameter == nil || !callbackParameter.Optional ||
		callbackParameter.Callability != typefacts.CallabilityMixed ||
		callbackParameter.TypeDescriptor == nil ||
		callbackParameter.TypeDescriptor.Text != "((value: number) => number) | undefined" {
		t.Errorf("instantiated callback parameter = %+v, type = %q", callbackParameter, callbackParameter.TypeDescriptor.Text)
	}

	spread := entities[1].ResolvedCall
	if spread == nil || len(spread.Arguments) != 1 {
		t.Fatalf("spread call = %+v", spread)
	}
	if mapping := spread.Arguments[0]; mapping.Status != typefacts.ArgumentMappingUnresolved ||
		mapping.Unresolved != typefacts.ArgumentMappingSpreadArgument || mapping.Parameter != nil {
		t.Errorf("spread mapping = %+v", mapping)
	}
}

func TestResolvedCallsReuseCompilerIdenticalTypeDescriptors(t *testing.T) {
	dir := t.TempDir()
	source := `function first(value: number): number { return value; }
function second(value: number): number { return value; }
first(1);
second(2);
`
	sourcePath := filepath.Join(dir, "descriptor-cache.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	demands := make([]typefacts.EntityDemand, 0, 2)
	for _, text := range []string{"first(1)", "second(2)"} {
		start := strings.LastIndex(source, text)
		demands = append(demands, typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(text)},
			ResolvedCall: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	first := entities[0].ResolvedCall.Arguments[0].Parameter.TypeDescriptor
	second := entities[1].ResolvedCall.Arguments[0].Parameter.TypeDescriptor
	if first == nil || second == nil {
		t.Fatalf("parameter descriptors are missing: first=%+v second=%+v", first, second)
	}
	if first != second {
		t.Fatalf("compiler-identical number types used distinct descriptors: %p and %p", first, second)
	}
}

func TestResolvedCallHandlesCallConstructAndIntersectionSignatures(t *testing.T) {
	dir := t.TempDir()
	source := `interface Callable {
	(callback: () => void): void;
}
interface Constructable {
	new (callback: () => void): object;
}
type Intersected = {
	(value: string): string;
} & {
	(value: number): number;
};
declare const callable: Callable;
declare const constructable: Constructable;
declare const intersected: Intersected;
callable(() => {});
new constructable(() => {});
intersected(1);
`
	sourcePath := filepath.Join(dir, "signatures.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	calls := []string{`callable(() => {})`, `new constructable(() => {})`, `intersected(1)`}
	demands := make([]typefacts.EntityDemand, 0, len(calls))
	for _, call := range calls {
		start := strings.LastIndex(source, call)
		demands = append(demands, typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(call)},
			ResolvedCall: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}

	want := []struct {
		kind      typefacts.CallKind
		qualified string
		declKind  string
	}{
		{typefacts.CallKindCall, "Callable.call", "CallSignature"},
		{typefacts.CallKindConstruct, "Constructable.construct", "ConstructSignature"},
		{typefacts.CallKindCall, "Intersected.call", "CallSignature"},
	}
	for index, expected := range want {
		call := entities[index].ResolvedCall
		if call == nil || call.Validity != typefacts.ResolvedCallValid ||
			call.Kind != expected.kind || call.Declaration == nil ||
			call.Declaration.QualifiedName != expected.qualified ||
			call.Declaration.Kind != expected.declKind {
			t.Errorf("%s fact = %+v declaration = %+v, want %+v", calls[index], call, call.Declaration, expected)
			continue
		}
		if len(call.Arguments) != 1 || call.Arguments[0].Status != typefacts.ArgumentMappingResolved {
			t.Errorf("%s mappings = %+v", calls[index], call.Arguments)
		}
	}
}

func TestResolvedUnionCallDerivesExhaustiveTargetCandidates(t *testing.T) {
	dir := t.TempDir()
	// The two implementations carry distinguishable literal return types:
	// structurally identical function types are subtype-reduced out of a
	// union by the compiler itself, which leaves the single selected
	// declaration fact rather than a candidate set.
	implsSource := `export function implA(value: string): "a" {
	return "a";
}
export function implB(value: string): "b" {
	return "b";
}
`
	source := `import { implA, implB } from "./impls";
declare const cond: boolean;
const dispatch = cond ? implA : implB;
export const direct = dispatch("value");
declare const pair: [typeof implA, typeof implB];
declare const index: number;
export const computed = pair[index]("value");
class Left {
	read(): string {
		return "left";
	}
}
class Right {
	read(): number {
		return 2;
	}
}
declare const union: Left | Right;
export const method = union.read();
interface Shape {
	(value: string): string;
}
declare const shaped: typeof implA | Shape;
export const structural = shaped("value");
declare const generic: typeof implA | (<T>(value: T) => T);
export const open = generic("value");
export const broken = dispatch("value", "extra");
`
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	implsPath := filepath.Join(dir, "impls.ts")
	if err := os.WriteFile(implsPath, []byte(implsSource), 0o644); err != nil {
		t.Fatal(err)
	}
	sourcePath := filepath.Join(dir, "union-targets.ts")
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	calls := []string{
		`dispatch("value")`,
		`pair[index]("value")`,
		`union.read()`,
		`shaped("value")`,
		`generic("value")`,
		`dispatch("value", "extra")`,
	}
	demands := make([]typefacts.EntityDemand, 0, len(calls))
	for _, call := range calls {
		start := strings.Index(source, call)
		if start < 0 {
			t.Fatalf("call %q not found", call)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(call)},
			ResolvedCall: true,
		})
	}
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}

	assertCandidates := func(index int, wantKind string, wantQualified []string, wantPath string) {
		t.Helper()
		call := entities[index].ResolvedCall
		if call == nil || call.Validity != typefacts.ResolvedCallValid {
			t.Fatalf("%s call = %+v", calls[index], call)
		}
		if call.Declaration != nil {
			t.Errorf("%s guessed one declaration %+v", calls[index], call.Declaration)
		}
		targets := call.Targets
		if targets == nil || !targets.Exhaustive {
			t.Fatalf("%s targets = %+v, want an exhaustive set", calls[index], targets)
		}
		if len(targets.Candidates) != len(wantQualified) {
			t.Fatalf("%s candidates = %+v, want %d", calls[index], targets.Candidates, len(wantQualified))
		}
		seen := map[typefacts.SymbolID]bool{}
		for candidateIndex, candidate := range targets.Candidates {
			if candidate.Symbol == "" || seen[candidate.Symbol] {
				t.Errorf("%s candidate %d symbol = %q, want distinct non-empty identities",
					calls[index], candidateIndex, candidate.Symbol)
			}
			seen[candidate.Symbol] = true
			if candidate.Kind != wantKind {
				t.Errorf("%s candidate %d kind = %q, want %q", calls[index], candidateIndex, candidate.Kind, wantKind)
			}
			if candidate.QualifiedName != wantQualified[candidateIndex] {
				t.Errorf("%s candidate %d qualified name = %q, want %q",
					calls[index], candidateIndex, candidate.QualifiedName, wantQualified[candidateIndex])
			}
			if candidate.Location.Path != wantPath {
				t.Errorf("%s candidate %d path = %q, want %q",
					calls[index], candidateIndex, candidate.Location.Path, wantPath)
			}
			if candidateIndex > 0 {
				previous := targets.Candidates[candidateIndex-1].Location
				if previous.Path > candidate.Location.Path ||
					(previous.Path == candidate.Location.Path && previous.StartByte > candidate.Location.StartByte) {
					t.Errorf("%s candidates are not deterministically ordered: %+v", calls[index], targets.Candidates)
				}
			}
		}
	}
	// A conditional union of two exact cross-file function declarations is a
	// proven two-candidate dispatch, whether the callee is an identifier or a
	// dynamically indexed tuple slot.
	assertCandidates(0, "FunctionDeclaration", []string{"implA", "implB"}, implsPath)
	assertCandidates(1, "FunctionDeclaration", []string{"implA", "implB"}, implsPath)
	// Same-named methods keep their own class identities.
	assertCandidates(2, "MethodDeclaration", []string{"Left.read", "Right.read"}, sourcePath)
	for index, label := range map[int]string{
		3: "structural interface constituent",
		4: "generic constituent",
		5: "recovery call",
	} {
		call := entities[index].ResolvedCall
		if call == nil {
			t.Fatalf("%s: no resolved call", label)
		}
		if call.Targets != nil {
			t.Errorf("%s emitted target candidates %+v, want none", label, call.Targets)
		}
	}
	if entities[5].ResolvedCall.Validity != typefacts.ResolvedCallRecovery {
		t.Errorf("broken call validity = %q, want recovery", entities[5].ResolvedCall.Validity)
	}
}

func TestResolvedUnionCallDoesNotGuessOneConstituentDeclaration(t *testing.T) {
	dir := t.TempDir()
	source := `interface Left {
	(value: string): string;
}
interface Right {
	(value: string): number;
}
declare const union: Left | Right;
const value = union("value");
`
	sourcePath := filepath.Join(dir, "union.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.LastIndex(source, `union("value")`)
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(
		context.Background(),
		[]typefacts.EntityDemand{{
			Location: typefacts.Location{
				Path: sourcePath, StartByte: start, EndByte: start + len(`union("value")`),
			},
			ResolvedCall: true,
		}},
	)
	if err != nil {
		t.Fatal(err)
	}
	call := entities[0].ResolvedCall
	if call == nil || call.Validity != typefacts.ResolvedCallValid {
		t.Fatalf("union call = %+v", call)
	}
	if call.Declaration != nil {
		t.Errorf("union call guessed declaration %+v", call.Declaration)
	}
	if len(call.Arguments) != 1 ||
		call.Arguments[0].Status != typefacts.ArgumentMappingUnresolved ||
		call.Arguments[0].Unresolved != typefacts.ArgumentMappingCompositeSignature {
		t.Errorf("union argument mappings = %+v", call.Arguments)
	}
}

func TestResolvedCallRejectsStaleDeclarationNodesAfterUpdate(t *testing.T) {
	dir := t.TempDir()
	configPath := filepath.Join(dir, "tsconfig.json")
	declarationPath := filepath.Join(dir, "library.ts")
	callPath := filepath.Join(dir, "consumer.ts")
	originalDeclaration := "export function invoke(callback: () => void) { callback(); }\n"
	updatedDeclaration := "// shifted declaration\n" + originalDeclaration
	callSource := "import { invoke } from \"./library\";\ninvoke(() => {});\n"
	for path, source := range map[string]string{
		configPath:      `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`,
		declarationPath: originalDeclaration,
		callPath:        callSource,
	} {
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	opened, err := OpenProject(context.Background(), configPath, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)
	callStart := strings.LastIndex(callSource, "invoke(")
	demand := typefacts.EntityDemand{
		Location: typefacts.Location{
			Path:      callPath,
			StartByte: callStart,
			EndByte:   callStart + len("invoke(() => {})"),
		},
		ResolvedCall: true,
	}
	if _, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{demand}); err != nil {
		t.Fatal(err)
	}
	if _, err := opened.Update(context.Background(), []typefacts.FileChange{{
		Path: declarationPath, Version: 1, Source: []byte(updatedDeclaration),
	}}); err != nil {
		t.Fatal(err)
	}

	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{demand})
	if err != nil {
		t.Fatal(err)
	}
	call := entities[0].ResolvedCall
	if call == nil || call.Declaration == nil || len(call.Arguments) != 1 ||
		call.Arguments[0].Parameter == nil || call.Arguments[0].Parameter.Declaration == nil {
		t.Fatalf("updated call fact = %+v", call)
	}
	wantDeclarationStart := strings.Index(updatedDeclaration, "invoke")
	if got := call.Declaration.Location.StartByte; got != wantDeclarationStart {
		t.Errorf("selected declaration starts at stale byte %d, want current byte %d", got, wantDeclarationStart)
	}
	wantParameterStart := strings.Index(updatedDeclaration, "callback")
	if got := call.Arguments[0].Parameter.Declaration.Location.StartByte; got != wantParameterStart {
		t.Errorf("parameter declaration starts at stale byte %d, want current byte %d", got, wantParameterStart)
	}
}

func TestReferenceSpaceAndCanonicalRuntimeIdentity(t *testing.T) {
	dir := t.TempDir()
	write := func(name, source string) string {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	write("runtime.ts", `export interface JSX { node: unknown }
export function Portal() {}
export class Both {}
export type Shared = { typeOnly: true };
export const Shared = () => 1;
`)
	write("named.ts", `export { Portal as NamedPortal } from "./runtime";
`)
	write("star.ts", `export * from "./named";
`)
	write("local.ts", `import { Portal } from "./runtime";
export { Portal as LocalPortal };
`)
	if err := os.MkdirAll(filepath.Join(dir, "node_modules", "runtime-package"), 0o755); err != nil {
		t.Fatal(err)
	}
	write("node_modules/runtime-package/package.json", `{"name":"runtime-package","exports":{"./subpath":"./subpath.d.ts"}}`)
	write("node_modules/runtime-package/subpath.d.ts", `export { Portal as SubpathPortal } from "../../runtime";`)
	consumerSource := `import { JSX, Portal, Portal as Unused, Both } from "./runtime";
import type { JSX as TypeOnlyJSX } from "./runtime";
import { Shared } from "./runtime";
import { NamedPortal } from "./star";
import { LocalPortal } from "./local";
import { SubpathPortal } from "runtime-package/subpath";
type Element = TypeOnlyJSX;
Portal();
type BothType = Both;
new Both();
NamedPortal();
LocalPortal();
SubpathPortal();
type SharedType = Shared;
Shared();
`
	consumerPath := write("consumer.ts", consumerSource)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)
	demandName := func(name string) typefacts.EntityDemand {
		start := strings.Index(consumerSource, name)
		return typefacts.EntityDemand{
			Location: typefacts.Location{Path: consumerPath, StartByte: start, EndByte: start + len(name)},
			Symbol:   true, ReferenceSpace: true, RuntimeIdentity: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{
		demandName("JSX"),
		demandName("Portal"),
		demandName("Both"),
		demandName("NamedPortal"),
		demandName("LocalPortal"),
		demandName("SubpathPortal"),
		demandName("TypeOnlyJSX"),
		demandName("Unused"),
		demandName("Shared"),
	})
	if err != nil {
		t.Fatal(err)
	}
	wantSpaces := []typefacts.ReferenceSpace{
		typefacts.ReferenceSpaceNeither,
		typefacts.ReferenceSpaceValue,
		typefacts.ReferenceSpaceBoth,
		typefacts.ReferenceSpaceValue,
		typefacts.ReferenceSpaceValue,
		typefacts.ReferenceSpaceValue,
		typefacts.ReferenceSpaceType,
		typefacts.ReferenceSpaceNeither,
		typefacts.ReferenceSpaceBoth,
	}
	for index, want := range wantSpaces {
		if got := entities[index].ReferenceSpace; got != want {
			t.Errorf("entity %d reference space = %q, want %q", index, got, want)
		}
	}
	if entities[0].RuntimeIdentity != "" {
		t.Errorf("type-only JSX runtime identity = %q, want empty", entities[0].RuntimeIdentity)
	}
	if entities[1].RuntimeIdentity == "" {
		t.Fatal("Portal has no runtime identity")
	}
	if entities[3].RuntimeIdentity != entities[1].RuntimeIdentity {
		t.Errorf("reexport identity = %q, want Portal identity %q", entities[3].RuntimeIdentity, entities[1].RuntimeIdentity)
	}
	if entities[6].RuntimeIdentity != "" {
		t.Errorf("type-only import runtime identity = %q, want empty", entities[6].RuntimeIdentity)
	}
	for _, index := range []int{4, 5, 7} {
		if entities[index].RuntimeIdentity != entities[1].RuntimeIdentity {
			t.Errorf("alias %d identity = %q, want Portal identity %q", index, entities[index].RuntimeIdentity, entities[1].RuntimeIdentity)
		}
	}
	if entities[8].RuntimeIdentity == "" {
		t.Fatal("merged type/value symbol has no runtime identity")
	}
}

func TestReferenceSpaceClassifiesQualifiedTypeNames(t *testing.T) {
	dir := t.TempDir()
	write := func(name, contents string) string {
		t.Helper()
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
		return path
	}
	write("tsconfig.json", `{
		"compilerOptions": {
			"module": "esnext",
			"moduleResolution": "bundler",
			"strict": true
		},
		"include": ["*.ts"]
	}`)
	write("runtime.ts", `
		export namespace JSX {
			export interface CSSProperties {
				color?: string;
			}
			export interface Element {
				node: unknown;
			}
		}

		export function Portal() {}

		export namespace Namespace {
			export namespace Type {
				export interface Member {
					value: unknown;
				}
			}
		}

		export namespace Shared {
			export interface Type {
				value: unknown;
			}
			export const runtime = 1;
		}
	`)
	source := `
		import { JSX, Portal, Namespace, Shared } from "./runtime";

		type Style = JSX.CSSProperties;
		type Element = JSX.Element;
		type Deep = Namespace.Type.Member;
		type SharedType = Shared.Type;

		Shared.runtime;
		Portal();
	`
	sourcePath := write("consumer.ts", source)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	demands := make([]typefacts.EntityDemand, 0, 4)
	for _, name := range []string{"JSX", "Portal", "Namespace", "Shared"} {
		start := strings.Index(source, name)
		demands = append(demands, typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(name),
			},
			Symbol:         true,
			ReferenceSpace: true,
		})
	}

	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatalf("SemanticEntities: %v", err)
	}

	want := map[string]typefacts.ReferenceSpace{
		"JSX":       typefacts.ReferenceSpaceType,
		"Portal":    typefacts.ReferenceSpaceValue,
		"Namespace": typefacts.ReferenceSpaceType,
		"Shared":    typefacts.ReferenceSpaceBoth,
	}
	for i, demand := range demands {
		name := source[demand.Location.StartByte:demand.Location.EndByte]
		if got := entities[i].ReferenceSpace; got != want[name] {
			t.Errorf("%s referenceSpace = %q, want %q", name, got, want[name])
		}
	}
}

func TestDemandedArrayShapeUsesCompilerArrayPredicateAtExactSpans(t *testing.T) {
	dir := t.TempDir()
	source := `type Handlers = [(data: number, event: MouseEvent) => void, number];
interface SafeArray<T> extends Array<T> {}

declare const aliasedTuple: Handlers;
declare const bareTuple: [(d: number) => void, number];
declare const readonlyTuple: readonly [string, number];
declare const arrayGeneric: Array<string>;
declare const readonlyArrayGeneric: ReadonlyArray<string>;
declare const suffixArray: string[];
declare const functionArray: ((n: number) => void)[];
declare const arrayReturningFunction: () => string[];
declare const plainFunction: (event: MouseEvent) => void;
declare const plainString: string;
declare const safeArrayValue: SafeArray<number>;
declare const maybeTuple: Handlers | undefined;
declare const anyValue: any;
declare const neverValue: never;
function genericSubject<T>(genericValue: T) { genericValue; }

aliasedTuple;
bareTuple;
readonlyTuple;
arrayGeneric;
readonlyArrayGeneric;
suffixArray;
functionArray;
arrayReturningFunction;
plainFunction;
plainString;
safeArrayValue;
maybeTuple;
anyValue;
neverValue;
[(n: number) => n * n, 2];
`
	sourcePath := filepath.Join(dir, "shapes.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	tests := []struct {
		expression string
		want       typefacts.ArrayShape
	}{
		// An alias renders as its own name and defeats every text test; the
		// compiler predicate sees through it to the tuple. This is the case the
		// fact exists for.
		{`aliasedTuple`, typefacts.ArrayShapeArray},
		{`bareTuple`, typefacts.ArrayShapeArray},
		{`readonlyTuple`, typefacts.ArrayShapeArray},
		{`arrayGeneric`, typefacts.ArrayShapeArray},
		{`readonlyArrayGeneric`, typefacts.ArrayShapeArray},
		{`suffixArray`, typefacts.ArrayShapeArray},
		// An array of functions and a function returning an array render with the
		// same trailing `[]`. Asking the type, not the text, separates them
		// without a second callability fact.
		{`functionArray`, typefacts.ArrayShapeArray},
		{`arrayReturningFunction`, typefacts.ArrayShapeNotArray},
		{`plainFunction`, typefacts.ArrayShapeNotArray},
		{`plainString`, typefacts.ArrayShapeNotArray},
		// Array-*like* is deliberately not enough: its author chose an interface
		// over an array.
		{`safeArrayValue`, typefacts.ArrayShapeNotArray},
		{`maybeTuple`, typefacts.ArrayShapeMixed},
		{`anyValue`, typefacts.ArrayShapeUnknown},
		{`neverValue`, typefacts.ArrayShapeUnknown},
		{`genericValue`, typefacts.ArrayShapeUnknown},
		{`[(n: number) => n * n, 2]`, typefacts.ArrayShapeArray},
	}
	demands := make([]typefacts.EntityDemand, len(tests))
	for index, testCase := range tests {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands[index] = typefacts.EntityDemand{
			Location:   typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.expression)},
			ArrayShape: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range tests {
		if got := entities[index].ArrayShape; got != testCase.want {
			t.Errorf("%s array shape = %q, want %q", testCase.expression, got, testCase.want)
		}
	}

	// A span that is not exactly one expression yields no field at all, and an
	// undemanded fact stays absent. Both are distinct from ArrayShapeUnknown:
	// the producer did not look.
	literal := `[(n: number) => n * n, 2]`
	literalStart := strings.LastIndex(source, literal)
	boundary, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{
		{Location: typefacts.Location{Path: sourcePath, StartByte: literalStart, EndByte: literalStart + len(literal) - 1}, ArrayShape: true},
		{Location: typefacts.Location{Path: sourcePath, StartByte: literalStart, EndByte: literalStart + len(literal)}, TypeDescriptor: true},
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := boundary[0].ArrayShape; got != "" {
		t.Errorf("non-exact span array shape = %q, want absent", got)
	}
	if got := boundary[1].ArrayShape; got != "" {
		t.Errorf("undemanded array shape = %q, want absent", got)
	}
}

func TestDemandedTupleShapeDescribesSlotsAndFirstElement(t *testing.T) {
	dir := t.TempDir()
	source := `declare const pair: [(data: number, event: MouseEvent) => void, number];
declare const overArity: [(a: number, b: MouseEvent, c: string) => void, number];
declare const optionalArity: [(a: number, b: MouseEvent, c?: string) => void, number];
declare const restArity: [(...args: unknown[]) => void, number];
declare const numbers: [number, number, number];
declare const single: [(event: MouseEvent) => void];
declare const optionalTail: [(event: MouseEvent) => void, number?];
declare const restTail: [(event: MouseEvent) => void, ...number[]];
declare const spreadOnly: [...((event: MouseEvent) => void)[]];
declare const roPair: readonly [(event: MouseEvent) => void, number];
type Handlers = [(data: number, event: MouseEvent) => void, number];
declare const aliased: Handlers;
declare const plainArray: ((event: MouseEvent) => void)[];
declare const readonlyArray: ReadonlyArray<(event: MouseEvent) => void>;
declare const maybePair: Handlers | undefined;
type OtherHandlers = [(data: string, event: MouseEvent) => void, string];
declare const bothPairs: Handlers | OtherHandlers;
declare const unionOverArity: Handlers | [(a: number, b: MouseEvent, c: string) => void, number];
declare const unionBadHead: Handlers | [number, number];
declare const unionRest: Handlers | [(e: MouseEvent) => void, ...number[]];
declare const unionFunction: Handlers | ((e: MouseEvent) => void);
declare const unionArray: Handlers | number[];
declare const notArray: (event: MouseEvent) => void;
declare const anyValue: any;
declare const empty: [];
declare const untypedSlot: [Function];
declare const untypedUnion: [Function] | [() => void];

pair;
overArity;
optionalArity;
restArity;
numbers;
single;
optionalTail;
restTail;
spreadOnly;
roPair;
aliased;
plainArray;
readonlyArray;
maybePair;
bothPairs;
unionOverArity;
unionBadHead;
unionRest;
unionFunction;
unionArray;
notArray;
anyValue;
empty;
untypedSlot;
untypedUnion;
`
	sourcePath := filepath.Join(dir, "tuples.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	tests := []struct {
		expression string
		want       *typefacts.TupleShape
	}{
		{`pair`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		// The first slot is callable but demands more arguments than a caller
		// with two will supply, which callability alone cannot express.
		{`overArity`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 3}},
		// Optional and rest parameters lower the requirement, matching
		// assignability.
		{`optionalArity`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		{`restArity`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable}},
		// A tuple whose first slot is not callable. Structurally a tuple, but
		// nothing a numbered-member interface expecting a function would accept.
		{`numbers`, &typefacts.TupleShape{FixedLength: 3, ExactLength: 3, ExactLengthKnown: true, ElementZero: typefacts.CallabilityNonCallable}},
		{`single`, &typefacts.TupleShape{FixedLength: 1, ExactLength: 1, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 1}},
		// An optional slot still counts toward fixedLength, matching the compiler.
		{`optionalTail`, &typefacts.TupleShape{FixedLength: 2, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 1}},
		{`restTail`, &typefacts.TupleShape{FixedLength: 1, HasRest: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 1}},
		{`roPair`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 1}},
		// The alias is transparent, exactly as it is to arrayShape.
		{`aliased`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		{`empty`, &typefacts.TupleShape{FixedLength: 0, ExactLengthKnown: true, ElementZero: typefacts.CallabilityUnknown}},
		// Arrays have a number index signature, not fixed slots. This is the
		// distinction arrayShape collapses and the duplicate it left open.
		{`plainArray`, nil},
		{`readonlyArray`, nil},
		// The compiler normalizes a spread-only tuple to the array type it is
		// equivalent to, so this is not a tuple by the time we see it. That is
		// TypeScript's reduction, not ours, and it is why no case here produces
		// a zero fixed length with a rest tail.
		{`spreadOnly`, nil},
		// A union answers with the meet of its constituents. A nullish
		// constituent carries no structure and is skipped, so an optional pair
		// still describes the pair it is when present.
		{`maybePair`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		{`bothPairs`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		// The meet takes the strictest demand: a caller must satisfy whichever
		// constituent it gets.
		{`unionOverArity`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 3}},
		// One head is not callable, so the union's is not provably callable.
		{`unionBadHead`, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityMixed, ElementZeroMinimumParameters: 2}},
		// Slots are kept only where both have them: the rest tail is not shared,
		// so the fixed length drops to the shorter one.
		{`unionRest`, &typefacts.TupleShape{FixedLength: 1, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		// A constituent that is not a tuple voids the whole answer -- there is no
		// shape every value of this type has.
		{`unionFunction`, nil},
		{`unionArray`, nil},
		{`notArray`, nil},
		{`anyValue`, nil},
		// The signature-less Function-supertype family reaches elementZero
		// exactly as it reaches callability: the slot is callable, but no
		// signature can be read from it.
		{`untypedSlot`, &typefacts.TupleShape{FixedLength: 1, ExactLength: 1, ExactLengthKnown: true, ElementZero: typefacts.CallabilityUntypedCallable}},
		// meetTupleShapes mirrors callabilityOfType's own union rung here: a
		// slot that is callable in one constituent and untypedCallable in the
		// other meets to the weaker of the two rather than falling to mixed.
		{`untypedUnion`, &typefacts.TupleShape{FixedLength: 1, ExactLength: 1, ExactLengthKnown: true, ElementZero: typefacts.CallabilityUntypedCallable}},
	}
	demands := make([]typefacts.EntityDemand, len(tests))
	for index, testCase := range tests {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands[index] = typefacts.EntityDemand{
			Location:   typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.expression)},
			TupleShape: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range tests {
		got := entities[index].TupleShape
		if testCase.want == nil {
			if got != nil {
				t.Errorf("%s tuple shape = %+v, want absent", testCase.expression, got)
			}
			continue
		}
		if got == nil || *got != *testCase.want {
			t.Errorf("%s tuple shape = %+v, want %+v", testCase.expression, got, testCase.want)
		}
	}

	undemanded, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:   demands[0].Location,
		ArrayShape: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := undemanded[0].TupleShape; got != nil {
		t.Errorf("undemanded tuple shape = %+v, want absent", got)
	}
	if got := undemanded[0].ArrayShape; got != typefacts.ArrayShapeArray {
		t.Errorf("array shape of a tuple = %q, want %q", got, typefacts.ArrayShapeArray)
	}
}

// Contextual typing decides whether a JSX array literal becomes a tuple, and
// consumers depend on that: a literal in a position typed by an interface with
// numbered members gets fixed slots, while the same literal in an unconstrained
// position stays a plain array. The distinction is what lets a consumer tell
// "the checker examined this and it is not a valid pair" from "nothing here
// constrains it".
func TestContextualTypingDecidesJsxLiteralTupleness(t *testing.T) {
	dir := t.TempDir()
	globals := `declare namespace JSX {
  interface EventHandler<T, E> { (e: E & { currentTarget: T; target: Element }): void }
  interface BoundEventHandler<T, E, EH extends EventHandler<T, any> = EventHandler<T, E>> {
    0: (data: any, ...e: Parameters<EH>) => void;
    1: any;
  }
  type EventHandlerUnion<T, E, EH extends EventHandler<T, any> = EventHandler<T, E>> =
    EH | BoundEventHandler<T, E, EH>;
  interface IntrinsicElements {
    button: { onClick?: EventHandlerUnion<HTMLButtonElement, MouseEvent> };
    loose: any;
  }
  interface Element {}
}
`
	source := `declare const handler: (data: number, event: MouseEvent) => void;

export const Pair = () => <button onClick={[handler, 1]} />;
export const NotAPair = () => <button onClick={[1, 2, 3]} />;
export const Unconstrained = () => <loose onClick={[handler, 1]} />;
`
	sourcePath := filepath.Join(dir, "contextual.tsx")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"jsx":"preserve","module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.tsx","*.d.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "globals.d.ts"), []byte(globals), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	tests := []struct {
		name    string
		literal string
		nth     int
		want    *typefacts.TupleShape
	}{
		{"bound pair", "[handler, 1]", 0, &typefacts.TupleShape{FixedLength: 2, ExactLength: 2, ExactLengthKnown: true, ElementZero: typefacts.CallabilityCallable, ElementZeroMinimumParameters: 2}},
		{"wrong element types", "[1, 2, 3]", 0, &typefacts.TupleShape{FixedLength: 3, ExactLength: 3, ExactLengthKnown: true, ElementZero: typefacts.CallabilityNonCallable}},
		// Same literal, unconstrained position: no fixed slots at all.
		{"unconstrained", "[handler, 1]", 1, nil},
	}
	for _, testCase := range tests {
		start, seen := -1, -1
		for offset := 0; ; {
			index := strings.Index(source[offset:], testCase.literal)
			if index < 0 {
				break
			}
			seen++
			start = offset + index
			offset = start + 1
			if seen == testCase.nth {
				break
			}
		}
		if start < 0 {
			t.Fatalf("%s: %q not found", testCase.name, testCase.literal)
		}
		entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
			Location:   typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.literal)},
			TupleShape: true,
			ArrayShape: true,
		}})
		if err != nil {
			t.Fatal(err)
		}
		got := entities[0].TupleShape
		switch {
		case testCase.want == nil && got != nil:
			t.Errorf("%s tuple shape = %+v, want absent", testCase.name, got)
		case testCase.want != nil && (got == nil || *got != *testCase.want):
			t.Errorf("%s tuple shape = %+v, want %+v", testCase.name, got, testCase.want)
		}
		// Every one of them is array-shaped, which is precisely why arrayShape
		// alone could not separate them.
		if entities[0].ArrayShape != typefacts.ArrayShapeArray {
			t.Errorf("%s array shape = %q, want %q", testCase.name, entities[0].ArrayShape, typefacts.ArrayShapeArray)
		}
	}
}

func TestDemandedLibraryTypesResolveThroughSpelling(t *testing.T) {
	dir := t.TempDir()
	source := `import { Stamps, Ids } from "./aliases";
type LocalWhen = Date;
type Pair = [Date, string];
interface Map<K, V> { fake: K | V }

declare const written: Date;
declare const aliased: LocalWhen;
declare const imported: Stamps;
declare const setAlias: Ids;
declare const suffixArray: Date[];
declare const genericArray: Array<Date>;
declare const optional: Date | undefined;
declare const mixedUnion: Date | Set<string>;
declare const intersected: Date & { tag: "t" };
declare const globalMap: ReadonlyMap<string, number>;
declare const bytes: Uint8Array;
declare const nested: { when: Date };
declare const returnsDate: () => Date;
declare const shadowed: Map<string, number>;
declare const plain: string;
declare const tuple: Pair;

written;
aliased;
imported;
setAlias;
suffixArray;
genericArray;
optional;
mixedUnion;
intersected;
globalMap;
bytes;
nested;
returnsDate;
shadowed;
plain;
tuple;
`
	sourcePath := filepath.Join(dir, "library.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext","lib":["esnext","dom"]},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "aliases.ts"), []byte("export type Stamps = Date[];\nexport type Ids = Set<string>;\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	tests := []struct {
		expression string
		want       []string
	}{
		// The point of the fact: every spelling of the same runtime type answers
		// the same, including a local alias and one imported from another file.
		{`written`, []string{"Date"}},
		{`aliased`, []string{"Date"}},
		{`imported`, []string{"Array", "Date"}},
		{`setAlias`, []string{"Set"}},
		// `Date[]` and `Array<Date>` are the same runtime value and now answer
		// alike; the text screen this replaced matched only the first.
		{`suffixArray`, []string{"Array", "Date"}},
		{`genericArray`, []string{"Array", "Date"}},
		// Union and intersection members are as top-level as the type itself; a
		// nullish member simply contributes nothing.
		{`optional`, []string{"Date"}},
		{`mixedUnion`, []string{"Date", "Set"}},
		{`intersected`, []string{"Date"}},
		{`globalMap`, []string{"ReadonlyMap"}},
		{`bytes`, []string{"Uint8Array"}},
		// Not top level: an object's property and a function's return type. This
		// is the existing boundary, kept — an unproven member is not a proven one.
		{`nested`, nil},
		{`returnsDate`, nil},
		// A user-declared `Map` shadows the global. Name matching could not tell
		// these apart; a declaration-file check can.
		{`shadowed`, nil},
		{`plain`, nil},
		// A tuple's own symbol is its tuple target, not the global Array, so only
		// its element types contribute.
		{`tuple`, []string{"Date"}},
	}
	demands := make([]typefacts.EntityDemand, len(tests))
	for index, testCase := range tests {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands[index] = typefacts.EntityDemand{
			Location:     typefacts.Location{Path: sourcePath, StartByte: start, EndByte: start + len(testCase.expression)},
			LibraryTypes: true,
		}
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	for index, testCase := range tests {
		got := entities[index].LibraryTypes
		if len(got) != len(testCase.want) {
			t.Errorf("%s library types = %v, want %v", testCase.expression, got, testCase.want)
			continue
		}
		for position := range got {
			if got[position] != testCase.want[position] {
				t.Errorf("%s library types = %v, want %v", testCase.expression, got, testCase.want)
				break
			}
		}
	}

	undemanded, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:       demands[0].Location,
		TypeDescriptor: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := undemanded[0].LibraryTypes; got != nil {
		t.Errorf("undemanded library types = %v, want absent", got)
	}
}

// The signature-less `Function`-supertype family, which ADR 0020 named as a
// follow-up: lib.es5.d.ts's `Function` interface declares apply/call/bind and
// no call or construct signature, so GetSignaturesOfType alone reports it as a
// non-function and a consumer reading `nonCallable` + `nonConstructable` as
// proof of non-function was wrong about every value of that type.
//
// The boundary is the compiler's, not this repository's, and it is narrower
// than the family list that prose reached for. Every row below carries the
// subtype answer that decides it, so the boundary is pinned as a *relation* and
// not as a list of type names: `object`, `{}`, `Record<string, unknown>` and an
// interface that merely declares `bind` are NOT subtypes of `Function`, the
// compiler refuses to call them, and `nonCallable` is the honest answer there.
func TestCallabilityAnswersTheSignatureLessFunctionSupertypeFamily(t *testing.T) {
	dir := t.TempDir()
	source := `declare const bare: Function;
declare const callableFunction: CallableFunction;
declare const newableFunction: NewableFunction;
type Handler = Function;
declare const aliased: Handler;
interface Middleware extends Function { tag: string }
declare const extended: Middleware;
declare const branded: Function & { brand: "route" };
const assignedFunction: Function = () => {};
declare const plainObject: object;
declare const emptyObject: {};
declare const record: Record<string, unknown>;
interface OnlyBind { bind(this: void): void }
declare const onlyBind: OnlyBind;
declare const numeric: number;
declare const typedFunction: () => void;
class Widget {}
declare const withNumber: Function | number;
declare const optional: Function | undefined;
declare const eitherFunction: Function | (() => void);
export { bare, callableFunction, newableFunction, aliased, extended, branded, assignedFunction, plainObject, emptyObject, record, onlyBind, numeric, typedFunction, Widget, withNumber, optional, eitherFunction };
`
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	cases := []struct {
		// expression is matched at its LAST occurrence: the export clause.
		expression string
		// subtypeOfFunction is the compiler's own isTypeSubtypeOf answer
		// against the global Function type, which is the relation
		// isFunctionObjectType and the untyped-call rule both turn on.
		subtypeOfFunction bool
		callability       typefacts.Callability
		constructability  typefacts.Constructability
	}{
		// The family. Every one of these is callable at runtime and exposes no
		// signature; `untypedCallable` says exactly that.
		{"bare", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		{"callableFunction", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		// Named "Newable" and still not constructable: `new x()` on it is a
		// compile error, pinned below. Its declared overloads live on `bind`,
		// not on a construct signature.
		{"newableFunction", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		// An alias renders as its own name, which is why no consumer could
		// detect this family from typeDescriptor.text. The type is transparent.
		{"aliased", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		// An interface *extending* Function inherits the shape and adds
		// members; still no signature of its own.
		{"extended", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		// A branded intersection is the row that decides which compiler
		// predicate this fact uses. isFunctionObjectType answers false here —
		// its `bind` quick-out reads the resolved members map, which the
		// compiler leaves empty for every intersection by construction — while
		// the untyped-call rule, and the subtype relation below it, answer
		// true. The call is permitted, so the fact follows the call rule.
		{"branded", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
		// The annotation, not the initializer, is the fact: a real function
		// declared as `Function` answers by its declared type, and that answer
		// is now the same one the value deserves.
		{"assignedFunction", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},

		// Outside the family. These are the controls that keep the new value
		// from becoming "anything a function could be assigned to": a function
		// *is* assignable to `object`, and `object` is still not callable.
		{"plainObject", false, typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		{"emptyObject", false, typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		{"record", false, typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		// A `bind` member alone is not the rule — it is only the compiler's
		// cheap pre-filter before the subtype check that is.
		{"onlyBind", false, typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},
		{"numeric", false, typefacts.CallabilityNonCallable, typefacts.ConstructabilityNonConstructable},

		// Unchanged answers: a readable signature still answers `callable`, and
		// a class value type still answers `constructable`.
		{"typedFunction", true, typefacts.CallabilityCallable, typefacts.ConstructabilityNonConstructable},
		{"Widget", true, typefacts.CallabilityNonCallable, typefacts.ConstructabilityConstructable},

		// Aggregation. A non-callable constituent beside a callable one is
		// still `mixed`, which is where `Function | number` and the optional
		// case land — both answered `nonCallable` before this rule and both
		// were wrong.
		{"withNumber", false, typefacts.CallabilityMixed, typefacts.ConstructabilityNonConstructable},
		{"optional", false, typefacts.CallabilityMixed, typefacts.ConstructabilityNonConstructable},
		// Every constituent callable, one of them unreadably: the weaker of the
		// two callable answers wins. That promise is per constituent, not a
		// claim about the union's own call: tsc still finds one readable,
		// arity-enforced signature here (a wrong argument count is TS2554),
		// which this fact under-checks rather than misreports.
		// Subtype-of-Function is true for the union as a whole here, because it
		// is true of every constituent — the relation is not a proxy for "is a
		// single Function-family type".
		{"eitherFunction", true, typefacts.CallabilityUntypedCallable, typefacts.ConstructabilityNonConstructable},
	}
	demands := make([]typefacts.EntityDemand, 0, len(cases))
	for _, testCase := range cases {
		start := strings.LastIndex(source, testCase.expression)
		if start < 0 {
			t.Fatalf("%q not found", testCase.expression)
		}
		demands = append(demands, typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(testCase.expression),
			},
			Callability:      true,
			Constructability: true,
		})
	}
	entities, err := semantic.SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != len(cases) {
		t.Fatalf("entities = %d, want %d", len(entities), len(cases))
	}
	for index, testCase := range cases {
		if got := entities[index].Callability; got != testCase.callability {
			t.Errorf("%s callability = %q, want %q", testCase.expression, got, testCase.callability)
		}
		if got := entities[index].Constructability; got != testCase.constructability {
			t.Errorf("%s constructability = %s, want %s", testCase.expression, got, testCase.constructability)
		}
	}

	// The relation itself, asked of the compiler. Without this the rows above
	// would only pin the answers and not the boundary that produces them, and a
	// compiler bump that moved `Record<string, unknown>` into the family would
	// look like a producer bug rather than an upstream change.
	proj := opened.(*project)
	proj.mu.Lock()
	defer proj.mu.Unlock()
	typeChecker := proj.checker
	if typeChecker == nil {
		t.Fatal("callability demands did not create a checker")
	}
	file := proj.program.GetSourceFile(sourcePath)
	if file == nil {
		t.Fatalf("source file %q is not in the program", sourcePath)
	}
	cursor := &semanticNodeCursor{sourceFile: file}
	typeOf := func(expression string) *checker.Type {
		t.Helper()
		start := strings.LastIndex(source, expression)
		node := cursor.exactExpressionAt(start, start+len(expression))
		if node == nil {
			t.Fatalf("%q has no expression at its export specifier", expression)
		}
		return typeChecker.GetTypeAtLocation(node)
	}
	functionType := typeOf("bare")
	for _, testCase := range cases {
		valueType := typeOf(testCase.expression)
		if got := checker.Checker_isTypeSubtypeOf(typeChecker, valueType, functionType); got != testCase.subtypeOfFunction {
			t.Errorf(
				"%s (%s) isTypeSubtypeOf(Function) = %v, want %v",
				testCase.expression,
				typeChecker.TypeToString(valueType),
				got,
				testCase.subtypeOfFunction,
			)
		}
		// isFunctionObjectType — the predicate ADR 0020's follow-up named — is
		// the compiler's `typeof x === "function"` answer. It is also true for
		// any type that simply has signatures, including a construct-only class
		// value type that callability answers `nonCallable` for on purpose, so
		// the conformance claim is about the signature-less case only: where the
		// compiler calls a signature-less object type a function, this fact must
		// not answer nonCallable. The `branded` intersection row is where the
		// predicate is *false* and this fact is positive anyway, which is why
		// the untyped-call rule replaced it rather than joining it.
		if valueType.Flags()&checker.TypeFlagsObject == 0 {
			continue
		}
		if len(typeChecker.GetSignaturesOfType(valueType, checker.SignatureKindCall)) != 0 ||
			len(typeChecker.GetSignaturesOfType(valueType, checker.SignatureKindConstruct)) != 0 {
			continue
		}
		if !checker.Checker_isFunctionObjectType(typeChecker, valueType) {
			continue
		}
		if testCase.callability == typefacts.CallabilityNonCallable {
			t.Errorf(
				"%s (%s) is a signature-less compiler function object but callability = %q",
				testCase.expression,
				typeChecker.TypeToString(valueType),
				testCase.callability,
			)
		}
	}
}

// The asymmetry the family forces on the pair, asked of the compiler's own
// diagnostics rather than asserted from prose: calling a `Function`-typed value
// is legal (TS 1.0 §4.12's untyped call, resolved to anySignature), and `new`-ing
// one is not (resolveNewExpression has no untyped fallback). That is why
// callability gained a value for this family and constructability did not.
func TestTheFunctionSupertypeFamilyIsCallableButNotConstructable(t *testing.T) {
	dir := t.TempDir()
	source := `declare const handler: Function;
export const called = handler();
export const constructed = new handler();
`
	sourcePath := filepath.Join(dir, "facts.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	proj := opened.(*project)
	proj.mu.Lock()
	defer proj.mu.Unlock()
	if err := proj.ensureCheckerLocked(context.Background()); err != nil {
		t.Fatal(err)
	}
	file := proj.program.GetSourceFile(sourcePath)
	if file == nil {
		t.Fatalf("source file %q is not in the program", sourcePath)
	}
	callStart := strings.Index(source, "handler()")
	constructStart := strings.Index(source, "new handler()")
	var onCall, onConstruct []int32
	for _, diagnostic := range proj.program.GetSemanticDiagnostics(context.Background(), file) {
		position := diagnostic.Pos()
		switch {
		case position >= callStart && position < callStart+len("handler()"):
			onCall = append(onCall, diagnostic.Code())
		case position >= constructStart && position < constructStart+len("new handler()"):
			onConstruct = append(onConstruct, diagnostic.Code())
		}
	}
	if len(onCall) != 0 {
		t.Errorf("calling a Function-typed value reported diagnostic codes %v, want none", onCall)
	}
	if len(onConstruct) == 0 {
		t.Error("new on a Function-typed value reported no diagnostic, want one")
	}
}

// ParameterFact.Callability shares callabilityOfType with EntityFact.callability
// (both resolvedCall's argument-mapping path and this one call
// callabilityOfType directly on the formal parameter's type), and only the
// entity-level path had a regression test for the signature-less
// Function-supertype family. A parameter typed exactly `Function` must answer
// the same way an expression typed `Function` does.
func TestResolvedCallParameterAnswersUntypedCallableForFunctionTypedParameter(t *testing.T) {
	dir := t.TempDir()
	source := `declare function register(handler: Function): void;
declare const fn: () => void;
register(fn);
`
	sourcePath := filepath.Join(dir, "parameter.ts")
	if err := os.WriteFile(filepath.Join(dir, "tsconfig.json"), []byte(`{"compilerOptions":{"strict":true,"module":"esnext","target":"esnext"},"include":["*.ts"]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(source), 0o644); err != nil {
		t.Fatal(err)
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	semantic := opened.(typefacts.SemanticEntityLookup)

	callStart := strings.LastIndex(source, "register(fn)")
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location: typefacts.Location{
			Path:      sourcePath,
			StartByte: callStart,
			EndByte:   callStart + len("register(fn)"),
		},
		ResolvedCall: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	call := entities[0].ResolvedCall
	if call == nil {
		t.Fatal("resolved call fact is missing")
	}
	if len(call.Arguments) != 1 || call.Arguments[0].Parameter == nil {
		t.Fatalf("argument mapping = %+v, want one resolved parameter", call.Arguments)
	}
	if got := call.Arguments[0].Parameter.Callability; got != typefacts.CallabilityUntypedCallable {
		t.Errorf("Function-typed parameter callability = %q, want untypedCallable", got)
	}
}
