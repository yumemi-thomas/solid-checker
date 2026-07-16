package typefacts_test

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
	"github.com/yumemi-thomas/solid-check/internal/typefacts/tsgo"
)

func TestProjectResolveAliasRejectsOrdinarySymbolsWithoutPanicking(t *testing.T) {
	project, fixture := openAliasedProject(t)
	sourcePath := filepath.Join(fixture, "source.ts")
	location := locationOf(t, sourcePath, "count =")
	location.EndByte = location.StartByte + len("count")
	symbol, err := project.SymbolAt(context.Background(), location)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := project.ResolveAlias(context.Background(), symbol); !errors.Is(err, typefacts.ErrNotFound) {
		t.Fatalf("ResolveAlias() error = %v, want ErrNotFound", err)
	}
}

func TestProjectResolvesImportedAliasToOriginalDeclaration(t *testing.T) {
	project, fixture := openAliasedProject(t)

	usePath := filepath.Join(fixture, "use.ts")
	aliasUse := locationOf(t, usePath, "localCount()")
	aliasUse.EndByte = aliasUse.StartByte + len("localCount")

	aliasID, err := project.SymbolAt(context.Background(), aliasUse)
	if err != nil {
		t.Fatalf("resolve imported name: %v", err)
	}
	originalID, err := project.ResolveAlias(context.Background(), aliasID)
	if err != nil {
		t.Fatalf("resolve alias: %v", err)
	}
	if originalID == aliasID {
		t.Fatal("import alias and original declaration have the same opaque identity")
	}

	declarations, err := project.Declarations(context.Background(), originalID)
	if err != nil {
		t.Fatalf("get declarations: %v", err)
	}
	if len(declarations) != 1 {
		t.Fatalf("got %d declarations, want 1", len(declarations))
	}
	declaration := declarations[0]
	wantPath, err := filepath.Abs(filepath.Join(fixture, "source.ts"))
	if err != nil {
		t.Fatal(err)
	}
	if filepath.Clean(declaration.Location.Path) != wantPath {
		t.Errorf("declaration path = %q, want %q", declaration.Location.Path, wantPath)
	}
	if declaration.Name != "count" {
		t.Errorf("declaration name = %q, want count", declaration.Name)
	}
	if declaration.Kind != "variable" {
		t.Errorf("declaration kind = %q, want variable", declaration.Kind)
	}

	wantLocation := locationOf(t, wantPath, "count =")
	if declaration.Location.StartByte != wantLocation.StartByte {
		t.Errorf("declaration start = %d, want %d", declaration.Location.StartByte, wantLocation.StartByte)
	}
}

func TestProjectFindsUsageAcrossImportedAlias(t *testing.T) {
	project, fixture := openAliasedProject(t)

	usePath := filepath.Join(fixture, "use.ts")
	aliasUse := locationOf(t, usePath, "localCount()")
	aliasUse.EndByte = aliasUse.StartByte + len("localCount")
	aliasID, err := project.SymbolAt(context.Background(), aliasUse)
	if err != nil {
		t.Fatal(err)
	}
	originalID, err := project.ResolveAlias(context.Background(), aliasID)
	if err != nil {
		t.Fatal(err)
	}

	references, err := project.References(context.Background(), originalID)
	if err != nil {
		t.Fatal(err)
	}
	if len(references) != 1 {
		t.Fatalf("got %d usages, want 1: %#v", len(references), references)
	}
	wantPath, err := filepath.Abs(usePath)
	if err != nil {
		t.Fatal(err)
	}
	if references[0].Path != wantPath || references[0].StartByte != aliasUse.StartByte || references[0].EndByte != aliasUse.EndByte {
		t.Errorf("usage = %#v, want %s:%d:%d", references[0], wantPath, aliasUse.StartByte, aliasUse.EndByte)
	}
}

