package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// ADR 0166, handshake protocol 72: a host constant the request carries decides
// a condition that names exactly its import binding, for branch reachability
// and for ImplementationCall.Unconditional alike. Nothing else is decided by
// it: a shadow, a written alias, another module, or an absent constant leaves
// both arms live.
// censusUnder answers the implementation census of `check` in a one-file
// program that imports `@solidjs/web`, with `constants` set for the request.
// The declaration file is the real package's (`export declare const isServer:
// boolean`), which says nothing about the value, so a fold can only come from
// the premise.
func censusUnder(t *testing.T, source string, constants []typefacts.HostConstant) *typefacts.ExportImplementationTranscript {
	t.Helper()
	dir := t.TempDir()
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	web := filepath.Join(dir, "node_modules", "@solidjs", "web")
	if err := os.MkdirAll(web, 0o755); err != nil {
		t.Fatal(err)
	}
	for name, text := range map[string]string{
		"package.json": `{"name":"@solidjs/web","version":"2.0.0-rc.9","types":"index.d.ts"}`,
		"index.d.ts":   "export declare const isServer: boolean;\nexport declare const isDev: boolean;\n",
	} {
		if err := os.WriteFile(filepath.Join(web, name), []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	path := filepath.Join(dir, "facts.ts")
	if constants != nil {
		for index := range constants {
			if constants[index].Importer == "" {
				constants[index].Importer = path
			} else {
				constants[index].Importer = filepath.Join(dir, constants[index].Importer)
			}
		}
		opened.(typefacts.HostConstantScoped).SetHostConstants(constants)
	}
	start := strings.LastIndex(source, "check")
	impl := strings.Index(source, "check(")
	answer, err := opened.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location:               typefacts.Location{Path: path, StartByte: start, EndByte: start + 5},
		ImplementationLocation: &typefacts.Location{Path: path, StartByte: impl, EndByte: impl + 5},
	}})
	if err != nil {
		t.Fatal(err)
	}
	implementation := answer.Transcripts[0].Implementation
	if implementation == nil {
		t.Fatal("missing implementation census")
	}
	return implementation
}

func webConstant(name string, value bool) typefacts.HostConstant {
	return typefacts.HostConstant{Specifier: "@solidjs/web", Name: name, Value: value}
}

func TestAHostConstantDecidesOnlyTheConditionBoundToItsImport(t *testing.T) {
	type want struct {
		reach         typefacts.Reachability
		unconditional bool
	}
	const header = `import { isServer } from "@solidjs/web";` + "\n"
	cases := []struct {
		name      string
		header    string
		body      string
		constants []typefacts.HostConstant // nil: no host constant sent
		call      string
		want      want
	}{
		{"host free", header, `if (isServer) return; cb();`, nil, "cb()", want{typefacts.Reachable, false}},
		{"browser", header, `if (isServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, true}},
		{"node", header, `if (isServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", true)}, "cb()", want{typefacts.Unreachable, false}},
		{"a live arm under browser", header, `if (!isServer) { cb(); }`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, true}},
		{"a dead arm under browser", header, `if (isServer) { cb(); }`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Unreachable, false}},
		{"a ternary arm under node", header, `void (isServer ? cb() : 0);`, []typefacts.HostConstant{webConstant("isServer", true)}, "cb()", want{typefacts.Reachable, true}},
		{"a never-written const", header, `const isSupported = !isServer; if (isSupported) { cb(); }`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, true}},
		{"a written alias", header, `let s = isServer; s = flag; if (s) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, false}},
		{"a shadowing parameter", header, `const run = (isServer: boolean) => { if (isServer) return; cb(); }; run(flag);`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, false}},
		{"a shadowing const", header, `{ const isServer = flag; if (isServer) return; cb(); }`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, false}},
		{"another module's constant", header, `if (isServer) return; cb();`, []typefacts.HostConstant{{Importer: "other.ts", Specifier: "@solidjs/web", Name: "isServer", Value: false}}, "cb()", want{typefacts.Reachable, false}},
		{"another specifier", header, `if (isServer) return; cb();`, []typefacts.HostConstant{{Specifier: "solid-js/web", Name: "isServer", Value: false}}, "cb()", want{typefacts.Reachable, false}},
		{"another name", header, `if (isServer) return; cb();`, []typefacts.HostConstant{webConstant("isDev", false)}, "cb()", want{typefacts.Reachable, false}},
		{"an aliased import", `import { isServer as onServer } from "@solidjs/web";` + "\n", `if (onServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, true}},
		{"an alias named for another export", `import { isDev as isServer } from "@solidjs/web";` + "\n", `if (isServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, false}},
		{"a namespace member", `import * as web from "@solidjs/web";` + "\n", `if (web.isServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false)}, "cb()", want{typefacts.Reachable, false}},
		{"two answers for one import", header, `if (isServer) return; cb();`, []typefacts.HostConstant{webConstant("isServer", false), webConstant("isServer", true)}, "cb()", want{typefacts.Reachable, false}},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			source := testCase.header + "export function check(cb: () => void, flag: boolean) { " + testCase.body + " }\nvoid check;\n"
			implementation := censusUnder(t, source, testCase.constants)
			found := false
			for _, call := range implementation.Calls {
				if source[call.Location.StartByte:call.Location.EndByte] != testCase.call {
					continue
				}
				found = true
				if call.Reach != testCase.want.reach || call.Unconditional != testCase.want.unconditional {
					t.Errorf("%s: reach %v unconditional %v, want %v %v",
						testCase.call, call.Reach, call.Unconditional, testCase.want.reach, testCase.want.unconditional)
				}
			}
			if !found {
				t.Fatalf("no census row for %s", testCase.call)
			}
		})
	}
}

