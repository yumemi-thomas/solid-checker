package typefacts

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

func goldenWireTable() FactTable {
	path := "/p/a.tsx"
	descriptor := &TypeDescriptor{
		Text:         "Accessor<number>",
		OriginModule: "solid-js",
		AliasDeclarations: []Declaration{{
			Name: "Accessor", Kind: "TypeAlias",
			Location: Location{Path: "/p/solid-js.d.ts", StartByte: 10, EndByte: 30},
		}},
	}
	call := &Call{
		Target: "symbol:h:1", ReturnTypeText: "() => number",
		Validity: ResolvedCallValid, Kind: CallKindCall,
		Declaration: &ResolvedDeclaration{
			Symbol: "symbol:h:1", Name: "count", Kind: "FunctionDeclaration",
			Location: Location{Path: path, StartByte: 1, EndByte: 4},
			Owners: []DeclarationOwner{{
				Symbol: "symbol:h:4", Name: "Counter", Kind: "InterfaceDeclaration",
				Location: Location{Path: path, StartByte: 0, EndByte: 1},
			}},
			QualifiedName: "Counter.count", SourceFile: path,
		},
		Arguments: []ArgumentMapping{{
			ArgumentIndex: 0, Status: ArgumentMappingResolved,
			Parameter: &ParameterFact{
				Index: 0, Symbol: "symbol:h:5", Callability: CallabilityCallable,
				Declaration: &Declaration{
					Name: "callback", Kind: "declaration",
					Location: Location{Path: path, StartByte: 5, EndByte: 13},
				},
				TypeDescriptor: descriptor,
			},
		}},
	}
	return FactTable{
		Schema: TypeFactsSchemaVersion, Generation: 3, ProjectID: "/p/tsconfig.json",
		Sources: []SourceFile{{Path: path, Source: []byte("export const value = 1\n")}},
		Entities: []EntityFact{{
			Location: Location{Path: path, StartByte: 2, EndByte: 8},
			Symbol:   "symbol:h:2", TypeDescriptor: descriptor, ResolvedCall: call,
			Callability: CallabilityCallable, ReferenceSpace: ReferenceSpaceBoth,
			RuntimeIdentity: "runtime:h:1",
		}},
		Symbols: []SymbolFact{
			{ID: "symbol:h:1", Declarations: []Declaration{{
				Name: "count", Kind: "Variable",
				Location: Location{Path: path, StartByte: 1, EndByte: 4},
			}}, References: []Location{{Path: path, StartByte: 2, EndByte: 8}}},
			{ID: "symbol:h:3", AliasTarget: "symbol:h:1"},
		},
		Files: []FileFact{{
			Path: path,
			Calls: []SourceCall{{
				Location:  Location{Path: path, StartByte: 2, EndByte: 8},
				Callee:    Location{Path: path, StartByte: 2, EndByte: 7},
				Arguments: []Location{{Path: path, StartByte: 7, EndByte: 8}},
				Target:    "symbol:h:1",
			}},
			Bindings: []SourceBinding{{
				Array: true, Names: []Location{{Path: path, StartByte: 0, EndByte: 1}},
				Initializer: SourceCall{
					Location: Location{Path: path, StartByte: 2, EndByte: 8},
					Callee:   Location{Path: path, StartByte: 2, EndByte: 7},
				},
			}},
			Functions: []SourceFunction{{
				Name:       Location{Path: path, StartByte: 20, EndByte: 25},
				Body:       Location{Path: path, StartByte: 26, EndByte: 40},
				Parameters: []Location{{Path: path, StartByte: 21, EndByte: 22}},
				Exported:   true, Arrow: true,
			}},
			AsyncFunctions: []AsyncFunctionFact{{
				Expression: Location{Path: path, StartByte: 26, EndByte: 40},
				Symbol:     "symbol:h:2", Target: "symbol:h:1", CanReturnAsync: true,
				CallsAfterAwait: []Location{{Path: path, StartByte: 30, EndByte: 34}},
			}},
		}},
	}
}

