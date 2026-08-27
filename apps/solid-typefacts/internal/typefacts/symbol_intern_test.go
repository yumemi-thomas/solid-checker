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

func TestSymbolHandleSetGrowsGeometricallyDuringColdInterning(t *testing.T) {
	interner := newSymbolInterner()
	set := newSymbolHandleSet(interner, nil)
	previousCapacity := cap(set.members)
	growths := 0
	for index := range 50_000 {
		set.addID(SymbolID(fmt.Sprintf("symbol:h:%024d", index)))
		if capacity := cap(set.members); capacity != previousCapacity {
			growths++
			if previousCapacity != 0 && capacity < previousCapacity*2 {
				t.Fatalf("growth %d -> %d is not geometric", previousCapacity, capacity)
			}
			previousCapacity = capacity
		}
	}
	if growths > 11 {
		t.Fatalf("50,000 cold symbols caused %d backing allocations, want at most 11", growths)
	}
	if set.len() != 50_000 {
		t.Fatalf("set length = %d, want 50,000", set.len())
	}
}

func BenchmarkColdSymbolHandleSetMemory(b *testing.B) {
	const symbols = 50_000
	ids := make([]SymbolID, symbols)
	for index := range ids {
		ids[index] = SymbolID(fmt.Sprintf("symbol:h:%024d", index))
	}
	b.ReportAllocs()
	b.ReportMetric(symbols, "symbols/op")
	for b.Loop() {
		interner := newSymbolInterner()
		set := newSymbolHandleSet(interner, nil)
		for _, id := range ids {
			set.addID(id)
		}
		if set.len() != symbols {
			b.Fatalf("set length = %d, want %d", set.len(), symbols)
		}
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
