package tsgo

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// Durable IDs persist across sessions and processes, so the digest rendering
// is a frozen contract: the buffer-built form must stay byte-identical to the
// historical fmt.Sprintf form it replaced, forever.
func TestDurableIDRenderingMatchesHistoricalForm(t *testing.T) {
	refs := []durableSymbolRef{
		{path: "/p/src/mod.ts", startByte: 0, endByte: 1, name: "x"},
		{path: "/p/src/mod.ts", startByte: 1234, endByte: 987654321, name: "longSymbolName"},
		{path: "", startByte: 0, endByte: 0, name: ""},
		{path: "/p/日本語.ts", startByte: 7, endByte: 42, name: "名前"},
		{path: "/p/a.ts", startByte: 10, endByte: 20, name: "with\x00nul"},
	}
	for _, ref := range refs {
		historical := sha256.Sum256([]byte(fmt.Sprintf(
			"%s\x00%d\x00%d\x00%s", ref.path, ref.startByte, ref.endByte, ref.name)))
		want := typefacts.SymbolID("symbol:h:" + hex.EncodeToString(historical[:12]))
		if got := ref.id(); got != want {
			t.Errorf("ref %+v id = %s, want %s", ref, got, want)
		}

		historicalExported := sha256.Sum256([]byte(fmt.Sprintf(
			"export\x00%s\x00%s", ref.path, ref.name)))
		wantExported := typefacts.SymbolID("symbol:h:" + hex.EncodeToString(historicalExported[:12]))
		if got := ref.exportedID(); got != wantExported {
			t.Errorf("ref %+v exportedID = %s, want %s", ref, got, wantExported)
		}
	}
}
