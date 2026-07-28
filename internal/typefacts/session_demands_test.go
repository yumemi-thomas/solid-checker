package typefacts

import (
	"path/filepath"
	"testing"
)

func demandAt(path string, start int) EntityDemand {
	return EntityDemand{
		Location: Location{Path: path, StartByte: start, EndByte: start + 1},
		Symbol:   true,
	}
}

func TestRetainedDemandTransactionRollsBackReplaceInsertAndRemove(t *testing.T) {
	var store retainedDemandStore
	initial := store.begin([]EntityDemand{
		demandAt("a.ts", 1),
		demandAt("c.ts", 1),
	}, nil, true)
	initial.commit()

	transaction := store.begin(
		[]EntityDemand{
			demandAt("a.ts", 2),
			demandAt("b.ts", 1),
		},
		[]string{"c.ts"},
		false,
	)
	if len(transaction.groups()) != 2 ||
		transaction.groups()[0].Path != "a.ts" ||
		transaction.groups()[1].Path != "b.ts" {
		t.Fatalf("candidate groups = %#v", transaction.groups())
	}
	transaction.rollback()

	if len(store.groups) != 2 ||
		store.groups[0].Path != "a.ts" ||
		store.groups[0].Demands[0].Location.StartByte != 1 ||
		store.groups[1].Path != "c.ts" {
		t.Fatalf("rolled-back groups = %#v", store.groups)
	}
}

func TestRetainedDemandTransactionCanonicalizesOnlyUnorderedChanges(t *testing.T) {
	var store retainedDemandStore
	transaction := store.begin([]EntityDemand{
		demandAt("b.ts", 1),
		demandAt("a.ts", 2),
		demandAt("a.ts", 1),
	}, nil, true)
	transaction.commit()

	if len(store.groups) != 2 ||
		store.groups[0].Path != "a.ts" ||
		store.groups[1].Path != "b.ts" ||
		len(store.groups[0].Demands) != 2 ||
		store.groups[0].Demands[0].Location.StartByte != 1 ||
		store.groups[0].Demands[1].Location.StartByte != 2 {
		t.Fatalf("canonical groups = %#v", store.groups)
	}
}

func TestRetainedDemandTransactionCleansPathsWithoutMutatingRequest(t *testing.T) {
	clean := filepath.Join("project", "source.ts")
	separator := string(filepath.Separator)
	dirty := "project" + separator + "." + separator + "nested" + separator + ".." + separator + "source.ts"
	query := Location{Path: dirty, StartByte: 2, EndByte: 3}
	request := []EntityDemand{{
		Location:      Location{Path: dirty, StartByte: 1, EndByte: 2},
		QueryLocation: &query,
		Symbol:        true,
	}}

	var store retainedDemandStore
	transaction := store.begin(request, nil, true)
	transaction.commit()

	if len(store.groups) != 1 ||
		store.groups[0].Path != clean ||
		store.groups[0].Demands[0].Location.Path != clean ||
		store.groups[0].Demands[0].QueryLocation == nil ||
		store.groups[0].Demands[0].QueryLocation.Path != clean {
		t.Fatalf("canonical groups = %#v", store.groups)
	}
	if request[0].Location.Path != dirty || request[0].QueryLocation.Path != dirty {
		t.Fatalf("request was mutated: %#v", request)
	}
}
