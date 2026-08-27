package typefacts_test

import (
	"context"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

func TestProjectDiscoversSemanticAsyncFunctionsAndPostAwaitCalls(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true,"target":"ES2022"},"include":["*.ts"]}`)
	source := `
declare function after(): void;
declare function nested(): void;
declare function conditionalAfter(): void;
declare function dominatedRight(): void;
declare function methodAfter(): void;
declare function switchSibling(): void;
declare function switchDominated(): void;
const inline = async () => {
  await new Promise<void>(resolve => setTimeout(resolve, 1));
  after();
  queueMicrotask(() => nested());
};
const conditional = async (wait: boolean) => {
  wait && (await Promise.resolve());
  conditionalAfter();
};
const dominated = async (read: boolean) => {
  await Promise.resolve();
  read && dominatedRight();
};
const object = {
  async method() {
    await Promise.resolve();
    methodAfter();
  },
};
const switched = async (kind: "wait" | "read") => {
  switch (kind) {
    case "wait":
      await Promise.resolve();
      break;
    case "read":
      switchSibling();
      break;
  }
};
const fullyDominatedSwitch = async (kind: "first" | "second") => {
  switch (kind) {
    case "first":
      await Promise.resolve(1);
      break;
    case "second":
      await Promise.resolve(2);
      break;
    default:
      await Promise.resolve(3);
  }
  switchDominated();
};
function promised() { return Promise.resolve(1); }
type Stream<T> = AsyncIterable<T>;
declare function makeStream(): AsyncIterable<number>;
function aliasedStream(): Stream<number> { return makeStream(); }
function maybeStream(stream: boolean): AsyncIterable<number> | number {
  return stream ? makeStream() : 1;
}
function genericStream<T extends AsyncIterable<number>>(value: T): T { return value; }
interface CustomStream extends AsyncIterable<number> {}
function customStream(value: CustomStream): CustomStream { return value; }
`
	path := filepath.Join(root, "async.ts")
	writeProjectFile(t, root, "async.ts", source)
	project := openProject(t, root)
	discoverer := project.(typefacts.AsyncFunctionDiscoverer)
	facts, err := discoverer.SourceAsyncFunctions(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	asyncFunctions, promisedFunctions := 0, 0
	afterCalls, nestedCalls, conditionalCalls, dominatedCalls := 0, 0, 0, 0
	methodCalls, switchSiblingCalls, switchDominatedCalls := 0, 0, 0
	for _, fact := range facts {
		if fact.CanReturnAsync {
			asyncFunctions++
		}
		if fact.Symbol != "" && fact.CanReturnAsync {
			promisedFunctions++
		}
		for _, call := range fact.CallsAfterAwait {
			name := source[call.StartByte:call.EndByte]
			switch name {
			case "after":
				afterCalls++
			case "nested":
				nestedCalls++
			case "conditionalAfter":
				conditionalCalls++
			case "dominatedRight":
				dominatedCalls++
			case "methodAfter":
				methodCalls++
			case "switchSibling":
				switchSiblingCalls++
			case "switchDominated":
				switchDominatedCalls++
			}
		}
	}
	if asyncFunctions < 11 || promisedFunctions < 11 || afterCalls != 1 || nestedCalls != 0 ||
		conditionalCalls != 0 || dominatedCalls != 1 || methodCalls != 1 ||
		switchSiblingCalls != 0 || switchDominatedCalls != 1 {
		t.Fatalf(
			"facts=%#v; async=%d promised=%d after=%d nested=%d conditional=%d dominated=%d method=%d switchSibling=%d switchDominated=%d",
			facts,
			asyncFunctions,
			promisedFunctions,
			afterCalls,
			nestedCalls,
			conditionalCalls,
			dominatedCalls,
			methodCalls,
			switchSiblingCalls,
			switchDominatedCalls,
		)
	}
}

func TestProjectLooksUpOnlyDemandedAsyncFunctionsAndAliases(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true,"target":"ES2022"},"include":["*.ts"]}`)
	source := `
declare function consume(callback: () => unknown): void;
declare function after(): void;
const unused = async () => Promise.resolve("unused");
const selected = () => Promise.resolve("selected");
const alias = selected;
consume(alias);
consume(() => Promise.resolve("inline"));
async function awaited() {
  await Promise.resolve();
  after();
}
`
	path := filepath.Join(root, "lookup.ts")
	writeProjectFile(t, root, "lookup.ts", source)
	project := openProject(t, root)
	lookup, ok := project.(typefacts.AsyncFunctionLookup)
	if !ok {
		t.Fatal("project does not expose demand-shaped async lookup")
	}
	location := func(needle string) typefacts.Location {
		t.Helper()
		start := strings.Index(source, needle)
		if start < 0 {
			t.Fatalf("%q not found in fixture", needle)
		}
		return typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)}
	}
	locations := []typefacts.Location{
		location("alias);"),
		location("() => Promise.resolve(\"inline\")"),
		location("await Promise.resolve()"),
	}
	locations[0].EndByte = locations[0].StartByte + len("alias")
	facts, err := lookup.AsyncFunctionsAt(context.Background(), locations)
	if err != nil {
		t.Fatal(err)
	}
	again, err := lookup.AsyncFunctionsAt(context.Background(), locations)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(facts, again) {
		t.Fatalf("memoized lookup changed facts:\nfirst  %#v\nsecond %#v", facts, again)
	}
	var expressions []string
	var afterCalls int
	for _, fact := range facts {
		expressions = append(expressions, source[fact.Expression.StartByte:fact.Expression.EndByte])
		for _, call := range fact.CallsAfterAwait {
			if source[call.StartByte:call.EndByte] == "after" {
				afterCalls++
			}
		}
	}
	joined := strings.Join(expressions, "\n")
	for _, wanted := range []string{
		`() => Promise.resolve("selected")`,
		"selected",
		`() => Promise.resolve("inline")`,
		"async function awaited()",
	} {
		if !strings.Contains(joined, wanted) {
			t.Fatalf("lookup facts omit %q: %#v", wanted, expressions)
		}
	}
	if strings.Contains(joined, "unused") {
		t.Fatalf("lookup classified unrelated function: %#v", expressions)
	}
	if afterCalls != 1 {
		t.Fatalf("after-await calls = %d, want 1; facts=%#v", afterCalls, facts)
	}
}