func TestProjectResolvesCallTargetAndReturnType(t *testing.T) {
	project, fixture := openAliasedProject(t)

	usePath := filepath.Join(fixture, "use.ts")
	callLocation := locationOf(t, usePath, "localCount()")
	aliasLocation := callLocation
	aliasLocation.EndByte = aliasLocation.StartByte + len("localCount")
	aliasID, err := project.SymbolAt(context.Background(), aliasLocation)
	if err != nil {
		t.Fatal(err)
	}
	originalID, err := project.ResolveAlias(context.Background(), aliasID)
	if err != nil {
		t.Fatal(err)
	}

	call, err := project.ResolvedCall(context.Background(), callLocation)
	if err != nil {
		t.Fatal(err)
	}
	if call.Target != originalID {
		t.Errorf("call target = %q, want %q", call.Target, originalID)
	}
	if call.ReturnType == "" {
		t.Error("call has empty return type identity")
	}
	if call.ReturnTypeText != "number" {
		t.Errorf("return type = %q, want number", call.ReturnTypeText)
	}
}

func TestProjectTypeAtCallMatchesResolvedReturnType(t *testing.T) {
	project, fixture := openAliasedProject(t)
	callLocation := locationOf(t, filepath.Join(fixture, "use.ts"), "localCount()")

	call, err := project.ResolvedCall(context.Background(), callLocation)
	if err != nil {
		t.Fatal(err)
	}
	typeID, err := project.TypeAt(context.Background(), callLocation)
	if err != nil {
		t.Fatal(err)
	}
	if typeID != call.ReturnType {
		t.Errorf("type at call = %q, resolved return type = %q", typeID, call.ReturnType)
	}
}

func TestProjectReturnsBulkOriginalSources(t *testing.T) {
	project, _ := openAliasedProject(t)

	files, err := project.SourceFiles(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	wantNames := []string{"consumer.ts", "source.ts", "unrelated.ts", "use.ts"}
	if len(files) != len(wantNames) {
		t.Fatalf("source files = %#v, want %d files", files, len(wantNames))
	}
	for index, wantName := range wantNames {
		if filepath.Base(files[index].Path) != wantName {
			t.Errorf("source file %d path = %q, want %q", index, files[index].Path, wantName)
		}
		if len(files[index].Source) == 0 {
			t.Errorf("source file %d has no original source bytes", index)
		}
	}
}

func TestProjectUpdateRechecksChangedFileAndImporter(t *testing.T) {
	project, fixture := openAliasedProject(t)

	sourcePath := filepath.Join(fixture, "source.ts")
	affected, err := project.Update(context.Background(), []typefacts.FileChange{{
		Path:    sourcePath,
		Version: 1,
		Source:  []byte("export const count = () => \"updated\";\n"),
	}})
	if err != nil {
		t.Fatal(err)
	}
	wantSourcePath, err := filepath.Abs(sourcePath)
	if err != nil {
		t.Fatal(err)
	}
	usePath := filepath.Join(fixture, "use.ts")
	wantUsePath, err := filepath.Abs(usePath)
	if err != nil {
		t.Fatal(err)
	}
	wantConsumerPath, err := filepath.Abs(filepath.Join(fixture, "consumer.ts"))
	if err != nil {
		t.Fatal(err)
	}
	wantAffected := []string{wantConsumerPath, wantSourcePath, wantUsePath}
	if len(affected.Files) != len(wantAffected) {
		t.Fatalf("affected files = %#v, want %#v", affected.Files, wantAffected)
	}
	for i := range wantAffected {
		if affected.Files[i] != wantAffected[i] {
			t.Errorf("affected file %d = %q, want %q", i, affected.Files[i], wantAffected[i])
		}
	}

	call, err := project.ResolvedCall(context.Background(), locationOf(t, usePath, "localCount()"))
	if err != nil {
		t.Fatal(err)
	}
	if call.ReturnTypeText != "string" {
		t.Errorf("return type after update = %q, want string", call.ReturnTypeText)
	}
}

func openAliasedProject(t *testing.T) (typefacts.Project, string) {
	t.Helper()
	fixture := filepath.Join("testdata", "aliased-import")
	project, err := tsgo.OpenProject(context.Background(), filepath.Join(fixture, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = project.Close() })
	return project, fixture
}

func locationOf(t *testing.T, path, needle string) typefacts.Location {
	t.Helper()
	source, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	start := strings.Index(string(source), needle)
	if start < 0 {
		t.Fatalf("%q not found in %s", needle, path)
	}
	return typefacts.Location{Path: path, StartByte: start, EndByte: start + len(needle)}
}
