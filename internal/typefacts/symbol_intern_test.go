package typefacts

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
