package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestExportValueTranscriptResolvesImportedAliasWithoutInventingCall(t *testing.T) {
	dir := t.TempDir()
	packageDir := filepath.Join(dir, "node_modules", "fixture-package")
	if err := os.MkdirAll(packageDir, 0o755); err != nil {
		t.Fatal(err)
	}
	write := func(path, source string) {
		t.Helper()
		if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write(filepath.Join(packageDir, "package.json"), `{"name":"fixture-package","version":"1.0.0","types":"index.d.ts"}`)
	write(filepath.Join(packageDir, "index.d.ts"), `export declare const nestedValue: { nested: { callback: (value: number) => void } };`)
	write(filepath.Join(dir, "tsconfig.json"), `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["harness.ts"]}`)
	source := `import { nestedValue as __solid_checker_export_0 } from "fixture-package";
void __solid_checker_export_0;
`
	harness := filepath.Join(dir, "harness.ts")
	write(harness, source)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer, ok := opened.(typefacts.ExportValueAnalyzer)
	if !ok {
		t.Fatal("TypeScript-Go project does not implement ExportValueAnalyzer")
	}
	needle := "__solid_checker_export_0"
	start := strings.LastIndex(source, needle)
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location:      typefacts.Location{Path: harness, StartByte: start, EndByte: start + len(needle)},
		CallableDepth: 3,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(answer.Transcripts) != 1 {
		t.Fatalf("transcripts = %d, want 1", len(answer.Transcripts))
	}
	transcript := answer.Transcripts[0]
	if !transcript.Complete || len(transcript.OpenReasons) != 0 {
		t.Fatalf("transcript = %#v, want exact closed identity", transcript)
	}
	if transcript.QueryName != needle || transcript.Declaration == nil || transcript.Target == "" {
		t.Fatalf("transcript identity = %#v, want query, target, and declaration", transcript)
	}
	if got := filepath.Clean(transcript.Declaration.Location.Path); !strings.HasSuffix(got, filepath.Join("node_modules", "fixture-package", "index.d.ts")) {
		t.Fatalf("declaration path = %q, want package declaration", got)
	}
	found := false
	for _, path := range transcript.CallablePaths {
		if len(path.Path) == 2 && path.Path[0].Property == "nested" && path.Path[1].Property == "callback" {
			found = path.Complete && path.Callability == typefacts.CallabilityCallable
		}
	}
	if !found {
		t.Fatalf("callable paths = %#v, want exact nested.callback", transcript.CallablePaths)
	}
}

func TestExportValueTranscriptKeepsUnknownOpen(t *testing.T) {
	dir := t.TempDir()
	source := "declare const value: unknown;\nvoid value;\n"
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	start := strings.LastIndex(source, "value")
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: start, EndByte: start + len("value")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	if got := answer.Transcripts[0].Value.Callability; got != typefacts.CallabilityUnknown {
		t.Fatalf("callability = %q, want unknown rather than a positive or negative", got)
	}
}

func TestExportValueTranscriptSelectionFailureHasWireValidUnknownValue(t *testing.T) {
	dir := t.TempDir()
	source := "const value = 1;\n"
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	analyzer := opened.(typefacts.ExportValueAnalyzer)
	answer, err := analyzer.ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
		Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: 0, EndByte: len("const")},
	}})
	if err != nil {
		t.Fatal(err)
	}
	transcript := answer.Transcripts[0]
	if transcript.Complete || len(transcript.OpenReasons) == 0 {
		t.Fatalf("transcript = %#v, want an explicit selection refusal", transcript)
	}
	if transcript.Value.Callability != typefacts.CallabilityUnknown ||
		transcript.Value.Constructability != typefacts.InvocationConstructUnknown ||
		!transcript.Value.Primitive.Unknown {
		t.Fatalf("value = %#v, want a wire-valid unknown domain", transcript.Value)
	}
	if len(transcript.Value.OpenReasons) != 1 || transcript.Value.OpenReasons[0] != "valueUnavailable" {
		t.Fatalf("value open reasons = %#v", transcript.Value.OpenReasons)
	}
}
