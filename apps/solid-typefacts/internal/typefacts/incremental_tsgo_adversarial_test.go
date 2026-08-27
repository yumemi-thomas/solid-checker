package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

func writeAdversarialProject(t *testing.T, files map[string]string) (string, map[string][]byte) {
	t.Helper()
	root := t.TempDir()
	originals := make(map[string][]byte, len(files))
	names := make([]string, 0, len(files))
	for name := range files {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		source := []byte(files[name])
		path := filepath.Join(root, name)
		if err := os.WriteFile(path, source, 0o600); err != nil {
			t.Fatal(err)
		}
		originals[name] = source
	}
	return root, originals
}

func assertTSGoAlternatingScriptMatchesFresh(
	t *testing.T,
	files map[string]string,
	script func(root string, originals map[string][]byte) []typefacts.FileChange,
) {
	t.Helper()
	ctx := context.Background()
	root, originals := writeAdversarialProject(t, files)
	projectID := filepath.Join(root, "tsconfig.json")
	changes := script(root, originals)

	open := func() (*typefacts.DemandClosure, demandSource) {
		t.Helper()
		backend, err := tsgo.OpenProject(ctx, projectID, nil)
		if err != nil {
			t.Fatal(err)
		}
		closure, err := typefacts.NewDemandClosure(backend, nil)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = closure.Close() })
		return closure, backend.(demandSource)
	}

	incremental, incrementalBackend := open()
	initialDemands := realisticDemands(t, incrementalBackend, ctx)
	if _, err := incremental.DemandTableForGroups(
		ctx, 1, groupedDemands(initialDemands), demandPaths(initialDemands),
	); err != nil {
		t.Fatal(err)
	}

	for step, change := range changes {
		if _, err := incremental.Update(ctx, []typefacts.FileChange{change}); err != nil {
			t.Fatalf("incremental update %d: %v", step, err)
		}
		generation := uint64(step + 2)
		retainedDemands := realisticDemands(t, incrementalBackend, ctx)
		retained, err := incremental.DemandTableForGroups(
			ctx, generation, groupedDemands(retainedDemands), nil,
		)
		if err != nil {
			t.Fatalf("incremental materialization %d: %v", step, err)
		}

		fresh, freshBackend := open()
		for replay := 0; replay <= step; replay++ {
			if _, err := fresh.Update(ctx, []typefacts.FileChange{changes[replay]}); err != nil {
				t.Fatalf("fresh replay %d/%d: %v", replay, step, err)
			}
		}
		freshDemands := realisticDemands(t, freshBackend, ctx)
		whole, err := fresh.DemandTableForGroups(
			ctx, generation, canonicalDemandGroups(freshDemands), nil,
		)
		if err != nil {
			t.Fatalf("fresh materialization %d: %v", step, err)
		}
		assertFullWireTransitionsIdentical(t, "adversarial retained table", step, projectID, retained, whole)
	}
}

func alternatingFileChanges(root, name string, before, after []byte, count int) []typefacts.FileChange {
	changes := make([]typefacts.FileChange, 0, count)
	for index := 0; index < count; index++ {
		source := after
		if index%2 != 0 {
			source = before
		}
		changes = append(changes, typefacts.FileChange{
			Path: filepath.Join(root, name), Version: uint64(index + 1), Source: source,
		})
	}
	return changes
}

func TestModuleAndGlobalAugmentationAlternatingGenerationsMatchFresh(t *testing.T) {
	config := `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}`
	tests := []struct {
		name   string
		files  map[string]string
		target string
		after  string
	}{
		{
			name: "module augmentation",
			files: map[string]string{
				"tsconfig.json": config,
				"base.ts":       "export interface Box { value: number }\nexport const box: Box = { value: 1, extra: \"x\" } as Box;\n",
				"augment.ts":    "import \"./base\";\ndeclare module \"./base\" { interface Box { extra: string } }\nexport {};\n",
				"consumer.ts":   "import { box } from \"./base\";\nfunction read(): string { return box.extra; }\nexport const result = read();\n",
				"unrelated.ts":  "export function alone(): number { return 1; }\nexport const local = alone();\n",
			},
			target: "augment.ts",
			after:  "import \"./base\";\ndeclare module \"./base\" { interface Box { extra: number } }\nexport {};\n",
		},
		{
			name: "global augmentation",
			files: map[string]string{
				"tsconfig.json": config,
				"augment.ts":    "declare global { interface Window { extra: string } }\nexport {};\n",
				"consumer.ts":   "function read(): string { return window.extra; }\nexport const result = read();\n",
				"unrelated.ts":  "export function alone(): number { return 1; }\nexport const local = alone();\n",
			},
			target: "augment.ts",
			after:  "declare global { interface Window { extra: number } }\nexport {};\n",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			assertTSGoAlternatingScriptMatchesFresh(t, test.files, func(root string, originals map[string][]byte) []typefacts.FileChange {
				return alternatingFileChanges(root, test.target, originals[test.target], []byte(test.after), 6)
			})
		})
	}
}

func TestInferredExportAlternatingGenerationsMatchFresh(t *testing.T) {
	files := map[string]string{
		"tsconfig.json": `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}`,
		"dependency.ts": "export function value() { return 1; }\n",
		"consumer.ts":   "import { value } from \"./dependency\";\nexport const result = value();\n",
		"unrelated.ts":  "export function alone(): number { return 1; }\nexport const local = alone();\n",
	}
	assertTSGoAlternatingScriptMatchesFresh(t, files, func(root string, originals map[string][]byte) []typefacts.FileChange {
		return alternatingFileChanges(
			root,
			"dependency.ts",
			originals["dependency.ts"],
			[]byte("export function value() { return \"changed\"; }\n"),
			8,
		)
	})
}

func TestDeletionRestorationAndConfigAlternationMatchFresh(t *testing.T) {
	files := map[string]string{
		"tsconfig.json": `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":true},"include":["*.ts"]}`,
		"dependency.ts": "export function value(): number { return 1; }\n",
		"consumer.ts":   "import { value } from \"./dependency\";\nexport const result = value();\n",
		"unrelated.ts":  "export function alone(): number { return 1; }\nexport const local = alone();\n",
	}
	assertTSGoAlternatingScriptMatchesFresh(t, files, func(root string, originals map[string][]byte) []typefacts.FileChange {
		configLoose := []byte(`{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext","strict":false},"include":["*.ts"]}`)
		return []typefacts.FileChange{
			{Path: filepath.Join(root, "dependency.ts"), Version: 1, Deleted: true},
			{Path: filepath.Join(root, "dependency.ts"), Version: 2, Source: originals["dependency.ts"]},
			{Path: filepath.Join(root, "tsconfig.json"), Version: 1, Source: configLoose},
			{Path: filepath.Join(root, "tsconfig.json"), Version: 2, Source: originals["tsconfig.json"]},
			{Path: filepath.Join(root, "dependency.ts"), Version: 3, Deleted: true},
			{Path: filepath.Join(root, "dependency.ts"), Version: 4, Source: originals["dependency.ts"]},
		}
	})
}
