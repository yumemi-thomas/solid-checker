package typefacts_test

import (
	"os"
	"path/filepath"
	"regexp"
	"testing"
)

// The shims live in this repository, but the invariant they inherited from
// the tsgolint era stands: every shim must be validated against ONE
// typescript-go revision, because they alias and go:linkname that revision's
// internal packages. A mixed set silently pairs one package's shims with
// another package's compiler.
func TestAllShimsRequireOneReviewedTypeScriptGoRevision(t *testing.T) {
	rootModule, err := os.ReadFile("../../../../go.mod")
	if err != nil {
		t.Fatal(err)
	}
	locals := regexp.MustCompile(`github\.com/microsoft/typescript-go/shim/\S+ => \./shims/\S+`).FindAll(rootModule, -1)
	if len(locals) != 9 {
		t.Fatalf("local shim replacements = %d, want 9", len(locals))
	}

	moduleFiles, err := filepath.Glob("../../../../shims/*/go.mod")
	if err != nil {
		t.Fatal(err)
	}
	nested, err := filepath.Glob("../../../../shims/*/*/go.mod")
	if err != nil {
		t.Fatal(err)
	}
	moduleFiles = append(moduleFiles, nested...)
	if len(moduleFiles) != 9 {
		t.Fatalf("shim modules = %d, want 9", len(moduleFiles))
	}
	revision := regexp.MustCompile(`github\.com/microsoft/typescript-go (v0\.0\.0-\d+-[0-9a-f]+)`)
	want := ""
	for _, moduleFile := range moduleFiles {
		contents, err := os.ReadFile(moduleFile)
		if err != nil {
			t.Fatal(err)
		}
		match := revision.FindSubmatch(contents)
		if match == nil {
			t.Fatalf("%s pins no typescript-go revision", moduleFile)
		}
		if want == "" {
			want = string(match[1])
			continue
		}
		if string(match[1]) != want {
			t.Fatalf("mixed typescript-go revisions: %s pins %q, want %q", moduleFile, match[1], want)
		}
	}
}
