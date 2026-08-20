package typefacts

import (
	"context"
	"fmt"
	"slices"
	"testing"
)

func TestPreparedContributionTakesAndCompactsEntityBacking(t *testing.T) {
	location := Location{Path: "/project/source.ts", StartByte: 1, EndByte: 2}
	domain := &RuntimeValueDomain{MayBeCallable: true, MayBeUndefined: true}
	primitiveDomain := NewPrimitiveValueDomain(true, false, true, false, false, false, false, false)
	callResultDomain := &RuntimeValueDomain{MayBeOther: true}
	constantValue := &ConstantValue{Kind: ConstantValueString, String: "constant"}
	tupleShape := &TupleShape{FixedLength: 2, ElementZero: CallabilityCallable, ElementZeroMinimumParameters: 2}
	entities := []EntityFact{
		{Location: location, Symbol: "symbol"},
		{
			Location:             location,
			TypeDescriptor:       &TypeDescriptor{Text: "Value"},
			Callability:          CallabilityCallable,
			RuntimeValueDomain:   domain,
			PrimitiveValueDomain: primitiveDomain,
			CallResultDomain:     callResultDomain,
			ConstantValue:        constantValue,
			ArrayShape:           ArrayShapeArray,
			TupleShape:           tupleShape,
			LibraryTypes:         []string{"Date"},
		},
	}
	structural := []SymbolID{"", "accessor"}
	contribution, err := prepareRetainedContribution(
		location.Path,
		1,
		[]EntityDemand{
			{Location: location, Symbol: true},
			{Location: location, TypeDescriptor: true, Callability: true, RuntimeValueDomain: true, PrimitiveValueDomain: true, CallResultDomain: true, ConstantValue: true, ArrayShape: true, TupleShape: true, LibraryTypes: true},
		},
		SemanticDemandRunResult{
			Entities:   entities,
			Structural: structural,
			Durable:    true,
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(contribution.entities) != 1 {
		t.Fatalf("compacted entities = %d, want 1", len(contribution.entities))
	}
	if &contribution.entities[0] != &entities[0] {
		t.Fatal("prepared contribution cloned the TS-Go entity backing")
	}
	if len(contribution.structural) != 1 ||
		&contribution.structural[0] != &structural[0] ||
		contribution.structural[0] != "accessor" {
		t.Fatalf("prepared structural evidence did not take its arena: %v", contribution.structural)
	}
	if contribution.entities[0].Symbol != "symbol" ||
		contribution.entities[0].Callability != CallabilityCallable ||
		contribution.entities[0].RuntimeValueDomain != domain ||
		contribution.entities[0].PrimitiveValueDomain != primitiveDomain ||
		contribution.entities[0].CallResultDomain != callResultDomain ||
		contribution.entities[0].ConstantValue != constantValue ||
		contribution.entities[0].ArrayShape != ArrayShapeArray ||
		contribution.entities[0].TupleShape != tupleShape ||
		!slices.Equal(contribution.entities[0].LibraryTypes, []string{"Date"}) {
		t.Fatalf("compacted entity lost demand fields: %+v", contribution.entities[0])
	}
	if !slices.Equal(contribution.descriptorSymbols, []SymbolID{"symbol"}) {
		t.Fatalf("descriptor symbols = %v, want symbol", contribution.descriptorSymbols)
	}
}

func retainedStoreTestContribution(
	dependencies []string,
	descriptorSymbols []SymbolID,
	durable bool,
) *retainedContribution {
	entities := make([]EntityFact, len(descriptorSymbols))
	for index, symbol := range descriptorSymbols {
		entities[index].Symbol = symbol
	}
	return &retainedContribution{
		dependencies:      append([]string(nil), dependencies...),
		entities:          entities,
		descriptorSymbols: append([]SymbolID(nil), descriptorSymbols...),
		durable:           durable,
	}
}

func TestRetainedContributionStoreInvalidatesOnlyDirectDependents(t *testing.T) {
	const (
		source     = "/project/source.ts"
		direct     = "/project/direct.ts"
		peer       = "/project/peer.ts"
		transitive = "/project/transitive.ts"
		unrelated  = "/project/unrelated.ts"
	)
	var store retainedContributionStore
	store.add(source, retainedStoreTestContribution(nil, nil, true))
	store.add(direct, retainedStoreTestContribution([]string{source}, nil, true))
	store.add(peer, retainedStoreTestContribution([]string{source}, nil, true))
	store.add(transitive, retainedStoreTestContribution([]string{direct}, nil, true))
	store.add(unrelated, retainedStoreTestContribution([]string{"/project/elsewhere.ts"}, nil, true))

	changed := map[string]struct{}{source: {}}
	store.invalidate(changed)

	if len(changed) != 3 {
		t.Fatalf("invalidated paths = %#v, want source and two direct dependents", changed)
	}
	for _, path := range []string{source, direct, peer} {
		if _, ok := changed[path]; !ok {
			t.Errorf("invalidated paths omit %q: %#v", path, changed)
		}
		if store.get(path) != nil {
			t.Errorf("contribution %q survived source invalidation", path)
		}
	}
	for _, path := range []string{transitive, unrelated} {
		if _, ok := changed[path]; ok {
			t.Errorf("source invalidation fanned out to %q: %#v", path, changed)
		}
		if store.get(path) == nil {
			t.Errorf("unrelated contribution %q was removed", path)
		}
	}
}

func TestPathMembershipPromotesWithoutChangingSetSemantics(t *testing.T) {
	var membership pathMembership
	for index := range pathMembershipMapThreshold + 4 {
		path := fmt.Sprintf("/project/member-%02d.ts", index)
		membership = membership.add(path)
		membership = membership.add(path)
	}
	if membership.large == nil {
		t.Fatal("high-fanout membership did not promote")
	}
	membership = membership.remove("/project/member-03.ts")
	membership = membership.remove("/project/not-present.ts")

	got := make([]string, 0, membership.len())
	membership.rangePaths(func(path string) {
		got = append(got, path)
	})
	slices.Sort(got)
	if len(got) != pathMembershipMapThreshold+3 {
		t.Fatalf("membership size = %d, want %d", len(got), pathMembershipMapThreshold+3)
	}
	if slices.Contains(got, "/project/member-03.ts") {
		t.Fatal("removed path remains in promoted membership")
	}
}

func TestRetainedContributionStoreDiscardDoesNotFanOut(t *testing.T) {
	const (
		source = "/project/source.ts"
		direct = "/project/direct.ts"
		peer   = "/project/peer.ts"
	)
	var store retainedContributionStore
	store.add(source, retainedStoreTestContribution(nil, nil, true))
	directContribution := retainedStoreTestContribution([]string{source}, nil, true)
	peerContribution := retainedStoreTestContribution([]string{source}, nil, true)
	store.add(direct, directContribution)
	store.add(peer, peerContribution)

	discarded := map[string]struct{}{source: {}}
	store.discard(discarded)

	if len(discarded) != 1 {
		t.Fatalf("discard expanded to other demand runs: %#v", discarded)
	}
	if store.get(source) != nil {
		t.Fatal("discarded demand run is still retained")
	}
	if store.get(direct) != directContribution || store.get(peer) != peerContribution {
		t.Fatal("discard removed a source-dependent contribution")
	}
	var gotDependents []string
	store.dependentsByPath[source].rangePaths(func(path string) {
		gotDependents = append(gotDependents, path)
	})
	slices.Sort(gotDependents)
	if !slices.Equal(gotDependents, []string{direct, peer}) {
		t.Fatalf("source dependency users = %#v, want direct demand runs", gotDependents)
	}
}

func TestRetainedContributionStoreReconcilesDescriptorUsers(t *testing.T) {
	const (
		pathA = "/project/a.ts"
		pathB = "/project/b.ts"
		pathC = "/project/c.ts"

		oldSymbol    SymbolID = "symbol:h:old"
		sharedSymbol SymbolID = "symbol:h:shared"
		newSymbol    SymbolID = "symbol:h:new"
	)
	var store retainedContributionStore
	oldA := retainedStoreTestContribution(nil, []SymbolID{oldSymbol, sharedSymbol}, true)
	oldB := retainedStoreTestContribution(nil, []SymbolID{oldSymbol}, true)
	store.add(pathA, oldA)
	store.add(pathB, oldB)

	replacementA := retainedStoreTestContribution(nil, []SymbolID{newSymbol, sharedSymbol}, true)
	newC := retainedStoreTestContribution(nil, []SymbolID{newSymbol}, true)
	nonDurable := retainedStoreTestContribution(nil, []SymbolID{oldSymbol}, false)
	desiredScratch := map[string]struct{}{"/stale/scratch.ts": {}}
	store.commit([]demandGroup{
		{path: pathA, contribution: replacementA},
		{path: pathC, contribution: newC},
		{path: "/project/volatile.ts", contribution: nonDurable},
	}, desiredScratch)

	if len(desiredScratch) != 0 {
		t.Fatalf("reconcile left desired scratch populated: %#v", desiredScratch)
	}
	if store.get(pathA) != replacementA || store.get(pathC) != newC {
		t.Fatal("reconcile did not install replacement contributions")
	}
	if store.get(pathB) != nil || store.get("/project/volatile.ts") != nil {
		t.Fatal("reconcile retained a removed or non-durable contribution")
	}
	if got := store.descriptorUsers[oldSymbol].len(); got != 0 {
		t.Fatalf("old descriptor user count = %d, want none", got)
	}
	if got := store.descriptorUsers[sharedSymbol]; got.len() != 1 {
		t.Fatalf("shared descriptor user count = %d, want 1", got.len())
	}
	var gotNewUsers []string
	store.rangeDescriptorUsers(newSymbol, func(path string) {
		gotNewUsers = append(gotNewUsers, path)
	})
	slices.Sort(gotNewUsers)
	if !slices.Equal(gotNewUsers, []string{pathA, pathC}) {
		t.Fatalf("new descriptor users = %#v, want replacement paths", gotNewUsers)
	}

	store.commit([]demandGroup{{path: pathC, contribution: newC}}, desiredScratch)
	if store.get(pathA) != nil {
		t.Fatal("second reconcile did not remove departed contribution")
	}
	if got := store.descriptorUsers[sharedSymbol].len(); got != 0 {
		t.Fatalf("removed contribution remains in shared descriptor index: %d users", got)
	}
	var gotRemainingUsers []string
	store.rangeDescriptorUsers(newSymbol, func(path string) {
		gotRemainingUsers = append(gotRemainingUsers, path)
	})
	if !slices.Equal(gotRemainingUsers, []string{pathC}) {
		t.Fatalf("new descriptor users after removal = %#v, want [%q]", gotRemainingUsers, pathC)
	}
}

type preparedContributionFailureBackend struct {
	transportOnlyBackend
	failPreparation bool
	semanticCalls   int
	referenceCalls  int
}

func (b *preparedContributionFailureBackend) SemanticDemandRuns(
	ctx context.Context,
	runs []SemanticDemandRun,
	scope SemanticScope,
) ([]SemanticDemandRunResult, error) {
	b.semanticCalls++
	results, err := b.transportOnlyBackend.SemanticDemandRuns(ctx, runs, scope)
	if err == nil && b.failPreparation {
		b.failPreparation = false
		results[0].Dependencies = []string{"/project/z.ts", "/project/a.ts"}
	}
	return results, err
}

func (b *preparedContributionFailureBackend) ReferencesBatch(
	ctx context.Context,
	ids []SymbolID,
) (map[SymbolID][]Location, error) {
	b.referenceCalls++
	return b.transportOnlyBackend.ReferencesBatch(ctx, ids)
}

func TestRetainedContributionCommitIsTransactional(t *testing.T) {
	const (
		path        = "/project/source.ts"
		oldPath     = "/project/old.ts"
		oldSuppress = SymbolID("symbol:h:old-suppression")
	)
	location := Location{Path: path, StartByte: 13, EndByte: 18}
	backend := &preparedContributionFailureBackend{
		transportOnlyBackend: transportOnlyBackend{source: SourceFile{
			Path: path, Source: []byte("export const value = 1\n"),
		}},
		failPreparation: true,
	}
	closure, err := NewDemandClosure(backend, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	oldContribution := retainedStoreTestContribution(nil, []SymbolID{oldSuppress}, true)
	closure.retained.add(oldPath, oldContribution)
	closure.lastSuppression = map[SymbolID]struct{}{oldSuppress: {}}
	groups := []DemandGroup{{
		Path: path,
		Demands: []EntityDemand{{
			Location:           location,
			Symbol:             true,
			References:         true,
			StructuralAccessor: true,
		}},
	}}

	if _, err := closure.DemandTableForGroups(context.Background(), 1, groups, nil); err == nil {
		t.Fatal("failed materialization unexpectedly succeeded")
	}
	if backend.semanticCalls != 1 || backend.referenceCalls != 0 {
		t.Fatalf("backend calls after failure = semantic:%d references:%d, want 1 and 0",
			backend.semanticCalls, backend.referenceCalls)
	}
	if closure.retained.get(oldPath) != oldContribution || closure.retained.get(path) != nil {
		t.Fatal("failed materialization changed installed contributions")
	}
	if len(closure.lastSuppression) != 1 {
		t.Fatalf("last suppression after failure = %#v, want original set", closure.lastSuppression)
	}
	if _, ok := closure.lastSuppression[oldSuppress]; !ok {
		t.Fatalf("last suppression advanced after failure: %#v", closure.lastSuppression)
	}

	table, err := closure.DemandTableForGroups(context.Background(), 1, groups, nil)
	if err != nil {
		t.Fatalf("retry materialization: %v", err)
	}
	if table == nil || len(table.entityRuns) != 1 || len(table.entityRuns[0].entities) != 1 {
		t.Fatalf("retry table = %#v, want one entity", table)
	}
	if backend.semanticCalls != 2 || backend.referenceCalls != 0 {
		t.Fatalf("backend calls after retry = semantic:%d references:%d, want 2 and 0",
			backend.semanticCalls, backend.referenceCalls)
	}
	if closure.retained.get(oldPath) != nil || closure.retained.get(path) == nil {
		t.Fatal("successful retry did not commit the new contribution set")
	}
	newSuppress := doubleSymbolID(location)
	if len(closure.lastSuppression) != 1 {
		t.Fatalf("last suppression after retry = %#v, want one symbol", closure.lastSuppression)
	}
	if _, ok := closure.lastSuppression[newSuppress]; !ok {
		t.Fatalf("retry did not commit suppression %q: %#v", newSuppress, closure.lastSuppression)
	}
}

func BenchmarkRetainedContributionHighFanoutInvalidation(b *testing.B) {
	const dependency = "/project/shared.ts"
	for _, fanout := range []int{100, 1_000, 10_000} {
		b.Run(fmt.Sprintf("fanout-%d", fanout), func(b *testing.B) {
			for range b.N {
				b.StopTimer()
				var store retainedContributionStore
				store.add(dependency, retainedStoreTestContribution(nil, nil, true))
				for index := range fanout {
					path := fmt.Sprintf("/project/user-%05d.ts", index)
					store.add(path, retainedStoreTestContribution([]string{dependency}, nil, true))
				}
				changed := map[string]struct{}{dependency: {}}
				b.StartTimer()

				store.invalidate(changed)

				b.StopTimer()
				if len(changed) != fanout+1 {
					b.Fatalf("invalidated %d paths, want %d", len(changed), fanout+1)
				}
			}
		})
	}
}
