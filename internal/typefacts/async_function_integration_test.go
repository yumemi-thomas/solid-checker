package typefacts_test

import (
	"context"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
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
	fused, ok := project.(typefacts.FileFactsDiscoverer)
	if !ok {
		t.Fatal("project does not expose fused file facts")
	}
	fileFacts, err := fused.SourceFileFacts(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(fileFacts.AsyncFunctions, facts) {
		t.Fatalf(
			"fused and standalone async facts diverged:\nstandalone %#v\nfused      %#v",
			facts,
			fileFacts.AsyncFunctions,
		)
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
