package wirecbor_test

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/internal/wirecbor"
)

func TestMarshalIsCoreDeterministic(t *testing.T) {
	first, err := wirecbor.Marshal(map[string]uint64{"longer": 2, "a": 1})
	if err != nil {
		t.Fatal(err)
	}
	second, err := wirecbor.Marshal(map[string]uint64{"a": 1, "longer": 2})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatalf("canonical encodings differ: %x != %x", first, second)
	}
}

func TestTypeFactsV1GoldenRoundTripsIdentically(t *testing.T) {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	golden, err := os.ReadFile(filepath.Join(filepath.Dir(filename), "..", "..", "benchmarks", "phase1", "typefacts-v1-golden.cbor"))
	if err != nil {
		t.Fatal(err)
	}
	var response typefacts.BatchResponse
	if err := wirecbor.Unmarshal(golden, &response); err != nil {
		t.Fatal(err)
	}
	if response.Schema != 1 || response.Table.Schema != 1 || response.Generation != response.Table.Generation {
		t.Fatalf("golden identity = %#v", response)
	}
	encoded, err := wirecbor.Marshal(response)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, golden) {
		t.Fatalf("golden re-encoding changed: %x != %x", encoded, golden)
	}
}

func TestLimitsMatchLanguageNeutralSchema(t *testing.T) {
	_, filename, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve test path")
	}
	contents, err := os.ReadFile(filepath.Join(filepath.Dir(filename), "..", "..", "schema", "typefacts-codec-limits.json"))
	if err != nil {
		t.Fatal(err)
	}
	var limits struct {
		MaxMessageBytes     int `json:"maxMessageBytes"`
		MaxGenerationBytes  int `json:"maxGenerationBytes"`
		MaxNestedLevels     int `json:"maxNestedLevels"`
		MaxCollectionLength int `json:"maxCollectionLength"`
	}
	if err := json.Unmarshal(contents, &limits); err != nil {
		t.Fatal(err)
	}
	if limits.MaxMessageBytes != wirecbor.MaxMessageBytes || limits.MaxGenerationBytes != wirecbor.MaxGenerationBytes || limits.MaxNestedLevels != wirecbor.MaxNestedLevels || limits.MaxCollectionLength != wirecbor.MaxCollectionLength {
		t.Fatalf("schema limits = %#v", limits)
	}
}

func TestUnmarshalRejectsUnknownAndDuplicateFields(t *testing.T) {
	type message struct {
		Value uint64 `cbor:"value"`
	}
	unknown, err := wirecbor.Marshal(map[string]uint64{"future": 1})
	if err != nil {
		t.Fatal(err)
	}
	if err := wirecbor.Unmarshal(unknown, &message{}); err == nil {
		t.Fatal("unknown field accepted")
	}
	duplicate := []byte{0xa2, 0x65, 'v', 'a', 'l', 'u', 'e', 0x01, 0x65, 'v', 'a', 'l', 'u', 'e', 0x02}
	if err := wirecbor.Unmarshal(duplicate, &message{}); err == nil {
		t.Fatal("duplicate field accepted")
	}
}
