package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// TestDefaultLibraryInvokerOverAShippedRuntimeArtifact runs the invoker table
// over the shape certification actually sees: a bundled `.js` runtime artifact
// with `allowJs` on and `checkJs` off, paired with the published `.d.ts`. It
// pins which halves of the table survive that erasure.
//
// A bare global — `setTimeout`, `requestAnimationFrame` — still resolves,
// because the global's own declarations are in the default library no matter how
// the calling file is typed. That is why `@solid-primitives/utils::afterPaint`
// is provable.
//
// A member on a parameter does not. In a `.js` file with no annotations and no
// JSDoc, the compiler gives the parameter `any`, so `node.addEventListener`
// resolves to no symbol at all — which is exactly the `any`-typed receiver the
// invoker table already refuses. That is not a gap in the table; it is the
// table refusing to assert what it cannot resolve, and it is why
// `@solid-primitives/gestures@1.2.1::registerPointerListener` stays open.
func TestDefaultLibraryInvokerOverAShippedRuntimeArtifact(t *testing.T) {
	dir := t.TempDir()
	runtime := `export function registerListener(node, downCallback) {
  const handler = (event) => { downCallback(event); };
  node.addEventListener("pointerdown", handler);
}
export function schedulePaint(callback) {
  requestAnimationFrame(() => requestAnimationFrame(callback));
}
`
	declarations := `export declare function registerListener(node: HTMLElement, downCallback: (event: PointerEvent) => void): void;
export declare function schedulePaint(callback: () => void): void;
`
	harness := `import { registerListener, schedulePaint } from "./artifact.js";
void registerListener;
void schedulePaint;
`
	write := func(name, contents string) {
		t.Helper()
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	// The same options the private certification project uses.
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext","allowJs":true,"checkJs":false,"allowImportingTsExtensions":true,"moduleDetection":"force","types":[]},"files":["artifact.js","artifact.d.ts","harness.ts"]}`)
	write("artifact.js", runtime)
	write("artifact.d.ts", declarations)
	write("harness.ts", harness)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	harnessPath := filepath.Join(dir, "harness.ts")
	runtimePath := filepath.Join(dir, "artifact.js")
	demands := make([]typefacts.ExportValueDemand, 0, 2)
	for _, name := range []string{"registerListener", "schedulePaint"} {
		queryStart := strings.LastIndex(harness, "void "+name+";") + len("void ")
		implementationStart := strings.Index(runtime, "function "+name) + len("function ")
		demands = append(demands, typefacts.ExportValueDemand{
			Location: typefacts.Location{
				Path: harnessPath, StartByte: queryStart, EndByte: queryStart + len(name),
			},
			ImplementationLocation: &typefacts.Location{
				Path:      runtimePath,
				StartByte: implementationStart,
				EndByte:   implementationStart + len(name),
			},
		})
	}
	answer, err := analyzer.ExportValueTranscripts(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}

	// The listener registration: the argument slot does carry the handler — the
	// `const` descent is untouched by erasure — but the callee is unresolvable,
	// so there is no invoker fact and the position stays open.
	listener := answer.Transcripts[0].Implementation
	if listener == nil {
		t.Fatalf("registerListener has no implementation census: %#v", answer.Transcripts[0])
	}
	registration := callAt(t, listener, runtime, `node.addEventListener("pointerdown", handler)`)
	if len(argumentCallables(registration, 1)) != 1 {
		t.Fatalf(
			"registration argument callables = %#v, want the handler carried at slot 1",
			registration.ArgumentCallables,
		)
	}
	if registration.DefaultLibraryInvoker != "" || len(registration.InvokedArguments) != 0 {
		t.Fatalf(
			"registration invoker = %q slots = %#v: an `any` receiver resolves to no symbol, so the table must stay silent",
			registration.DefaultLibraryInvoker, registration.InvokedArguments,
		)
	}

	// The bare global survives erasure, because its own declarations are what
	// the table resolves and they live in the default library.
	paint := answer.Transcripts[1].Implementation
	if paint == nil {
		t.Fatalf("schedulePaint has no implementation census: %#v", answer.Transcripts[1])
	}
	outer := callAt(t, paint, runtime, "requestAnimationFrame(() => requestAnimationFrame(callback))")
	if outer.DefaultLibraryInvoker != typefacts.DefaultLibraryInvokerRequestAnimationFrame ||
		len(outer.InvokedArguments) != 1 || outer.InvokedArguments[0] != 0 {
		t.Fatalf("outer rAF invoker = %q slots = %#v", outer.DefaultLibraryInvoker, outer.InvokedArguments)
	}
	if len(argumentCallables(outer, 0)) != 1 {
		t.Fatalf("outer rAF argument callables = %#v, want the inner arrow", outer.ArgumentCallables)
	}
	inner := callAt(t, paint, runtime, "requestAnimationFrame(callback)")
	if !inner.Captured || !locationWithinAny(inner.Location, argumentCallables(outer, 0)) {
		t.Fatalf("inner rAF %#v is not inside the carried arrow", inner.Location)
	}
}
