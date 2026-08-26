package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

type updateMetricTrace struct {
	updates []map[string]int64
}

func (*updateMetricTrace) Stage(string, time.Duration) {}

func (t *updateMetricTrace) Metrics(name string, values ...typefacts.Metric) {
	if name != "update" {
		return
	}
	update := make(map[string]int64, len(values))
	for _, value := range values {
		update[value.Key] = value.Value
	}
	t.updates = append(t.updates, update)
}

func TestLeafUpdateSkipsDeclarationProofUntilItGainsAnImporter(t *testing.T) {
	dir := t.TempDir()
	configPath := filepath.Join(dir, "tsconfig.json")
	leafPath := filepath.Join(dir, "leaf.ts")
	consumerPath := filepath.Join(dir, "consumer.ts")
	for path, source := range map[string]string{
		configPath:   `{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`,
		leafPath:     "export const value = 1;\n",
		consumerPath: "export const local = true;\n",
	} {
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	ctx := context.Background()
	trace := &updateMetricTrace{}
	opened, err := OpenProject(ctx, configPath, trace)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()

	leafSource := "export const value = \"changed\";\nexport const added = 1;\n"
	affected, err := opened.Update(ctx, []typefacts.FileChange{{
		Path: leafPath, Version: 1, Source: []byte(leafSource),
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(affected.Files) != 1 || filepath.Clean(affected.Files[0]) != filepath.Clean(leafPath) {
		t.Fatalf("leaf affected set = %v, want only %q", affected.Files, leafPath)
	}
	if len(trace.updates) != 1 || trace.updates[0]["leafCutoff"] != 1 {
		t.Fatalf("leaf update did not take the no-importer cutoff: %+v", trace.updates)
	}
	valueStart := strings.Index(leafSource, "value")
	if _, err := opened.SymbolAt(ctx, typefacts.Location{
		Path: leafPath, StartByte: valueStart, EndByte: valueStart + len("value"),
	}); err != nil {
		t.Fatalf("resolve changed leaf export: %v", err)
	}
	addedStart := strings.Index(leafSource, "added")
	if _, err := opened.SymbolAt(ctx, typefacts.Location{
		Path: leafPath, StartByte: addedStart, EndByte: addedStart + len("added"),
	}); err != nil {
		t.Fatalf("resolve added leaf export: %v", err)
	}

	consumerSource := "import { value } from \"./leaf\";\nexport const local = value;\n"
	if _, err := opened.Update(ctx, []typefacts.FileChange{{
		Path: consumerPath, Version: 1, Source: []byte(consumerSource),
	}}); err != nil {
		t.Fatal(err)
	}
	affected, err = opened.Update(ctx, []typefacts.FileChange{{
		Path: leafPath, Version: 2, Source: []byte("export const value = 2;\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	if trace.updates[len(trace.updates)-1]["leafCutoff"] != 0 {
		t.Fatalf("imported leaf unexpectedly took no-importer cutoff: %+v", trace.updates)
	}
	if !slices.ContainsFunc(affected.Files, func(path string) bool {
		return filepath.Clean(path) == filepath.Clean(consumerPath)
	}) {
		t.Fatalf("shape-changing leaf update did not affect importer: %v", affected.Files)
	}
}

func TestSemanticAffectedSetCutoff(t *testing.T) {
	tests := []struct {
		name             string
		before           string
		after            string
		importerAffected bool
		preserveExportID bool
	}{
		{
			name:             "annotated body edit stops at the edited file",
			before:           "export function value(): number {\n  return 1;\n}\n",
			after:            "export function value(): number {\n  return 2;\n}\n",
			importerAffected: false,
		},
		{
			name:             "inferred exported return change propagates",
			before:           "export function value() {\n  return 1;\n}\n",
			after:            "export function value() {\n  return \"changed\";\n}\n",
			importerAffected: true,
		},
		{
			name: "span shift above an unchanged export preserves identity",
			before: "const local = 1;\n" +
				"export function value(): number {\n  return 1;\n}\n",
			after: "const local = 100000;\n" +
				"export function value(): number {\n  return 1;\n}\n",
			importerAffected: false,
			preserveExportID: true,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			dir := t.TempDir()
			configPath := filepath.Join(dir, "tsconfig.json")
			dependencyPath := filepath.Join(dir, "dependency.ts")
			importerPath := filepath.Join(dir, "importer.ts")
			if err := os.WriteFile(
				configPath,
				[]byte(`{"compilerOptions":{"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`),
				0o644,
			); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(dependencyPath, []byte(test.before), 0o644); err != nil {
				t.Fatal(err)
			}
			importerSource := "import { value } from \"./dependency\";\nexport const result = value();\n"
			if err := os.WriteFile(
				importerPath,
				[]byte(importerSource),
				0o644,
			); err != nil {
				t.Fatal(err)
			}

			ctx := context.Background()
			opened, err := OpenProject(ctx, configPath, nil)
			if err != nil {
				t.Fatal(err)
			}
			defer opened.Close()

			var beforeExportID typefacts.SymbolID
			var importerAliasID typefacts.SymbolID
			if test.preserveExportID {
				start := strings.Index(test.before, "value")
				beforeExportID, err = opened.SymbolAt(ctx, typefacts.Location{
					Path:      dependencyPath,
					StartByte: start,
					EndByte:   start + len("value"),
				})
				if err != nil {
					t.Fatal(err)
				}
				importStart := strings.Index(importerSource, "value")
				importerAliasID, err = opened.SymbolAt(ctx, typefacts.Location{
					Path:      importerPath,
					StartByte: importStart,
					EndByte:   importStart + len("value"),
				})
				if err != nil {
					t.Fatal(err)
				}
				target, err := opened.ResolveAlias(ctx, importerAliasID)
				if err != nil {
					t.Fatal(err)
				}
				if target != beforeExportID {
					t.Fatalf("import target before edit = %q, want export %q", target, beforeExportID)
				}
			}
			affected, err := opened.Update(ctx, []typefacts.FileChange{{
				Path:    dependencyPath,
				Version: 1,
				Source:  []byte(test.after),
			}})
			if err != nil {
				t.Fatal(err)
			}
			importerAffected := false
			for _, path := range affected.Files {
				if filepath.Clean(path) == filepath.Clean(importerPath) {
					importerAffected = true
				}
			}
			if importerAffected != test.importerAffected {
				t.Fatalf(
					"importer affected = %t, want %t; affected set: %v",
					importerAffected,
					test.importerAffected,
					affected.Files,
				)
			}
			if test.preserveExportID {
				start := strings.Index(test.after, "value")
				afterExportID, err := opened.SymbolAt(ctx, typefacts.Location{
					Path:      dependencyPath,
					StartByte: start,
					EndByte:   start + len("value"),
				})
				if err != nil {
					t.Fatal(err)
				}
				if afterExportID != beforeExportID {
					t.Fatalf("export ID after span shift = %q, want preserved %q", afterExportID, beforeExportID)
				}
				target, err := opened.ResolveAlias(ctx, importerAliasID)
				if err != nil {
					t.Fatal(err)
				}
				if target != beforeExportID {
					t.Fatalf("retained import target after span shift = %q, want export %q", target, beforeExportID)
				}

				affected, err := opened.Update(ctx, []typefacts.FileChange{{
					Path:    dependencyPath,
					Version: 2,
					Source:  []byte(test.before),
				}})
				if err != nil {
					t.Fatal(err)
				}
				for _, path := range affected.Files {
					if filepath.Clean(path) == filepath.Clean(importerPath) {
						t.Fatalf("second span shift unexpectedly affected importer: %v", affected.Files)
					}
				}
				start = strings.Index(test.before, "value")
				restoredID, err := opened.SymbolAt(ctx, typefacts.Location{
					Path:      dependencyPath,
					StartByte: start,
					EndByte:   start + len("value"),
				})
				if err != nil {
					t.Fatal(err)
				}
				if restoredID != beforeExportID {
					t.Fatalf("export ID after second span shift = %q, want preserved %q", restoredID, beforeExportID)
				}
			}
		})
	}
}