// TestAsyncFunctionsAtCrossFileDeclarationInScriptFiles pins the reachability
// of cross-path async facts, which is what the closure's crossPathAsync
// bail-out defends against.
//
// appendAsyncSymbolFacts walks symbol.ValueDeclaration and never resolves
// import aliases, so in a module-kind project a cross-file reference dead-ends
// at the import specifier and every fact stays local. Script-kind files (no
// imports, no exports) share one global scope, so an identifier resolves
// straight to the declaration in another file — and a demand in one file yields
// a fact whose expression lives in another. Deleting the bail-out on the
// strength of module-only evidence would silently drop those facts from
// retained state.
func TestAsyncFunctionsAtCrossFileDeclarationInScriptFiles(t *testing.T) {
	ctx := context.Background()
	root, err := filepath.Abs(filepath.Join("testdata", "retained-scripts"))
	if err != nil {
		t.Fatal(err)
	}
	backend, err := tsgo.OpenProject(ctx, filepath.Join(root, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = backend.Close() })

	demanded := filepath.Clean(filepath.Join(root, "use-alpha.ts"))
	calls, err := backend.(typefacts.CallDiscoverer).SourceCalls(ctx, demanded)
	if err != nil {
		t.Fatal(err)
	}
	locations := make([]typefacts.Location, 0, len(calls))
	for _, call := range calls {
		locations = append(locations, call.Callee)
	}
	facts, err := backend.(typefacts.AsyncFunctionLookup).AsyncFunctionsAt(ctx, locations)
	if err != nil {
		t.Fatal(err)
	}
	foreign := make([]string, 0)
	for _, fact := range facts {
		if where := filepath.Clean(fact.Expression.Path); where != demanded {
			foreign = append(foreign, filepath.Base(where))
		}
	}
	if len(foreign) == 0 {
		t.Fatalf("no async fact resolved outside the demanded file, so crossPathAsync is unreachable here; facts=%#v", facts)
	}
	t.Logf("async demands in use-alpha.ts produced %d fact(s) declared in %v", len(foreign), foreign)
}