// The two bodies below are byte for byte
// `@solid-primitives/lifecycle@1.0.0-next.2` `dist/index.js` `onElementConnect`
// and `@solid-primitives/memo@2.0.0-next.2` `dist/index.js` `createPureReaction`
// (the JavaScript, typed loosely for a TypeScript program). Under node the
// guard always returns, so the value-carrying return and every call after it
// are unreachable; under browser it never returns, so what follows it runs
// whenever the function does, and only the conditional return keeps
// `onElementConnect`'s `onCleanup` conditional.
func TestAHostConstantDecidesTheLifecycleGuards(t *testing.T) {
	const onElementConnect = `import { isServer } from "@solidjs/web";
declare const onCleanup: (fn: () => void) => void;
export function check(el: HTMLElement, fn: () => unknown) {
	if (isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}
void check;
`
	const createPureReaction = `import { isServer } from "@solidjs/web";
declare const getOwner: () => object | null;
declare const onCleanup: (fn: () => void) => void;
export function check(onInvalidate: () => void) {
	if (isServer) return () => void 0;
	const owner = getOwner();
	let disposed = false;
	onCleanup(() => {
		disposed = true;
	});
	return owner;
}
void check;
`
	type row struct {
		reach         typefacts.Reachability
		unconditional bool
	}
	callRow := func(implementation *typefacts.ExportImplementationTranscript, source, text string) row {
		t.Helper()
		for _, call := range implementation.Calls {
			if source[call.Location.StartByte:call.Location.EndByte] == text {
				return row{call.Reach, call.Unconditional}
			}
		}
		t.Fatalf("no census row for %s", text)
		return row{}
	}
	hosts := []struct {
		name      string
		constants []typefacts.HostConstant
	}{
		{"host free", nil},
		{"browser", []typefacts.HostConstant{webConstant("isServer", false)}},
		{"node", []typefacts.HostConstant{webConstant("isServer", true)}},
	}
	for _, host := range hosts {
		t.Run("onElementConnect "+host.name, func(t *testing.T) {
			implementation := censusUnder(t, onElementConnect, append([]typefacts.HostConstant(nil), host.constants...))
			var valueReturn, bareReturn *typefacts.ReturnSite
			for index := range implementation.ControlFlow.Returns {
				site := &implementation.ControlFlow.Returns[index]
				switch onElementConnect[site.Location.StartByte:site.Location.EndByte] {
				case "return fn();":
					valueReturn = site
				case "return;":
					bareReturn = site
				}
			}
			if valueReturn == nil || bareReturn == nil {
				t.Fatalf("return sites: value %v bare %v", valueReturn, bareReturn)
			}
			wantValue := typefacts.Reachable
			if host.name == "node" {
				wantValue = typefacts.Unreachable
			}
			if valueReturn.Reach != wantValue {
				t.Errorf("return fn(): reach %v, want %v", valueReturn.Reach, wantValue)
			}
			wantBare := typefacts.Reachable
			if host.name == "browser" {
				wantBare = typefacts.Unreachable
			}
			if bareReturn.Reach != wantBare {
				t.Errorf("bare return: reach %v, want %v", bareReturn.Reach, wantBare)
			}
			// `el.isConnected` decides nothing under any host, so the cleanup
			// registration is never unconditional.
			cleanup := callRow(implementation, onElementConnect, "onCleanup(() => observer.disconnect())")
			if cleanup.unconditional {
				t.Errorf("onCleanup: unconditional under %s", host.name)
			}
			if host.name == "node" && cleanup.reach != typefacts.Unreachable {
				t.Errorf("onCleanup: reach %v under node, want unreachable", cleanup.reach)
			}
		})
		t.Run("createPureReaction "+host.name, func(t *testing.T) {
			implementation := censusUnder(t, createPureReaction, append([]typefacts.HostConstant(nil), host.constants...))
			registration := callRow(implementation, createPureReaction, "onCleanup(() => {\n\t\tdisposed = true;\n\t})")
			owner := callRow(implementation, createPureReaction, "getOwner()")
			wantUnconditional := host.name == "browser"
			wantReach := typefacts.Reachable
			if host.name == "node" {
				wantReach = typefacts.Unreachable
			}
			for name, got := range map[string]row{"onCleanup": registration, "getOwner": owner} {
				if got.reach != wantReach || got.unconditional != wantUnconditional {
					t.Errorf("%s under %s: reach %v unconditional %v, want %v %v",
						name, host.name, got.reach, got.unconditional, wantReach, wantUnconditional)
				}
			}
		})
	}
}
