package typefacts_test

import (
	"bytes"
	"context"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

// TestPackedInternalMatchesV2Route proves the direct internal encoder and the
// v2-mediated one produce the same frame, byte for byte, on a real corpus
// table — descriptors, aliases, async control flow, reference lists and all.
// The full-mode response uses the internal route; the frozen frame layout is
// whatever the v2 route says it is, so any divergence is a bug in the
// internal one.
func TestPackedInternalMatchesV2Route(t *testing.T) {
	if testing.Short() {
		t.Skip("scale coverage is skipped under -short; the default run includes it")
	}
	ctx := context.Background()
	root := generateCorpus(t)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	demands := realisticDemands(t, backend.(demandSource), ctx)
	table, err := closure.DemandTableForGroups(ctx, 1, groupedDemands(demands), demandPaths(demands))
	if err != nil {
		t.Fatal(err)
	}

	const generation = 1
	viaV2, err := typefacts.PackedFactTableV3From(typefacts.FactTableV2From(*table, projectID, generation))
	if err != nil {
		t.Fatal(err)
	}
	direct := typefacts.PackedFactTableV3FromInternal(*table, generation)
	if !bytes.Equal(viaV2, direct) {
		limit := min(len(viaV2), len(direct))
		divergence := limit
		for index := range limit {
			if viaV2[index] != direct[index] {
				divergence = index
				break
			}
		}
		t.Fatalf("internal route diverges from the v2 route: %d vs %d bytes, first difference at offset %d",
			len(viaV2), len(direct), divergence)
	}
}
