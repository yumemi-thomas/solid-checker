package tsgo

import (
	"path/filepath"
	"testing"
)

func TestLocalLiteralResultRequiresEveryReturnToNameOneLocalAllocation(t *testing.T) {
	const source = `import { make as imported } from "./helper";
function make(flag: boolean): any { const result: any = {}; if (flag) return result; return result; }
function replaced(flag: boolean): any { let result: any = {}; if (flag) result = external; return result; }
function mixed(flag: boolean): any { const result: any = {}; if (flag) return external; return result; }
function fallsThrough(flag: boolean): any { const result: any = {}; if (flag) return result; }
function accessor(): any { const result: any = { get member() { return 1; } }; return result; }
async function asyncResult(): Promise<any> { const result: any = {}; return result; }
function* generator(): any { const result: any = {}; return result; }
const moduleValue: any = {};
function moduleResult(): any { return moduleValue; }
declare const external: any;
export function good(key: string) { const data = make(true); return data[key]; }
export function crossFile(key: string) { const data = imported(true); return data[key]; }
export function replacedRead(key: string) { const data = replaced(true); return data[key]; }
export function mixedRead(key: string) { const data = mixed(true); return data[key]; }
export function fallsThroughRead(key: string) { const data = fallsThrough(true); return data[key]; }
export function accessorRead(key: string) { const data = accessor(); return data[key]; }
export function asyncRead(key: string) { const data: any = asyncResult(); return data[key]; }
export function generatorRead(key: string) { const data = generator(); return data[key]; }
export function moduleRead(key: string) { const data = moduleResult(); return data[key]; }
export function rewrittenRead(key: string) { let data = make(true); data = external; return data[key]; }
export function nestedRead(key: string) { const data = make(true); return data.child[key]; }
`
	analyzer, dir := markerProject(t, map[string]string{
		"results.ts": source,
		"helper.ts":  `export function make(flag: boolean): any { const result: any = {}; if (flag) return result; return result; }`,
	})
	path := filepath.Join(dir, "results.ts")
	for _, name := range []string{"good", "crossFile", "replacedRead", "mixedRead", "fallsThroughRead", "accessorRead", "asyncRead", "generatorRead", "moduleRead", "rewrittenRead", "nestedRead"} {
		transcript := implementationTranscriptFor(t, analyzer, path, source, name)
		forms := transcript.UncensusedInvokingForms
		wantForms := 1
		if name == "nestedRead" {
			wantForms = 2
		}
		if len(forms) != wantForms {
			t.Fatalf("%s: recorded %d forms, want %d (non-vacuity)", name, len(forms), wantForms)
		}
		count := 0
		for _, form := range forms {
			if premise := form.LocalLiteralResult; premise != nil {
				count++
				if len(premise.Returns) != 2 || premise.Allocation.Path == "" || premise.Callee.Path == "" {
					t.Fatalf("%s: incomplete positive premise: %+v", name, premise)
				}
			}
		}
		want := 0
		if name == "good" || name == "crossFile" || name == "nestedRead" {
			want = 1
		}
		if count != want {
			t.Fatalf("%s: %d literal-result premises, want %d", name, count, want)
		}
	}
}
