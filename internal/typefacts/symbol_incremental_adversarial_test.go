package typefacts

import (
	"bytes"
	"context"
	"testing"
)

// aliasMutationBackend makes the alias graph independently mutable from the
// demanded roots. It exercises the proof gates that a real checker generation
// drives, without relying on TypeScript's recovery behavior for illegal alias
// cycles.
type aliasMutationBackend struct {
	transportOnlyBackend
	aliases     map[SymbolID]SymbolID
	nextAliases map[SymbolID]SymbolID
}

func (b *aliasMutationBackend) ResolveAlias(_ context.Context, id SymbolID) (SymbolID, error) {
	target, ok := b.aliases[id]
	if !ok {
		return "", ErrNotFound
	}
	return target, nil
}

func (b *aliasMutationBackend) Update(context.Context, []FileChange) (AffectedSet, error) {
	if b.nextAliases != nil {
		b.aliases = b.nextAliases
		b.nextAliases = nil
	}
	return AffectedSet{Files: []string{b.source.Path}}, nil
}

func fullTransitionForAdversarialTest(t *testing.T, table *FactTable) []byte {
	t.Helper()
	transition, err := (&wireTransitionEncoder{}).Encode(wireTransitionInput{
		ProjectID: "adversarial",
		Target:    table,
	})
	if err != nil {
		t.Fatal(err)
	}
	return transition.Bytes
}

func freshAliasTable(
	t *testing.T,
	backend *aliasMutationBackend,
	generation uint64,
	groups []DemandGroup,
) *FactTable {
	t.Helper()
	freshBackend := &aliasMutationBackend{
		transportOnlyBackend: backend.transportOnlyBackend,
		aliases:              backend.aliases,
	}
	closure, err := NewDemandClosure(freshBackend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })
	for closure.generation < generation {
		if _, err := closure.Update(context.Background(), nil); err != nil {
			t.Fatal(err)
		}
	}
	table, err := closure.DemandTableForGroups(context.Background(), generation, groups, nil)
	if err != nil {
		t.Fatal(err)
	}
	return table
}

func assertAdversarialFreshParity(
	t *testing.T,
	backend *aliasMutationBackend,
	generation uint64,
	groups []DemandGroup,
	retained *FactTable,
) {
	t.Helper()
	fresh := freshAliasTable(t, backend, generation, groups)
	if !bytes.Equal(
		fullTransitionForAdversarialTest(t, retained),
		fullTransitionForAdversarialTest(t, fresh),
	) {
		t.Fatalf("generation %d retained alias closure differs from fresh materialization", generation)
	}
}

func tableSymbolFacts(table *FactTable) map[SymbolID]SymbolFact {
	facts := make(map[SymbolID]SymbolFact, table.symbolFactsCount())
	table.rangeSymbolFacts(func(fact SymbolFact) {
		facts[fact.ID] = fact
	})
	return facts
}

func TestSparseMaterializationLeavesSymbolClosureToClient(t *testing.T) {
	backend := &aliasMutationBackend{
		transportOnlyBackend: transportOnlyBackend{source: SourceFile{
			Path: "a.ts", Source: []byte("const value = 1;"),
		}},
		aliases: map[SymbolID]SymbolID{},
	}
	closure, err := NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })
	table, err := closure.DemandTableForGroups(context.Background(), 1, []DemandGroup{{
		Path: "a.ts",
		Demands: []EntityDemand{{
			Location: Location{Path: "a.ts", StartByte: 6, EndByte: 11},
			Symbol:   true,
		}},
	}}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if table.symbols != nil || len(table.Symbols) != 0 {
		t.Fatalf("producer retained symbol closure rows: symbols=%d store=%v", len(table.Symbols), table.symbols != nil)
	}
	if len(table.entityRuns) != 1 || len(table.entityRuns[0].entities) != 1 {
		t.Fatalf("sparse entity runs = %#v, want one path row", table.entityRuns)
	}
}

