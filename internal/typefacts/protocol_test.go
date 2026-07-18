package typefacts_test

import (
	"context"
	"errors"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

func TestBatchProtocolGenerationIsolationAndRoundLimit(t *testing.T) {
	descriptor := typefacts.GenerationDescriptor{Schema: 1, ProjectID: "project-a", Generation: 7}
	demand := typefacts.DemandProfile{Files: []string{"/workspace/App.tsx"}}
	universe := keySet(typefacts.DemandKeys(demand))
	valid := typefacts.BatchRequest{Schema: 1, ProjectID: "project-a", Generation: 7, Round: 1, Demand: demand}
	if err := typefacts.ValidateBatchRequest(descriptor, valid, universe, nil); err != nil {
		t.Fatal(err)
	}
	stale := valid
	stale.Generation--
	if err := typefacts.ValidateBatchRequest(descriptor, stale, universe, nil); !errors.Is(err, typefacts.ErrGenerationMismatch) {
		t.Fatalf("stale generation error = %v", err)
	}
	overLimit := valid
	overLimit.Round = typefacts.MaxDemandRounds + 1
	if err := typefacts.ValidateBatchRequest(descriptor, overLimit, universe, nil); !errors.Is(err, typefacts.ErrRoundLimit) {
		t.Fatalf("round-limit error = %v", err)
	}
}

func TestBatchProtocolKeysAreMonotonicAndInsideUniverse(t *testing.T) {
	first := typefacts.DemandProfile{Files: []string{"/workspace/App.tsx"}}
	second := typefacts.DemandProfile{Entities: []typefacts.EntityDemand{{
		Location: typefacts.Location{Path: "/workspace/App.tsx", StartByte: 10, EndByte: 14},
		Symbol:   true, References: true,
	}}}
	universe := keySet(append(typefacts.DemandKeys(first), typefacts.DemandKeys(second)...))
	descriptor := typefacts.GenerationDescriptor{Schema: 1, ProjectID: "p", Generation: 1}
	seen := keySet(typefacts.DemandKeys(first))
	request := typefacts.BatchRequest{Schema: 1, ProjectID: "p", Generation: 1, Round: 2, Demand: second}
	if err := typefacts.ValidateBatchRequest(descriptor, request, universe, seen); err != nil {
		t.Fatal(err)
	}
	repeated := typefacts.BatchRequest{Schema: 1, ProjectID: "p", Generation: 1, Round: 2, Demand: first}
	if err := typefacts.ValidateBatchRequest(descriptor, repeated, universe, seen); !errors.Is(err, typefacts.ErrRepeatedRequestKey) {
		t.Fatalf("repeated key error = %v", err)
	}
	outside := request
	outside.Demand.Files = []string{"/workspace/Other.tsx"}
	if err := typefacts.ValidateBatchRequest(descriptor, outside, universe, seen); !errors.Is(err, typefacts.ErrOutsideUniverse) {
		t.Fatalf("outside-universe error = %v", err)
	}
}

func TestMaterializedFactsHonorCancellationAtLookupBoundary(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	table := typefacts.FactTable{}
	if _, err := table.SymbolAt(ctx, typefacts.Location{}); !errors.Is(err, context.Canceled) {
		t.Fatalf("lookup cancellation = %v", err)
	}
}

func keySet(keys []string) map[string]struct{} {
	result := make(map[string]struct{}, len(keys))
	for _, key := range keys {
		result[key] = struct{}{}
	}
	return result
}
