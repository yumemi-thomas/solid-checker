package typefacts

import (
	"reflect"
	"testing"
)

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