// v3GoldenFixtures builds the request and response the frozen fixtures under
// benchmarks/phase1 pin. The pair is deliberately dense: an analyze carrying a
// compact demand snapshot, and a full response carrying a packed fact table,
// so both compression schemes and the deterministic-CBOR envelope are covered
// by one cross-language round trip.
func v3GoldenFixtures(t *testing.T) (LifecycleRequest, LifecycleResponse) {
	t.Helper()
	request := LifecycleRequest{
		Schema:     TypeFactsSchemaVersionV1,
		RequestID:  7,
		Operation:  LifecycleAnalyze,
		ProjectID:  "/p/tsconfig.json",
		Generation: 3,
		Changes: []FileChangeV3{
			{Path: "/p/a.tsx", Version: 2, Source: []byte("export const value = 1\n")},
			{Path: "/p/gone.tsx", Version: 3, Deleted: true},
		},
		CompactDemands: &CompactDemandsV3{
			Groups: []CompactDemandGroupV3{
				{Path: 1, Demands: []byte{0x03, 0x01, 0x03}},
				{Path: 2, Demands: []byte{0x0d, 0x02, 0x06}},
			},
			Strings: []string{"", "/p/a.tsx", "/p/b.tsx"},
		},
		StateToken:         "4",
		RemovedDemandPaths: []string{"/p/dropped.tsx"},
	}

	table := goldenWireTable()
	transition, err := (&wireTransitionEncoder{}).Encode(wireTransitionInput{
		ProjectID: "/p/tsconfig.json",
		Target:    &table,
	})
	if err != nil {
		t.Fatal(err)
	}
	response := LifecycleResponse{
		Schema:          TypeFactsSchemaVersionV1,
		RequestID:       7,
		ProjectID:       "/p/tsconfig.json",
		Generation:      3,
		OK:              true,
		TableTransition: transition.Bytes,
		StateToken:      "5",
		Affected:        []string{"/p/a.tsx"},
		Timings:         &LifecycleTimings{AnalyzeNs: 1234, Materialized: true, RetainedFiles: 2},
	}
	return request, response
}

func readV3Golden(t *testing.T, name string) []byte {
	t.Helper()
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	golden, err := os.ReadFile(filepath.Join(filepath.Dir(filename), "..", "..", "..", "..", "benchmarks", "typefacts", "phase1", name))
	if err != nil {
		t.Fatal(err)
	}
	return golden
}

func v3GoldenPath(t *testing.T, name string) string {
	t.Helper()
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	return filepath.Join(filepath.Dir(filename), "..", "..", "..", "..", "benchmarks", "typefacts", "phase1", name)
}