func aliasCycleLastRootReferenceTierAndRetargetMatchFresh(t *testing.T) {
	ctx := context.Background()
	path := "a.ts"
	locationA := Location{Path: path, StartByte: 1, EndByte: 2}
	locationB := Location{Path: path, StartByte: 3, EndByte: 4}
	locationC := Location{Path: path, StartByte: 5, EndByte: 6}
	idA, idB, idC := doubleSymbolID(locationA), doubleSymbolID(locationB), doubleSymbolID(locationC)
	backend := &aliasMutationBackend{
		transportOnlyBackend: transportOnlyBackend{source: SourceFile{Path: path, Source: []byte("abcdef")}},
		aliases:              map[SymbolID]SymbolID{idA: idB, idB: idA},
	}
	closure, err := NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	withReferences := []DemandGroup{{
		Path: path,
		Demands: []EntityDemand{{
			Location: locationA, Symbol: true, References: true,
		}},
	}}
	table, err := closure.DemandTableForGroups(ctx, 1, withReferences, []string{path})
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 1, withReferences, table)
	facts := tableSymbolFacts(table)
	if len(facts) != 2 {
		t.Fatalf("alias cycle did not reach a fixed point: %#v", facts)
	}

	// Removing the final external root must clear the whole unsupported cycle.
	table, err = closure.DemandTableForGroups(ctx, 1, nil, []string{path})
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 1, nil, table)
	if facts := tableSymbolFacts(table); len(facts) != 0 {
		t.Fatalf("alias cycle survived final-root removal: %#v", facts)
	}

	// Restoring the root outside Full tier must restore both symbols without
	// retaining either reference list from the preceding Full-tier generation.
	withoutReferences := []DemandGroup{{
		Path: path,
		Demands: []EntityDemand{{
			Location: locationA, Symbol: true,
		}},
	}}
	table, err = closure.DemandTableForGroups(ctx, 1, withoutReferences, []string{path})
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 1, withoutReferences, table)
	facts = tableSymbolFacts(table)
	if len(facts) != 2 || len(facts[idA].References) != 0 || len(facts[idB].References) != 0 {
		t.Fatalf("reference-tier removal retained references: %#v", facts)
	}

	// Equal roots with a retargeted alias must reject the stable-universe patch
	// and recompute the new fixed point, removing the old target.
	backend.nextAliases = map[SymbolID]SymbolID{idA: idC}
	if _, err := closure.Update(ctx, nil); err != nil {
		t.Fatal(err)
	}
	table, err = closure.DemandTableForGroups(ctx, 2, withoutReferences, nil)
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 2, withoutReferences, table)
	facts = tableSymbolFacts(table)
	if _, ok := facts[idB]; ok {
		t.Fatalf("old alias target survived retargeting: %#v", facts)
	}
	if facts[idA].AliasTarget != idC {
		t.Fatalf("alias target = %q, want %q", facts[idA].AliasTarget, idC)
	}
	if _, ok := facts[idC]; !ok {
		t.Fatalf("new alias target is absent: %#v", facts)
	}

	// Changing only Full-tier membership must populate and then remove the
	// canonical target's reference list without changing reachable symbols.
	table, err = closure.DemandTableForGroups(ctx, 2, withReferences, []string{path})
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 2, withReferences, table)
	facts = tableSymbolFacts(table)
	if len(facts[idC].References) == 0 {
		t.Fatalf("reference-tier addition did not hydrate canonical target: %#v", facts)
	}
	table, err = closure.DemandTableForGroups(ctx, 2, withoutReferences, []string{path})
	if err != nil {
		t.Fatal(err)
	}
	assertAdversarialFreshParity(t, backend, 2, withoutReferences, table)
	facts = tableSymbolFacts(table)
	if len(facts[idC].References) != 0 {
		t.Fatalf("reference-tier removal retained canonical references: %#v", facts)
	}
}
