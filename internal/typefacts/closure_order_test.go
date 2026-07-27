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
	interner := newSymbolInterner()
	factIndex := make([]int32, len(facts))
	for index := range facts {
		factIndex[interner.handle(facts[index].ID)] = int32(index) + 1
	}

	got, _ := orderSymbolFacts(facts, factIndex, interner, []SymbolID{"a", "c", "d"}, nil)
	want := []SymbolFact{
		{ID: "a"},
		{ID: "b"},
		{ID: "d", AliasTarget: "updated"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ordered facts = %#v, want %#v", got, want)
	}
}

func TestPatchCanonicalSymbolStoreChangesOnlyDeltaRows(t *testing.T) {
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
	interner := newSymbolInterner()
	builder := closureBuilder{
		backend: canonicalPatchBackend{
			changed:    []SymbolID{"b"},
			references: map[SymbolID][]Location{"b": {newReference}},
		},
		interner:             interner,
		symbolSeen:           testHandleSet(interner, "a", "b"),
		fullTier:             testHandleSet(interner, "a", "b"),
		cachedReferences:     cachedReferences,
		cachedCanonicalStore: newSymbolFactStore(previous),
		changedSymbols:       testChangedSet(interner, "b"),
	}

	store, ok, err := builder.patchCanonicalSymbolStore(context.Background(), current)
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("stable canonical table did not take the retained patch path")
	}
	got := store.symbolFactsSlice()
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
