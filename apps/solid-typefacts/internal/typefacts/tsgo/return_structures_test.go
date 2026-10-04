package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestReturnStructuresTypedMembersKeepSeparateCallableIdentity(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	write("solid.d.ts", `declare module "solid-js" { export type Accessor<T> = () => T; }`)
	source := `import type {Accessor} from "solid-js"; function read(props: {data: Accessor<number>}) { return props.data(); }`
	path := write("run.ts", source)
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	start := strings.Index(source, "props.data()")
	at := func(start, end int) typefacts.Location {
		return typefacts.Location{Path: path, StartByte: start, EndByte: end}
	}
	property := at(start+6, start+10)
	results, err := opened.(semanticDemandRunner).SemanticDemandRuns(context.Background(), []typefacts.SemanticDemandRun{{Path: path, Demands: []typefacts.EntityDemand{
		{Location: at(start, start+5), Symbol: true, TypeDescriptor: true, Callability: true},
		{Location: at(start, start+10), QueryLocation: &property, Symbol: true, TypeDescriptor: true, Callability: true},
		{Location: property, Symbol: true, TypeDescriptor: true, Callability: true},
	}}}, typefacts.SemanticScope{})
	if err != nil {
		t.Fatal(err)
	}
	facts := results[0].Entities
	if facts[0].Callability != typefacts.CallabilityNonCallable || facts[1].Callability != typefacts.CallabilityCallable || facts[2].Callability != typefacts.CallabilityCallable || facts[0].Symbol != facts[1].Symbol || facts[1].Symbol == facts[2].Symbol {
		t.Fatalf("root/member/property identity: %+v", facts)
	}
	descriptor := facts[2].TypeDescriptor
	if descriptor == nil || len(descriptor.AliasDeclarations) != 1 || descriptor.AliasDeclarations[0].Name != "Accessor" {
		t.Fatalf("member descriptor: %+v", descriptor)
	}
}

func TestReturnStructuresAreExhaustiveFreshLiteralCensuses(t *testing.T) {
	cases := []struct{ name, expression, kind string }{
		{"tuple", "[1, value]", "tuple"},
		{"emptyTuple", "[]", "tuple"},
		{"object", "({x: 1, y: value})", "object"},
		{"emptyObject", "({})", "object"},
		{"nested", "([{x: value}, 2] as const)", "tuple"},
		{"unknownLeaf", "[other()]", "tuple"},
		{"hole", "[, 1]", ""},
		{"spread", "[1, ...tail]", ""},
		{"nestedSpread", "[{x: [...tail]}]", ""},
		{"objectSpread", "({x: 1, ...value})", ""},
		{"getter", "({get x() { return 1; }})", ""},
		{"setter", "({set x(v) {}})", ""},
		{"method", "({x() { return 1; }})", ""},
		{"computed", "({['x']: 1})", ""},
		{"duplicate", "({x: 1, 'x': 2})", ""},
		{"escapedDuplicate", "({x: 1, '\\u0078': 2})", ""},
		{"prototype", "({'__proto__': value})", ""},
		{"binding", "value", ""},
		{"shorthand", "({value})", "object"},
		{"shorthandBeside", "({value, x: 1})", "object"},
		{"numeric", "({1: value})", ""},
		{"budget", "[" + strings.Repeat("1,", 129) + "]", ""},
		{"depth", strings.Repeat("[", 10) + "1" + strings.Repeat("]", 10), ""},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			transcript := exportImplementationForSolidMake(t, "export function make(value: any, tail: any[]) { return "+test.expression+"; }\nvoid make;\n")
			got := transcript.ControlFlow.Returns[0].Structure
			if test.kind == "" {
				if got != nil {
					t.Fatalf("unsupported literal produced a structure: %#v", got)
				}
				return
			}
			if got == nil || got.Kind != test.kind || !got.Complete {
				t.Fatalf("structure = %#v, want complete %s", got, test.kind)
			}
			if test.name == "tuple" {
				if len(got.Items) != 2 || !got.Items[0].PrimitiveSyntax || got.Items[1].Parameter == nil || got.Items[1].Parameter.ParameterIndex != 0 {
					t.Fatalf("leaf evidence: %#v", got.Items)
				}
			}
			// ADR 0181: a shorthand member's value is the binding it reads, the
			// caller's parameter here, not the object literal's own property.
			if test.name == "shorthand" {
				if len(got.Properties) != 1 || got.Properties[0].Name != "value" || got.Properties[0].Value.Parameter == nil || got.Properties[0].Value.Parameter.ParameterIndex != 0 {
					t.Fatalf("shorthand leaf evidence: %#v", got.Properties)
				}
			}
			if test.name == "unknownLeaf" && (got.Items[0].PrimitiveSyntax || got.Items[0].Parameter != nil) {
				t.Fatal("unknown leaf became primitive or parameter")
			}
		})
	}
}

func TestReturnStructuresStateBodyEndReachability(t *testing.T) {
	for _, test := range []struct {
		body string
		end  typefacts.Reachability
	}{
		{"return [1];", typefacts.Unreachable},
		{"if (value) return [1];", typefacts.Reachable},
		{"if (value) return [1]; else return {x: 2};", typefacts.Unreachable},
		{"if (value) throw value; return [1];", typefacts.Unreachable},
		{"for (;;) { return [1]; }", typefacts.ReachUnknown},
	} {
		transcript := exportImplementationForSolidMake(t, "export function make(value: any) {"+test.body+"} void make;")
		if transcript.ControlFlow.EndReach == nil || *transcript.ControlFlow.EndReach != test.end {
			t.Fatalf("%s: end reach = %v, want %s", test.body, transcript.ControlFlow.EndReach, test.end)
		}
	}
}

func TestReturnStructuresCoverConditionalArmsWithoutInferringIdentity(t *testing.T) {
	transcript := exportImplementationForSolidMake(t, `export function make(value: any) { value = 1; return value ? [value] : {x: 2}; } void make;`)
	site := transcript.ControlFlow.Returns[0]
	if site.Structure != nil || len(site.Arms) != 2 || site.Arms[0].Structure == nil || site.Arms[1].Structure == nil {
		t.Fatalf("conditional census: %#v", site)
	}
	if site.Arms[0].Structure.Items[0].Parameter != nil {
		t.Fatal("written parameter preserved original-input identity")
	}
}
