package tsgo

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// A demanded span's symbol is the node at its first byte. It describes the
// whole span only when that node is the whole span: `f(x)` and `o.p` start at
// their callee and object, so their symbols name a token inside the span.
func TestEntitySymbolCoversOnlyTheWholeDemandedSpan(t *testing.T) {
	dir := t.TempDir()
	source := `declare function makeHandler(cb: () => void): () => void;
const count = 1;
const obj = { handler() {} };
makeHandler(() => {});
count;
obj.handler;
`
	for name, contents := range map[string]string{
		"tsconfig.json": `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`,
		"calls.ts":      source,
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	// The token is located inside the anchor so each demand names exactly one
	// span of the source, whatever else the anchor's text shares with it.
	cases := []struct {
		name       string
		anchor     string
		token      string
		wantSymbol bool
		wantCover  bool
	}{
		{"call names its callee", "makeHandler(() => {})", "makeHandler(() => {})", true, false},
		{"identifier covers itself", "\ncount;", "count", true, true},
		{"binding covers itself", "const count", "count", true, true},
		{"member names its object", "\nobj.handler;", "obj.handler", true, false},
	}
	demands := make([]typefacts.EntityDemand, len(cases))
	for i, tc := range cases {
		start := strings.Index(source, tc.anchor) + strings.Index(tc.anchor, tc.token)
		demands[i] = typefacts.EntityDemand{
			Location: typefacts.Location{Path: filepath.Join(dir, "calls.ts"), StartByte: start, EndByte: start + len(tc.token)},
			Symbol:   true,
		}
	}
	entities, err := opened.(typefacts.SemanticEntityLookup).SemanticEntities(context.Background(), demands)
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != len(cases) {
		t.Fatalf("got %d entities for %d demands", len(entities), len(cases))
	}
	for i, tc := range cases {
		entity := entities[i]
		if (entity.Symbol != "") != tc.wantSymbol {
			t.Errorf("%s: symbol = %q, want present %t", tc.name, entity.Symbol, tc.wantSymbol)
		}
		if entity.SymbolCoversSpan != tc.wantCover {
			t.Errorf("%s: SymbolCoversSpan = %t, want %t", tc.name, entity.SymbolCoversSpan, tc.wantCover)
		}
	}
}
