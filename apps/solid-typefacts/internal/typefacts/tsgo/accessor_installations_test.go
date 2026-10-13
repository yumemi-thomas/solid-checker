package tsgo

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// accessorSites answers the census attached to one export's implementation.
func accessorSites(t *testing.T, analyzer typefacts.ExportValueAnalyzer, path, source, name string) []typefacts.AccessorInstallationSite {
	t.Helper()
	transcript := implementationTranscriptFor(t, analyzer, path, source, name)
	if transcript.AccessorInstallations == nil {
		t.Fatalf("%s: no accessor-installation census stated", name)
	}
	return transcript.AccessorInstallations.Sites
}

func accessorSiteAt(t *testing.T, sites []typefacts.AccessorInstallationSite, source, needle string) typefacts.AccessorInstallationSite {
	t.Helper()
	start := strings.Index(source, needle)
	if start < 0 {
		t.Fatalf("source does not contain %q", needle)
	}
	for _, site := range sites {
		if site.Site.StartByte <= start && start < site.Site.EndByte {
			return site
		}
	}
	t.Fatalf("no site contains %q in %+v", needle, sites)
	return typefacts.AccessorInstallationSite{}
}

const accessorBounded = `import { createComponent } from "solid-js";
export function createBox() {
  return new Proxy({}, { get: () => 1 });
}
export function createBounds(keys: string[]) {
  const bounds: Record<string, number> = {};
  keys.forEach((key) => {
    Object.defineProperty(bounds, key, { get: () => 1, enumerable: true });
  });
  return bounds;
}
export function readBounds(keys: string[]) {
  const measured = createBounds(keys);
  return measured.width;
}
export const callsReader = () => readBounds([]);
export function plainSum(a: number) { return a + 1; }
function Child(props: { box: any }) { return props.box.size; }
export function Parent() { return createComponent(Child, { box: createBox() }); }
export const namespaceObject = Object.freeze({ __proto__: null, value: 1 });
`

// ADR 0153 item C, the positive half: a fresh target returned and never
// operated on bounds every export, and one operated on bounds every export
// except the functions that may execute the operation, closed backward over
// their callers -- through an alias binding, an arrow held by a const, and a
// local component's props.
func TestAccessorInstallationBoundsTheExportsThatCannotOperateOnAFreshTarget(t *testing.T) {
	analyzer, dir := contextProject(t, map[string]string{"bounded.ts": accessorBounded})
	path := filepath.Join(dir, "bounded.ts")
	reached := func(name, needle string) bool {
		site := accessorSiteAt(t, accessorSites(t, analyzer, path, accessorBounded, name), accessorBounded, needle)
		if site.Kind != typefacts.AccessorInstallationFreshTarget {
			t.Fatalf("%s: site %q is %s (%s), want fresh-target", name, needle, site.Kind, site.Refusal)
		}
		return site.Reached
	}
	proxy := accessorSiteAt(t, accessorSites(t, analyzer, path, accessorBounded, "plainSum"), accessorBounded, "new Proxy")
	if proxy.Kind != typefacts.AccessorInstallationFreshTarget || len(proxy.Returns) != 1 ||
		len(proxy.Accesses) != 1 || proxy.Accesses[0].Kind != "member" || len(proxy.Readers) != 2 {
		t.Fatalf("proxy site %+v, want returned by createBox and read through Child's props by Child and Parent", proxy)
	}
	installed := accessorSiteAt(t, accessorSites(t, analyzer, path, accessorBounded, "plainSum"), accessorBounded, "Object.defineProperty")
	if len(installed.Returns) != 1 || len(installed.Accesses) != 1 || len(installed.Readers) != 2 {
		t.Fatalf("defineProperty site %+v, want returned by createBounds and read by readBounds and callsReader", installed)
	}
	for _, name := range []string{"createBox", "createBounds", "plainSum", "readBounds", "callsReader", "Parent"} {
		proxyReached := reached(name, "new Proxy")
		installReached := reached(name, "Object.defineProperty")
		wantProxy := name == "Parent"
		wantInstall := name == "readBounds" || name == "callsReader"
		if proxyReached != wantProxy || installReached != wantInstall {
			t.Fatalf("%s: reaches proxy %v, defineProperty %v; want %v, %v", name, proxyReached, installReached, wantProxy, wantInstall)
		}
	}
	null := accessorSiteAt(t, accessorSites(t, analyzer, path, accessorBounded, "plainSum"), accessorBounded, "__proto__")
	if null.Kind != typefacts.AccessorInstallationNullPrototype {
		t.Fatalf("__proto__: null site %+v, want null-prototype", null)
	}
}

