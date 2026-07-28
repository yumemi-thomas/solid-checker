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
		transaction.groups()[0].path != "a.ts" ||
		transaction.groups()[1].path != "b.ts" {
		t.Fatalf("candidate groups = %#v", transaction.groups())
	}
	transaction.rollback()

	if len(store.groups) != 2 ||
		store.groups[0].path != "a.ts" ||
		store.groups[0].demands[0].Location.StartByte != 1 ||
		store.groups[1].path != "c.ts" {
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
		store.groups[0].path != "a.ts" ||
		store.groups[1].path != "b.ts" ||
		len(store.groups[0].demands) != 2 ||
		store.groups[0].demands[0].Location.StartByte != 1 ||
		store.groups[0].demands[1].Location.StartByte != 2 {
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
		store.groups[0].path != clean ||
		store.groups[0].demands[0].Location.Path != clean ||
		store.groups[0].demands[0].QueryLocation == nil ||
		store.groups[0].demands[0].QueryLocation.Path != clean {
		t.Fatalf("canonical groups = %#v", store.groups)
	}
	if request[0].Location.Path != dirty || request[0].QueryLocation.Path != dirty {
		t.Fatalf("request was mutated: %#v", request)
	}
}

func TestRetainedDemandStoreOwnsCompactRunsWithoutExpandedRows(t *testing.T) {
	compact := CompactDemandsV3From([]EntityDemand{
		demandAt("a.ts", 1),
		demandAt("a.ts", 2),
		demandAt("b.ts", 3),
	})
	var store retainedDemandStore
	transaction, err := store.beginCompact(compact, nil, true)
	if err != nil {
		t.Fatal(err)
	}
	transaction.commit()

	if len(store.groups) != 2 {
		t.Fatalf("retained groups = %d, want 2", len(store.groups))
	}
	for index := range store.groups {
		group := &store.groups[index]
		if !group.isCompact() || group.demands != nil {
			t.Fatalf("group %q retained expanded demands: %#v", group.path, group.demands)
		}
	}
	if demands := store.at("a.ts"); len(demands) != 2 {
		t.Fatalf("expanded a.ts demands = %d, want 2", len(demands))
	}
}

func TestMalformedCompactDemandDoesNotMutateRetainedState(t *testing.T) {
	var store retainedDemandStore
	initial := store.begin([]EntityDemand{demandAt("a.ts", 1)}, nil, true)
	initial.commit()

	_, err := store.beginCompact(CompactDemandsV3{
		Strings: []string{"", "b.ts"},
		Groups: []CompactDemandGroupV3{{
			Path:    1,
			Demands: []byte{0x80},
		}},
	}, nil, false)
	if err == nil {
		t.Fatal("malformed compact demand was accepted")
	}
	if len(store.groups) != 1 ||
		store.groups[0].path != "a.ts" ||
		len(store.groups[0].demands) != 1 {
		t.Fatalf("retained state mutated after rejected compact demand: %#v", store.groups)
	}
}
