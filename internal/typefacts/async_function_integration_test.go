package typefacts_test

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

func TestProjectDiscoversSemanticAsyncFunctionsAndPostAwaitCalls(t *testing.T) {
	root := t.TempDir()
	writeProjectFile(t, root, "tsconfig.json", `{"compilerOptions":{"strict":true,"target":"ES2022"},"include":["*.ts"]}`)
	source := `
declare function after(): void;
declare function nested(): void;
const inline = async () => {
  await new Promise<void>(resolve => setTimeout(resolve, 1));
  after();
  queueMicrotask(() => nested());
};
function promised() { return Promise.resolve(1); }
`
	path := filepath.Join(root, "async.ts")
	writeProjectFile(t, root, "async.ts", source)
	project := openProject(t, root)
	discoverer := project.(typefacts.AsyncFunctionDiscoverer)
	facts, err := discoverer.SourceAsyncFunctions(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	asyncFunctions, promisedFunctions, afterCalls, nestedCalls := 0, 0, 0, 0
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
			}
		}
	}
	if asyncFunctions < 2 || promisedFunctions < 2 || afterCalls != 1 || nestedCalls != 0 {
		t.Fatalf("facts=%#v; async=%d promised=%d after=%d nested=%d", facts, asyncFunctions, promisedFunctions, afterCalls, nestedCalls)
	}
}
