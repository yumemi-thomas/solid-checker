package typefacts

import (
	"testing"
	"unicode/utf8"
)

func TestWireSymbolNameEscapesTypeScriptInternalNames(t *testing.T) {
	t.Parallel()

	got := declarationV2(Declaration{Name: "\xfeindex"})
	if got.Name != "__index" {
		t.Fatalf("declaration name = %q, want %q", got.Name, "__index")
	}
	if !utf8.ValidString(got.Name) {
		t.Fatalf("wire declaration name is not valid UTF-8: %q", got.Name)
	}
}

func TestWireSymbolNameRepairsUnexpectedInvalidUTF8(t *testing.T) {
	t.Parallel()

	got := wireSymbolName("before\xffafter")
	if got != "before\uFFFDafter" {
		t.Fatalf("wire symbol name = %q, want %q", got, "before\uFFFDafter")
	}
}
