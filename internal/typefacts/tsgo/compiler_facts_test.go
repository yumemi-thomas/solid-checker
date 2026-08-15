package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

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
		demands[index] = typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      sourcePath,
				StartByte: start,
				EndByte:   start + len(testCase.needle),
			},
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
