package typefacts

import (
	"testing"
	"unicode/utf8"
)

func TestWireSymbolNameEscapesTypeScriptInternalNames(t *testing.T) {
	t.Parallel()

	got := wireSymbolName("\xfeindex")
	if got != "__index" {
		t.Fatalf("wire symbol name = %q, want %q", got, "__index")
	}
	if !utf8.ValidString(got) {
		t.Fatalf("wire symbol name is not valid UTF-8: %q", got)
	}
}

func TestWireSymbolNameRepairsUnexpectedInvalidUTF8(t *testing.T) {
	t.Parallel()

	got := wireSymbolName("before\xffafter")
	if got != "before\uFFFDafter" {
		t.Fatalf("wire symbol name = %q, want %q", got, "before\uFFFDafter")
	}
}