// The negative half: every way the target, or a function that operates on it,
// can leave what the census follows makes the site unbounded, by name.
func TestAccessorInstallationRefusesEveryUnfollowedUse(t *testing.T) {
	cases := []struct {
		name, source, needle, refusal string
	}{
		{"assigned", `let cache: object | undefined;
export function makeCached() {
  const store = {};
  Object.defineProperty(store, "x", { get: () => 1 });
  cache = store;
  return store;
}
export function other() { return 1; }
`, "Object.defineProperty", "assigned"},
		{"second path", `declare function register(value: object): void;
export function makeRegistered() {
  const store = {};
  Object.defineProperty(store, "x", { get: () => 1 });
  register(store);
  return store;
}
export function other() { return 1; }
`, "Object.defineProperty", "passed to a call"},
		{"context value", `import { createContext, createComponent } from "solid-js";
const Ctx = createContext<object>();
function createMemoObject() { return new Proxy({}, { get: () => 1 }); }
export function Provider() { return createComponent(Ctx, { value: createMemoObject() }); }
export function other() { return 1; }
`, "new Proxy", "component this census does not follow"},
		{"object member", `function createMemoObject() { return new Proxy({}, { get: () => 1 }); }
export function route() { return { params: createMemoObject() }; }
export function other() { return 1; }
`, "new Proxy", "stored in an object"},
		{"tuple", `export function createStatic() {
  const store = {};
  Object.defineProperty(store, "x", { get: () => 1 });
  return [store, () => 1];
}
export function other() { return 1; }
`, "Object.defineProperty", "ArrayLiteralExpression"},
		{"anonymous access", `function make() { return new Proxy({}, { get: () => 1 }); }
export function deferred() { const value = make(); return () => value.x; }
export function other() { return 1; }
`, "new Proxy", "callable with no name"},
		{"module scope", `const shared = {};
Object.defineProperty(shared, "x", { get: () => 1 });
export function other() { return shared; }
`, "Object.defineProperty", "module scope"},
		{"reader as value", `function make() { return new Proxy({}, { get: () => 1 }); }
export function readIt() { return make().x; }
export const table = { readIt };
export function other() { return 1; }
`, "new Proxy", "escapes as a value"},
		{"returned from a method", `export class Factory {
  build() { return new Proxy({}, { get: () => 1 }); }
}
export function other() { return 1; }
`, "new Proxy", "not a named, synchronous function"},
		{"not fresh", `export function wrap(target: object) {
  Object.defineProperty(target, "x", { get: () => 1 });
  return target;
}
export function other() { return 1; }
`, "Object.defineProperty", "not an allocation"},
		{"aliased constructor", `const P = Proxy;
export function other() { return new P({}, {}); }
`, "Proxy;", "other than as the callee"},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			analyzer, dir := contextProject(t, map[string]string{"negative.ts": test.source})
			site := accessorSiteAt(t, accessorSites(t, analyzer, filepath.Join(dir, "negative.ts"), test.source, "other"), test.source, test.needle)
			if site.Kind != typefacts.AccessorInstallationUnbounded || !strings.Contains(site.Refusal, test.refusal) {
				t.Fatalf("site %+v, want unbounded naming %q", site, test.refusal)
			}
			if site.Reached || len(site.Readers) != 0 || site.Function != nil {
				t.Fatalf("an unbounded site states a flow: %+v", site)
			}
		})
	}
}

// Silence is not a census: a package with no installation states an empty
// one, which is the positive claim.
func TestAccessorInstallationCensusIsStatedEmptyWhenNothingInstalls(t *testing.T) {
	source := `export function plainSum(a: number) { return a + 1; }
`
	analyzer, dir := contextProject(t, map[string]string{"plain.ts": source})
	if sites := accessorSites(t, analyzer, filepath.Join(dir, "plain.ts"), source, "plainSum"); len(sites) != 0 {
		t.Fatalf("sites %+v, want none", sites)
	}
}
