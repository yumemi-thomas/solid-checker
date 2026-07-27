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
