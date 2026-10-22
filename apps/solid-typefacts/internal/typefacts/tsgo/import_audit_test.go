package tsgo

import (
	"go/parser"
	"go/token"
	"strconv"
	"strings"
	"testing"
)

func TestTypeFactsImportsOnlyReviewedTypeScriptShims(t *testing.T) {
	allowed := map[string]bool{
		"github.com/microsoft/TypeScript/tsc/shim/ast":       true,
		"github.com/microsoft/TypeScript/tsc/shim/bundled":   true,
		"github.com/microsoft/TypeScript/tsc/shim/checker":   true,
		"github.com/microsoft/TypeScript/tsc/shim/compiler":  true,
		"github.com/microsoft/TypeScript/tsc/shim/core":      true,
		"github.com/microsoft/TypeScript/tsc/shim/scanner":   true,
		"github.com/microsoft/TypeScript/tsc/shim/tsoptions": true,
		"github.com/microsoft/TypeScript/tsc/shim/vfs":       true,
		"github.com/microsoft/TypeScript/tsc/shim/vfs/osvfs": true,
	}
	packages, err := parser.ParseDir(token.NewFileSet(), ".", nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	for _, pkg := range packages {
		for filename, file := range pkg.Files {
			for _, spec := range file.Imports {
				path, err := strconv.Unquote(spec.Path.Value)
				if err != nil {
					t.Fatal(err)
				}
				if strings.Contains(path, "microsoft/TypeScript/tsc") && !allowed[path] {
					t.Errorf("%s imports unreviewed TypeScript package %q", filename, path)
				}
				if strings.Contains(path, "tsgolint") {
					t.Errorf("%s imports tsgolint product package %q", filename, path)
				}
			}
		}
	}
	if len(allowed) != 9 {
		t.Fatalf("reviewed shim set changed unexpectedly: %d", len(allowed))
	}
}
