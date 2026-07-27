package typefacts

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/wirecbor"
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
		Schema:     TypeFactsTableSchemaVersion,
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
				Location: LocationV2{Path: "/p/b.tsx", StartByte: 2, EndByte: 8},
				Symbol:   "symbol:h:2",
				ResolvedCall: &CallV2{
					Target: "symbol:h:1", ReturnTypeText: "() => number", Validity: ResolvedCallValid,
					Kind: CallKindCall,
					Declaration: &ResolvedDeclarationV2{
						Symbol: "symbol:h:1", Name: "count", Kind: "FunctionDeclaration",
						Location: LocationV2{Path: "/p/a.tsx", StartByte: 1, EndByte: 4},
						Owners: []DeclarationOwnerV2{{
							Symbol: "symbol:h:4", Name: "Counter", Kind: "InterfaceDeclaration",
							Location: LocationV2{Path: "/p/a.tsx", StartByte: 0, EndByte: 1},
						}},
						QualifiedName: "Counter.count", SourceFile: "/p/a.tsx",
					},
					Arguments: []ArgumentMappingV2{{
						ArgumentIndex: 0, Status: ArgumentMappingResolved,
						Parameter: &ParameterFactV2{
							Index: 0, Symbol: "symbol:h:5", Callability: CallabilityCallable,
							Declaration: &DeclarationV2{
								Name: "callback", Kind: "declaration",
								Location: LocationV2{Path: "/p/a.tsx", StartByte: 5, EndByte: 13},
							},
							TypeDescriptor: descriptor,
						},
					}},
				},
				Callability:     CallabilityCallable,
				ReferenceSpace:  ReferenceSpaceBoth,
				RuntimeIdentity: "runtime:h:1",
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
func goldenPackedTable(t *testing.T) FactTableV2 {
	t.Helper()
	table := richFactTable()
	for index := range table.Sources {
		table.Sources[index].SHA256 = "sha256:" + strings.Repeat("0", 64)
	}
	return table
}

// The packed frame is the only shape a full analyze response travels in, and
// the Rust decoder in crates/typefacts/src/v3.rs reads it positionally. Pin
// the exact bytes so a reordering of the writer — in particular the dictionary
// interning order — cannot silently change the frame.
func TestPackedFactTableLayoutIsFrozen(t *testing.T) {
	const goldenLength = 569
	const goldenDigest = "620fef0a04cc6e3dd7be86f4caab545690d3c4e8e724edc5693713354b04d4d1"

	packed, err := PackedFactTableV3From(goldenPackedTable(t))
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(packed)
	if len(packed) != goldenLength || hex.EncodeToString(digest[:]) != goldenDigest {
		t.Fatalf("packed frame changed: %d bytes, sha256 %s (want %d bytes, sha256 %s)",
			len(packed), hex.EncodeToString(digest[:]), goldenLength, goldenDigest)
	}
}

func TestPackedFactTableIsDeterministicAndSmallerThanPlainWire(t *testing.T) {
	table := goldenPackedTable(t)
	first, err := PackedFactTableV3From(table)
	if err != nil {
		t.Fatal(err)
	}
	second, err := PackedFactTableV3From(table)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatal("packed table encoding is not deterministic")
	}
	plain, err := wirecbor.Marshal(table)
	if err != nil {
		t.Fatal(err)
	}
	if len(first) >= len(plain) {
		t.Fatalf("packed table is %d bytes, plain wire table is %d", len(first), len(plain))
	}
}

func TestPackedFactTableRejectsNonCanonicalDigest(t *testing.T) {
	table := richFactTable()
	if _, err := PackedFactTableV3From(table); err == nil {
		t.Fatal("expected non-canonical source digest to fail")
	}
}

func TestPackedEntityGroupsRunsOfSharedPaths(t *testing.T) {
	entities := []EntityFactV2{
		{Location: LocationV2{Path: "/p/a.ts"}},
		{Location: LocationV2{Path: "/p/a.ts"}},
		{Location: LocationV2{Path: "/p/b.ts"}},
		{Location: LocationV2{Path: "/p/a.ts"}},
	}
	groups := packedEntityGroups(entities)
	if len(groups) != 3 || groups[0] != 2 || groups[1] != 1 || groups[2] != 1 {
		t.Fatalf("entity groups = %v, want [2 1 1]", groups)
	}
	if packedEntityGroups(nil) != nil {
		t.Fatal("expected no groups for an empty entity list")
	}
}
