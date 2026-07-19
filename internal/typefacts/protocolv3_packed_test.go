package typefacts

import (
	"bytes"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/wirecbor"
)

func TestPackedFactTableIsDeterministicAndSmallerThanCompact(t *testing.T) {
	table := richFactTable()
	for index := range table.Sources {
		table.Sources[index].SHA256 = "sha256:" + strings.Repeat("0", 64)
	}
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
	compact, err := wirecbor.Marshal(CompactFactTableV3From(table))
	if err != nil {
		t.Fatal(err)
	}
	if len(first) >= len(compact) {
		t.Fatalf("packed table is %d bytes, compact table is %d", len(first), len(compact))
	}
}

func TestPackedFactTableRejectsNonCanonicalDigest(t *testing.T) {
	table := richFactTable()
	if _, err := PackedFactTableV3From(table); err == nil {
		t.Fatal("expected non-canonical source digest to fail")
	}
}
