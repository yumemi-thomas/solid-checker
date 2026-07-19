package typefacts

import (
	"context"
	"reflect"
	"testing"
)

type canonicalPatchBackend struct {
	transportOnlyBackend
	changed    []SymbolID
	references map[SymbolID][]Location
}

func (b canonicalPatchBackend) ChangedReferences(context.Context) ([]SymbolID, bool, error) {
	return b.changed, true, nil
}

func (b canonicalPatchBackend) ReferencesBatch(context.Context, []SymbolID) (map[SymbolID][]Location, error) {
	return b.references, nil
}

func TestOrderSymbolFactsReusesCanonicalOrderAcrossDelta(t *testing.T) {
	facts := []SymbolFact{
		{ID: "d", AliasTarget: "updated"},
		{ID: "b"},
		{ID: "a"},
	}
	byID := make(map[SymbolID]*SymbolFact, len(facts))
	for index := range facts {
		byID[facts[index].ID] = &facts[index]
	}

	got, _ := orderSymbolFacts(facts, byID, []SymbolID{"a", "c", "d"}, nil)
	want := []SymbolFact{
		{ID: "a"},
		{ID: "b"},
		{ID: "d", AliasTarget: "updated"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ordered facts = %#v, want %#v", got, want)
	}
}

func TestPatchCanonicalSymbolsCopiesPreviousOrderAndChangesOnlyDeltaRows(t *testing.T) {
	oldReference := Location{Path: "old.ts", StartByte: 1, EndByte: 2}
	newReference := Location{Path: "new.ts", StartByte: 3, EndByte: 4}
	previous := []SymbolFact{
		{ID: "a", Declarations: []Declaration{{Location: oldReference}}, References: []Location{oldReference}},
		{ID: "b", Declarations: []Declaration{{Location: oldReference}}, References: []Location{oldReference}},
	}
	current := []SymbolFact{
		{ID: "b", Declarations: []Declaration{{Location: newReference}}},
		{ID: "a", Declarations: []Declaration{{Location: oldReference}}},
	}
	cachedReferences := map[SymbolID][]Location{
		"a": {oldReference},
		"b": {oldReference},
	}
	builder := closureBuilder{
		backend: canonicalPatchBackend{
			changed:    []SymbolID{"b"},
			references: map[SymbolID][]Location{"b": {newReference}},
		},
		symbolSeen:             map[SymbolID]struct{}{"a": {}, "b": {}},
		fullTier:               map[SymbolID]struct{}{"a": {}, "b": {}},
		referencesOnlyFullTier: true,
		cachedReferences:       cachedReferences,
		cachedCanonicalSymbols: previous,
		changedSymbolIDs:       map[SymbolID]struct{}{"b": {}},
	}

	got, ok, err := builder.patchCanonicalSymbols(context.Background(), current)
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("stable canonical table did not take the retained patch path")
	}
	want := []SymbolFact{
		{ID: "a", Declarations: []Declaration{{Location: oldReference}}, References: []Location{oldReference}},
		{ID: "b", Declarations: []Declaration{{Location: newReference}}, References: []Location{newReference}},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("patched facts = %#v, want %#v", got, want)
	}
	if builder.patchedSymbolRows == 0 {
		t.Fatal("retained patch path did not report any patched rows")
	}
}
