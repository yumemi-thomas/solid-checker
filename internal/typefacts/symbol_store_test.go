package typefacts

import (
	"fmt"
	"reflect"
	"testing"
)

func TestSymbolFactStoreSharesUntouchedChunksAcrossPatch(t *testing.T) {
	facts := make([]SymbolFact, 600)
	for index := range facts {
		facts[index] = SymbolFact{ID: SymbolID(fmt.Sprintf("symbol-%04d", index))}
	}
	previous := newSymbolFactStore(facts)
	present := make(map[SymbolID]struct{}, len(facts))
	for _, fact := range facts {
		present[fact.ID] = struct{}{}
	}
	changed := facts[300]
	changed.AliasTarget = "updated"

	next, shared, removed, complete := previous.Patch(
		present,
		map[SymbolID]SymbolFact{changed.ID: changed},
		nil,
	)

	if !complete {
		t.Fatal("copy-on-write patch did not preserve the complete symbol universe")
	}
	if len(removed) != 0 {
		t.Fatalf("removed symbols = %v, want none", removed)
	}
	if shared != 2 {
		t.Fatalf("shared chunks = %d, want 2", shared)
	}
	if &previous.chunks[0][0] != &next.chunks[0][0] {
		t.Fatal("untouched leading chunk was copied")
	}
	if &previous.chunks[2][0] != &next.chunks[2][0] {
		t.Fatal("untouched trailing chunk was copied")
	}
	got := next.symbolFactsSlice()
	want := append([]SymbolFact(nil), facts...)
	want[300] = changed
	if !reflect.DeepEqual(got, want) {
		t.Fatal("copy-on-write store differs from canonical patched rows")
	}
}

func TestSymbolFactStoreRejectsIncompleteRemovalCandidates(t *testing.T) {
	facts := []SymbolFact{{ID: "a"}, {ID: "b"}}
	previous := newSymbolFactStore(facts)
	present := map[SymbolID]struct{}{"a": {}}

	_, _, _, complete := previous.Patch(present, nil, nil)

	if complete {
		t.Fatal("patch accepted an incomplete removal-candidate set")
	}
}

func (s *symbolFactStore) symbolFactsSlice() []SymbolFact {
	facts := make([]SymbolFact, 0, s.Len())
	s.Range(func(fact SymbolFact) {
		facts = append(facts, fact)
	})
	return facts
}
