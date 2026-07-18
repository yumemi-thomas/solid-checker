package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

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
			opened, err := OpenProject(ctx, configPath)
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
