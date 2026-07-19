package typefacts

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/wirecbor"
)

func richFactTable() FactTableV2 {
	descriptor := &TypeDescriptorV2{
		Text:         "Accessor<number>",
		OriginModule: "solid-js",
		AliasDeclarations: []DeclarationV2{{
			Name: "Accessor", Kind: "TypeAlias",
			Location: LocationV2{Path: "/p/solid-js.d.ts", StartByte: 10, EndByte: 30},
		}},
	}
	return FactTableV2{
		Schema:     2,
		Generation: 7,
		ProjectID:  "/p/tsconfig.json",
		Sources: []SourceDigestV2{
			{Path: "/p/a.tsx", SHA256: "aa"},
			{Path: "/p/b.tsx", SHA256: "bb"},
		},
		Entities: []EntityFactV2{
			{Location: LocationV2{Path: "/p/a.tsx", StartByte: 1, EndByte: 4}, Symbol: "symbol:h:1"},
			{Location: LocationV2{Path: "/p/a.tsx", StartByte: 5, EndByte: 9}, TypeDescriptor: descriptor},
			{
				Location:     LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
				Symbol:       "symbol:h:2",
				ResolvedCall: &CallV2{Target: "symbol:h:1", ReturnTypeText: "() => number"},
			},
			{Location: LocationV2{Path: "/p/b.tsx", StartByte: 9, EndByte: 12}},
		},
		Symbols: []SymbolFactV2{
			{ID: "symbol:h:1", Declarations: []DeclarationV2{{
				Name: "count", Kind: "Variable",
				Location: LocationV2{Path: "/p/a.tsx", StartByte: 1, EndByte: 4},
			}}, References: []LocationV2{
				{Path: "/p/a.tsx", StartByte: 1, EndByte: 4},
				{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
			}},
			{ID: "symbol:h:3", AliasTarget: "symbol:h:1"},
		},
		Files: []FileFactV2{
			{Path: "/p/a.tsx"},
			{
				Path: "/p/b.tsx",
				Calls: []SourceCallV2{{
					Location:  LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
					Callee:    LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 7},
					Arguments: []LocationV2{{Path: "/p/b.tsx", StartByte: 7, EndByte: 8}},
					Target:    "symbol:h:1",
				}},
				Bindings: []SourceBindingV2{{
					Array: true,
					Names: []LocationV2{{Path: "/p/b.tsx", StartByte: 0, EndByte: 1}},
					Initializer: SourceCallV2{
						Location: LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
						Callee:   LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 7},
					},
				}},
				Functions: []SourceFunctionV2{{
					Name:       LocationV2{Path: "/p/b.tsx", StartByte: 20, EndByte: 25},
					Body:       LocationV2{Path: "/p/b.tsx", StartByte: 26, EndByte: 40},
					Parameters: []LocationV2{{Path: "/p/b.tsx", StartByte: 21, EndByte: 22}},
					Exported:   true,
					Arrow:      true,
				}},
				AsyncFunctions: []AsyncFunctionFactV2{{
					Expression:      LocationV2{Path: "/p/b.tsx", StartByte: 26, EndByte: 40},
					Symbol:          "symbol:h:2",
					Target:          "symbol:h:1",
					CanReturnAsync:  true,
					CallsAfterAwait: []LocationV2{{Path: "/p/b.tsx", StartByte: 30, EndByte: 34}},
				}},
			},
		},
	}
}

func assertTableWireEqual(t *testing.T, expected, actual FactTableV2) {
	t.Helper()
	expectedBytes, err := wirecbor.Marshal(expected)
	if err != nil {
		t.Fatal(err)
	}
	actualBytes, err := wirecbor.Marshal(actual)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(expectedBytes, actualBytes) {
		t.Fatalf("expanded table differs from original on the wire (%d vs %d bytes)", len(actualBytes), len(expectedBytes))
	}
}

