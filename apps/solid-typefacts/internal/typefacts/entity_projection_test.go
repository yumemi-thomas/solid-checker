package typefacts

import (
	"reflect"
	"testing"
)

// projectionRow builds one entity with a meaningful value in every field,
// including the four JSON excludes. Each call allocates fresh nested storage,
// so a second call is an independent expected value, not an alias.
func projectionRow(path string, start int) EntityFact {
	return EntityFact{
		Location:                   Location{Path: path, StartByte: start, EndByte: start + 3},
		Symbol:                     SymbolID("s:" + path),
		SymbolUnresolved:           false,
		TypeDescriptor:             &TypeDescriptor{Text: "T", OriginModule: "m", AliasDeclarations: []Declaration{{Name: "A", Kind: "alias", Location: Location{Path: path}}}},
		ResolvedCall:               &Call{Target: "target", ReturnTypeText: "R"},
		Callability:                CallabilityCallable,
		RuntimeValueDomain:         &RuntimeValueDomain{MayBeCallable: true},
		PrimitiveValueDomain:       PrimitiveValueDomain{bits: 5},
		PrimitiveLiteralCandidates: []PrimitiveLiteralCandidate{{Kind: PrimitiveLiteralString, String: "lit"}},
		Constructability:           ConstructabilityConstructable,
		CallResultDomain:           &RuntimeValueDomain{MayBeUndefined: true},
		ConstantValue:              &ConstantValue{Kind: ConstantValueString, String: "c"},
		ArrayShape:                 ArrayShapeArray,
		TupleShape:                 &TupleShape{FixedLength: 2},
		LibraryTypes:               []string{"Promise"},
		ReferenceSpace:             ReferenceSpaceValue,
		RuntimeIdentity:            RuntimeSymbolID("r:" + path),
		RuntimeBindingKind:         RuntimeBindingCallable,
	}
}

func projected(table *FactTable) []EntityFact {
	var rows []EntityFact
	table.RangeEntities(func(entity EntityFact) { rows = append(rows, entity) })
	return rows
}

func privatelyIterated(table *FactTable) []EntityFact {
	var rows []EntityFact
	table.rangeWireEntities(func(entity EntityFact) { rows = append(rows, entity) })
	return rows
}

func TestRangeEntitiesIsTheCanonicalWireIteration(t *testing.T) {
	public := &FactTable{Entities: []EntityFact{projectionRow("/a", 1), projectionRow("/b", 2)}}
	retained := &FactTable{
		Entities: []EntityFact{projectionRow("/x", 9)},
		entityRuns: []factTableEntityRun{
			{entities: []EntityFact{projectionRow("/a", 1)}},
			{entities: []EntityFact{projectionRow("/b", 2), projectionRow("/c", 3)}},
		},
	}
	for name, test := range map[string]struct {
		table *FactTable
		want  []EntityFact
	}{
		"public fallback":     {public, []EntityFact{projectionRow("/a", 1), projectionRow("/b", 2)}},
		"retained precedence": {retained, []EntityFact{projectionRow("/a", 1), projectionRow("/b", 2), projectionRow("/c", 3)}},
	} {
		got := projected(test.table)
		if !reflect.DeepEqual(got, test.want) {
			t.Errorf("%s: projected %+v, want %+v", name, got, test.want)
		}
		if !reflect.DeepEqual(got, privatelyIterated(test.table)) {
			t.Errorf("%s: projection differs from rangeWireEntities", name)
		}
		if len(got) != test.table.wireEntityCount() {
			t.Errorf("%s: %d rows, wireEntityCount %d", name, len(got), test.table.wireEntityCount())
		}
	}
}

func TestRangeEntitiesEmptyShapes(t *testing.T) {
	var nilTable *FactTable
	visits := 0
	nilTable.RangeEntities(func(EntityFact) { visits++ })
	if visits != 0 || nilTable.wireEntityCount() != 0 {
		t.Fatalf("a nil table visited %d rows", visits)
	}
	for name, table := range map[string]*FactTable{
		"zero table": {},
		// Non-nil empty runs are the post-release shape: they suppress the
		// public fallback even when public rows exist.
		"non-nil empty runs": {Entities: []EntityFact{projectionRow("/x", 9)}, entityRuns: []factTableEntityRun{}},
	} {
		if got := projected(table); len(got) != 0 || len(privatelyIterated(table)) != 0 || table.wireEntityCount() != 0 {
			t.Errorf("%s: projected %+v", name, got)
		}
	}
}

// TestRangeEntitiesOwnerReleaseLeavesNothingVisitable runs the closure's own
// transport release over a populated retained table.
func TestRangeEntitiesOwnerReleaseLeavesNothingVisitable(t *testing.T) {
	table := &FactTable{entityRuns: []factTableEntityRun{{entities: []EntityFact{projectionRow("/a", 1)}}}}
	if len(projected(table)) != 1 {
		t.Fatal("the populated retained table is not visitable")
	}
	closure := &DemandClosure{table: table}
	closure.releaseTransportRows()
	if got := projected(table); len(got) != 0 || table.entityRuns == nil || table.wireEntityCount() != 0 {
		t.Fatalf("after release: projected %+v, runs nil %v", got, table.entityRuns == nil)
	}
}

// TestRangeEntitiesDoesNotMutateTheTable checks the bounded claim the visitor
// makes: it changes nothing itself, and a callback reassigning its own value
// or that value's top-level fields changes nothing either. Nested storage is
// shared, as documented, so a callback writing through a pointer would be a
// contract violation; this test asserts the sharing rather than exercising it.
func TestRangeEntitiesDoesNotMutateTheTable(t *testing.T) {
	table := &FactTable{entityRuns: []factTableEntityRun{
		{entities: []EntityFact{projectionRow("/a", 1), projectionRow("/b", 2)}},
	}}
	want := []EntityFact{projectionRow("/a", 1), projectionRow("/b", 2)}
	index := 0
	table.RangeEntities(func(entity EntityFact) {
		if entity.TypeDescriptor != table.entityRuns[0].entities[index].TypeDescriptor {
			t.Errorf("row %d: nested storage is not the table's own", index)
		}
		index++
		entity.Location.Path = "/rewritten"
		entity.Symbol = ""
		entity.TypeDescriptor = nil
		entity.PrimitiveValueDomain = PrimitiveValueDomain{}
		entity.LibraryTypes = nil
		entity = EntityFact{}
		_ = entity
	})
	if !reflect.DeepEqual(table.entityRuns[0].entities, want) || table.Entities != nil {
		t.Fatalf("the table changed under projection:\n got %+v\nwant %+v", table.entityRuns[0].entities, want)
	}
	if !reflect.DeepEqual(projected(table), want) {
		t.Fatal("a second projection differs from the independent expected rows")
	}
}