// The Rust client decodes these same two files in
// crates/typefacts/tests/typefacts_v3_codec_golden.rs, so a drift in either
// language's field names, tags, or canonical ordering fails one of the pair.
func TestV3RequestGoldenRoundTripsIdentically(t *testing.T) {
	if os.Getenv("TYPEFACTS_UPDATE_GOLDEN") != "" {
		request, _ := v3GoldenFixtures(t)
		encoded, err := wirecbor.Marshal(request)
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(v3GoldenPath(t, "typefacts-v3-request-golden.cbor"), encoded, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	golden := readV3Golden(t, "typefacts-v3-request-golden.cbor")
	var request LifecycleRequest
	if err := wirecbor.Unmarshal(golden, &request); err != nil {
		t.Fatal(err)
	}
	if err := ValidateLifecycleRequest(request); err != nil {
		t.Fatalf("golden request invalid: %v", err)
	}
	expected, _ := v3GoldenFixtures(t)
	if request.RequestID != expected.RequestID || request.Operation != expected.Operation ||
		request.StateToken != expected.StateToken || request.CompactDemands == nil {
		t.Fatalf("golden request = %+v", request)
	}
	encoded, err := wirecbor.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("golden re-encoding changed: %x != %x", encoded, golden)
	}
}

func TestV3ResponseGoldenRoundTripsIdentically(t *testing.T) {
	if os.Getenv("TYPEFACTS_UPDATE_GOLDEN") != "" {
		_, response := v3GoldenFixtures(t)
		encoded, err := wirecbor.Marshal(response)
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(v3GoldenPath(t, "typefacts-v3-response-golden.cbor"), encoded, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	golden := readV3Golden(t, "typefacts-v3-response-golden.cbor")
	var response LifecycleResponse
	if err := wirecbor.Unmarshal(golden, &response); err != nil {
		t.Fatal(err)
	}
	if !response.OK || len(response.TableTransition) == 0 {
		t.Fatalf("golden response = %+v", response)
	}
	encoded, err := wirecbor.Marshal(response)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("golden re-encoding changed: %x != %x", encoded, golden)
	}
}

// moduleGraphGoldenFixtures builds the modules request and response the
// fixtures under benchmarks/phase1 pin. Every optional field of every module
// row is populated at least once across the two, so a drift in a field name,
// an enum spelling, or an omission rule fails the cross-language pair rather
// than surviving until a consumer reads a silently absent fact.
func moduleGraphGoldenFixtures() (LifecycleRequest, LifecycleResponse) {
	request := LifecycleRequest{
		Schema:     TypeFactsSchemaVersionV1,
		RequestID:  11,
		Operation:  LifecycleModules,
		ProjectID:  "/p/tsconfig.json",
		Generation: 3,
		ModuleGraph: &ModuleInventoryDemand{
			Imports:     true,
			ImportPaths: []string{"/p/src/index.ts"},
			Packages:    true,
		},
	}
	response := LifecycleResponse{
		Schema:     TypeFactsSchemaVersionV1,
		RequestID:  11,
		ProjectID:  "/p/tsconfig.json",
		Generation: 3,
		OK:         true,
		Modules: []ModuleFact{
			{Path: "/p/lib/src/channel.ts", Format: ModuleFormatESM, ProjectReference: &ProjectReferenceMapping{
				Source: "/p/lib/src/channel.ts", OutputDts: "/p/lib/dist/channel.d.ts",
			}},
			{Path: "/p/node_modules/.store/reactive@4.2.0/node_modules/reactive/index.d.ts",
				DeclarationFile: true, Format: ModuleFormatCommonJS,
				RedirectTargets: []string{"/p/vendor/reactive/index.d.ts"}},
			{Path: "/p/src/index.ts", Format: ModuleFormatESM},
			{Path: "/p/src/local-impl.ts", Format: ModuleFormatPreserve},
		},
		ModuleImports: []ModuleImportFact{
			{
				Specifier:    Location{Path: "/p/src/index.ts", StartByte: 30, EndByte: 48},
				Text:         "reactive-package",
				Resolution:   ModuleResolutionNonRelative,
				ResolvedPath: "/p/src/local-impl.ts",
				Extension:    ".ts",
				PathsPattern: "reactive-package",
			},
			{
				Specifier:    Location{Path: "/p/src/index.ts", StartByte: 70, EndByte: 80},
				Text:         "reactive",
				Resolution:   ModuleResolutionNodeModules,
				ResolvedPath: "/p/node_modules/.store/reactive@4.2.0/node_modules/reactive/index.d.ts",
				SymlinkPath:  "/p/node_modules/reactive/index.d.ts",
				Extension:    ".d.ts",
				Package: &PackageIdentity{
					ManifestPath: "/p/node_modules/.store/reactive@4.2.0/node_modules/reactive/package.json",
					Name:         "reactive",
					Version:      "4.2.0",
				},
				ResolverPackage: &ResolverPackageID{Name: "reactive", Version: "4.2.0"},
			},
			{
				Specifier:    Location{Path: "/p/src/index.ts", StartByte: 100, EndByte: 126},
				Text:         "../lib/dist/channel.js",
				Resolution:   ModuleResolutionRelative,
				ResolvedPath: "/p/lib/dist/channel.d.ts",
				IncludedPath: "/p/lib/src/channel.ts",
				Extension:    ".d.ts",
				TSExtension:  true,
			},
			{
				Specifier:  Location{Path: "/p/src/index.ts", StartByte: 150, EndByte: 166},
				Text:       "never-installed",
				Resolution: ModuleResolutionUnresolved,
			},
		},
		UnknownImportPaths: []string{"/p/src/absent.ts"},
	}
	return request, response
}

func TestModuleGraphGoldensRoundTripIdentically(t *testing.T) {
	request, response := moduleGraphGoldenFixtures()
	for _, fixture := range []struct {
		name  string
		value any
		into  func() any
	}{
		{"typefacts-module-graph-request-golden.cbor", request, func() any { return new(LifecycleRequest) }},
		{"typefacts-module-graph-response-golden.cbor", response, func() any { return new(LifecycleResponse) }},
	} {
		encoded, err := wirecbor.Marshal(fixture.value)
		if err != nil {
			t.Fatal(err)
		}
		if os.Getenv("TYPEFACTS_UPDATE_GOLDEN") != "" {
			if err := os.WriteFile(v3GoldenPath(t, fixture.name), encoded, 0o644); err != nil {
				t.Fatal(err)
			}
		}
		golden := readV3Golden(t, fixture.name)
		if !bytes.Equal(encoded, golden) {
			t.Fatalf("%s re-encoding changed:\n%x\n%x", fixture.name, encoded, golden)
		}
		decoded := fixture.into()
		if err := wirecbor.Unmarshal(golden, decoded); err != nil {
			t.Fatalf("%s: %v", fixture.name, err)
		}
		reencoded, err := wirecbor.Marshal(decoded)
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(reencoded, golden) {
			t.Fatalf("%s did not survive a decode/encode round trip", fixture.name)
		}
	}
	if err := ValidateLifecycleRequest(request); err != nil {
		t.Fatalf("golden modules request invalid: %v", err)
	}
}