func TestCompactFactTableRoundTripsRichTable(t *testing.T) {
	table := richFactTable()
	compact := CompactFactTableV3From(table)
	encoded, err := wirecbor.Marshal(compact)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CompactFactTableV3
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	expanded, err := decoded.Expand()
	if err != nil {
		t.Fatal(err)
	}
	assertTableWireEqual(t, table, expanded)
}

func TestCompactFactTableRoundTripsEmptyTable(t *testing.T) {
	table := FactTableV2{
		Schema: 2, Generation: 1, ProjectID: "/p/tsconfig.json",
		Sources: []SourceDigestV2{}, Entities: []EntityFactV2{},
		Symbols: []SymbolFactV2{}, Files: []FileFactV2{},
	}
	compact := CompactFactTableV3From(table)
	encoded, err := wirecbor.Marshal(compact)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CompactFactTableV3
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	expanded, err := decoded.Expand()
	if err != nil {
		t.Fatal(err)
	}
	assertTableWireEqual(t, table, expanded)
}

func TestCompactFactTableRoundTripsFrozenGoldenTable(t *testing.T) {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	golden, err := os.ReadFile(filepath.Join(filepath.Dir(filename), "..", "..", "benchmarks", "phase1", "typefacts-v2-golden.cbor"))
	if err != nil {
		t.Fatal(err)
	}
	var response ClosureResponse
	if err := wirecbor.Unmarshal(golden, &response); err != nil {
		t.Fatal(err)
	}
	compact := CompactFactTableV3From(response.Table)
	encoded, err := wirecbor.Marshal(compact)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CompactFactTableV3
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	expanded, err := decoded.Expand()
	if err != nil {
		t.Fatal(err)
	}
	assertTableWireEqual(t, response.Table, expanded)
	if len(encoded) >= len(golden) {
		t.Fatalf("compact golden table (%d bytes) is not smaller than plain (%d bytes)", len(encoded), len(golden))
	}
}

func TestCompactDemandsRoundTrip(t *testing.T) {
	demands := []EntityDemand{
		{
			Location: Location{Path: "/p/a.tsx", StartByte: 1, EndByte: 4},
			Symbol:   true, References: true,
		},
		{
			Location:      Location{Path: "/p/a.tsx", StartByte: 5, EndByte: 9},
			QueryLocation: &Location{Path: "/p/a.tsx", StartByte: 6, EndByte: 8},
			Symbol:        true, TypeDescriptor: true, ResolvedCall: true,
		},
		{
			Location: Location{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
			Async:    true, StructuralAccessor: true,
		},
		{
			Location:      Location{Path: "/p/b.tsx", StartByte: 9, EndByte: 12},
			QueryLocation: &Location{Path: "/p/other.tsx", StartByte: 0, EndByte: 2},
			Type:          true, ResolveAlias: true, Declarations: true,
		},
	}
	compact := CompactDemandsV3From(demands)
	encoded, err := wirecbor.Marshal(compact)
	if err != nil {
		t.Fatal(err)
	}
	var decoded CompactDemandsV3
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	expanded, err := decoded.Expand()
	if err != nil {
		t.Fatal(err)
	}
	expectedBytes, err := wirecbor.Marshal(demands)
	if err != nil {
		t.Fatal(err)
	}
	actualBytes, err := wirecbor.Marshal(expanded)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(expectedBytes, actualBytes) {
		t.Fatal("expanded demands differ from original on the wire")
	}
}

func TestCompactExpansionFailsClosedOnStringGaps(t *testing.T) {
	table := CompactFactTableV3{
		Strings: []string{""},
		Sources: []CompactSourceDigestV3{{Path: 3, SHA256: "aa"}},
	}
	if _, err := table.Expand(); err == nil {
		t.Fatal("expected out-of-range string index to fail expansion")
	}
	demands := CompactDemandsV3{
		Strings: []string{""},
		Groups:  []CompactDemandGroupV3{{Path: 9}},
	}
	if _, err := demands.Expand(); err == nil {
		t.Fatal("expected out-of-range demand path index to fail expansion")
	}
}
