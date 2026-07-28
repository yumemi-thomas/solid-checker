package typefacts

import (
	"fmt"
	"testing"
)

// The interner is the one symbol structure with no eviction of its own: a
// session minting fresh non-durable IDs every generation grows it without
// bound unless the reset policy replaces it once dead identities outnumber
// the live universe.
func TestMaybeResetInternerBoundsDeadIdentities(t *testing.T) {
	closure := &DemandClosure{}
	closure.maybeResetInterner(10)
	healthy := closure.interner
	if healthy == nil {
		t.Fatal("no interner was created")
	}
	for index := range 100 {
		healthy.handle(SymbolID(fmt.Sprintf("symbol:h:%024d", index)))
	}
	closure.maybeResetInterner(10)
	if closure.interner != healthy {
		t.Fatal("an interner within bounds was reset")
	}
	for generation := range 20 {
		for index := range 100 {
			healthy.handle(SymbolID(fmt.Sprintf("symbol:%d:%d", generation, index)))
		}
	}
	closure.maybeResetInterner(10)
	if closure.interner == healthy {
		t.Fatalf("an interner holding %d identities for a 10-symbol universe survived", healthy.size())
	}
	if closure.interner.size() != 0 {
		t.Fatalf("the replacement interner is not empty: %d", closure.interner.size())
	}
}

// Test constructors for the interned symbol sets, so white-box fixtures can
// state memberships as ID lists the way they used to state map literals.

func testHandleSet(interner *symbolInterner, ids ...SymbolID) *symbolHandleSet {
	set := newSymbolHandleSet(interner, nil)
	for _, id := range ids {
		set.addID(id)
	}
	return set
}

func testChangedSet(interner *symbolInterner, ids ...SymbolID) *changedSymbolSet {
	set := newChangedSymbolSet(interner, nil, nil)
	for _, id := range ids {
		set.add(id)
	}
	return set
}
