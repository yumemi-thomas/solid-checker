package wirecbor_test

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/internal/wirecbor"
)

func readGolden(t *testing.T, name string) []byte {
	t.Helper()
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	golden, err := os.ReadFile(filepath.Join(filepath.Dir(filename), "..", "..", "benchmarks", "phase1", name))
	if err != nil {
		t.Fatal(err)
	}
	return golden
}

func TestTypeFactsV2RequestGoldenRoundTripsIdentically(t *testing.T) {
	golden := readGolden(t, "typefacts-v2-request-golden.cbor")
	var request typefacts.ClosureRequest
	if err := wirecbor.Unmarshal(golden, &request); err != nil {
		t.Fatal(err)
	}
	if err := typefacts.ValidateClosureRequest(request); err != nil {
		t.Fatalf("golden request invalid: %v", err)
	}
	encoded, err := wirecbor.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("golden re-encoding changed: %x != %x", encoded, golden)
	}
}

func TestTypeFactsV2ResponseGoldenRoundTripsIdentically(t *testing.T) {
	requestGolden := readGolden(t, "typefacts-v2-request-golden.cbor")
	var request typefacts.ClosureRequest
	if err := wirecbor.Unmarshal(requestGolden, &request); err != nil {
		t.Fatal(err)
	}
	golden := readGolden(t, "typefacts-v2-golden.cbor")
	var response typefacts.ClosureResponse
	if err := wirecbor.Unmarshal(golden, &response); err != nil {
		t.Fatal(err)
	}
	if err := typefacts.ValidateClosureResponse(request, response); err != nil {
		t.Fatalf("golden response invalid: %v", err)
	}
	encoded, err := wirecbor.Marshal(response)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("golden re-encoding changed: %x != %x", encoded, golden)
	}
}

func TestValidateClosureResponseRejectsAliasReferences(t *testing.T) {
	golden := readGolden(t, "typefacts-v2-golden.cbor")
	var response typefacts.ClosureResponse
	if err := wirecbor.Unmarshal(golden, &response); err != nil {
		t.Fatal(err)
	}
	request := typefacts.ClosureRequest{
		Schema: typefacts.TypeFactsSchemaVersionV2, ProjectID: response.ProjectID,
		Generation: response.Generation, RulesetVersion: typefacts.ExpansionRulesetVersionV1,
	}
	for index := range response.Table.Symbols {
		if response.Table.Symbols[index].AliasTarget != "" {
			response.Table.Symbols[index].References = []typefacts.LocationV2{{Path: "/x.ts", StartByte: 0, EndByte: 1}}
		}
	}
	if err := typefacts.ValidateClosureResponse(request, response); err == nil {
		t.Fatal("alias symbol with references must be rejected")
	}
}
