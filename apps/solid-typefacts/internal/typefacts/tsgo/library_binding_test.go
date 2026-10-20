package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestStandardGlobalLibraryBindingSurvivesOnlyUnwrittenGenerations(t *testing.T) {
	dir := t.TempDir()
	source := `queueMicrotask(() => {});
(queueMicrotask as typeof queueMicrotask)(() => {});
declare const unknownScheduler: typeof queueMicrotask;
unknownScheduler(() => {});
`
	for name, contents := range map[string]string{
		"tsconfig.json": `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`,
		"calls.ts":      source,
		"writes.ts":     "export {};\n",
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	calls := []string{`queueMicrotask(() => {})`, `(queueMicrotask as typeof queueMicrotask)(() => {})`, `unknownScheduler(() => {})`}
	demands := make([]typefacts.EntityDemand, len(calls))
	for i, call := range calls {
		start := strings.Index(source, call)
		demands[i] = typefacts.EntityDemand{Location: typefacts.Location{Path: filepath.Join(dir, "calls.ts"), StartByte: start, EndByte: start + len(call)}, ResolvedCall: true}
	}
	check := func(want bool) {
		t.Helper()
		entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), demands)
		if err != nil {
			t.Fatal(err)
		}
		if len(entities) != len(calls) {
			t.Fatalf("got %d calls", len(entities))
		}
		for i, entity := range entities {
			call := entity.ResolvedCall
			if call == nil || call.Declaration == nil || !call.Declaration.StandardLibrary {
				t.Fatalf("selected signature must remain native for %s: %+v", calls[i], call)
			}
			if (call.DefaultLibraryBinding != nil) != (want && i < 2) {
				t.Fatalf("%s binding = %+v, want %t", calls[i], call.DefaultLibraryBinding, want && i < 2)
			}
		}
	}
	check(true)
	for i, contents := range []string{
		`queueMicrotask = callback => callback();`,
		`export {};`,
		`globalThis.queueMicrotask = callback => callback();`,
		`export {};`,
	} {
		if _, err := opened.Update(context.Background(), []typefacts.FileChange{{Path: filepath.Join(dir, "writes.ts"), Version: uint64(i + 1), Source: []byte(contents)}}); err != nil {
			t.Fatal(err)
		}
		check(i%2 == 1)
	}
}

func TestAugmentedGlobalLibraryBindingAndRetainedWrites(t *testing.T) {
	dir := t.TempDir()
	source := `import { schedule } from "./other";
setTimeout(() => {}, 1);
(setTimeout as typeof setTimeout)(() => {}, 1);
declare const alias: typeof setTimeout;
alias(() => {}, 1);
declare const holder: { schedule: typeof setTimeout };
holder.schedule(() => {}, 1);
schedule(() => {}, 3);
function shadow(setTimeout: typeof schedule) { setTimeout(() => {}, 2); }
`
	files := map[string]string{
		"tsconfig.json":  `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`,
		"calls.ts":       source,
		"overloads.d.ts": `declare function setTimeout<TArgs extends any[]>(callback: (...args: TArgs) => void, ms?: number, ...args: TArgs): { timer: true };`,
		"other.ts":       `export function schedule(callback: () => void, ms: number) { callback(); }`,
		"writes.ts":      "export {};\n",
	}
	for name, text := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	calls := []string{`setTimeout(() => {}, 1)`, `(setTimeout as typeof setTimeout)(() => {}, 1)`, `alias(() => {}, 1)`, `holder.schedule(() => {}, 1)`, `schedule(() => {}, 3)`, `setTimeout(() => {}, 2)`}
	demands := make([]typefacts.EntityDemand, len(calls))
	for i, call := range calls {
		start := strings.Index(source, call)
		demands[i] = typefacts.EntityDemand{Location: typefacts.Location{Path: filepath.Join(dir, "calls.ts"), StartByte: start, EndByte: start + len(call)}, ResolvedCall: true}
	}
	state := files["writes.ts"]
	check := func(want bool) {
		t.Helper()
		entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), demands)
		if err != nil {
			t.Fatal(err)
		}
		for i, entity := range entities {
			call := entity.ResolvedCall
			if call == nil || call.Validity != typefacts.ResolvedCallValid {
				t.Fatalf("%s: %+v", calls[i], call)
			}
			binding := call.DefaultLibraryBinding
			if (binding != nil) != (want && i < 2) {
				t.Fatalf("%s library binding = %+v, want %t after %s", calls[i], binding, want && i < 2, state)
			}
			if i < 2 {
				if call.Declaration == nil || call.Declaration.StandardLibrary || !strings.HasSuffix(call.Declaration.SourceFile, "overloads.d.ts") {
					t.Fatalf("selected overload was replaced: %+v", call)
				}
				if len(call.Arguments) != 2 || call.Arguments[0].Parameter == nil {
					t.Fatalf("selected mapping lost: %+v", call)
				}
				if binding != nil && (!binding.StandardLibrary || binding.QualifiedName != "setTimeout") {
					t.Fatalf("wrong binding: %+v", binding)
				}
			}
		}
	}
	check(true)
	for i, text := range []string{
		`setTimeout = (() => ({timer: true})) as typeof setTimeout;`,
		files["writes.ts"],
		`globalThis.setTimeout = (() => ({timer: true})) as typeof setTimeout;`,
		files["writes.ts"],
		`window.setTimeout = (() => 1) as typeof window.setTimeout;`,
		files["writes.ts"],
		`window["setTimeout"] = (() => 1) as typeof window.setTimeout;`,
		files["writes.ts"],
		"window[`setTimeout`] = (() => 1) as typeof window.setTimeout;",
		files["writes.ts"],
		`delete (window as any)["setTimeout"];`,
		files["writes.ts"],
	} {
		state = text
		if _, err := opened.Update(context.Background(), []typefacts.FileChange{{Path: filepath.Join(dir, "writes.ts"), Version: uint64(i + 1), Source: []byte(text)}}); err != nil {
			t.Fatal(err)
		}
		check(i%2 == 1)
	}
}
