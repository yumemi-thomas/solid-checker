package typefacts

import (
	"bytes"
	"context"
	"testing"
)

// postMaterializationCancelBackend cancels only after ReferencesBatch has
// produced its successful answer. ReferencesBatch is the last fallible backend
// call on these retained-symbol test paths, and neither closeSymbols nor
// materializeSemanticDemandRetained checks the context again before publishing
// the new closure table. Session.lifecycle does check it immediately after
// DemandTableForGroups returns, which deterministically exercises the gap
// between closure publication and session publication.
type postMaterializationCancelBackend struct {
	transportOnlyBackend
	sources               []SourceFile
	cancelAfterReferences context.CancelFunc
}

func (b *postMaterializationCancelBackend) SourceFiles(context.Context) ([]SourceFile, error) {
	return append([]SourceFile(nil), b.sources...), nil
}

func (b *postMaterializationCancelBackend) ReferencesBatch(
	ctx context.Context,
	ids []SymbolID,
) (map[SymbolID][]Location, error) {
	references, err := b.transportOnlyBackend.ReferencesBatch(ctx, ids)
	if err == nil && b.cancelAfterReferences != nil {
		cancel := b.cancelAfterReferences
		b.cancelAfterReferences = nil
		cancel()
	}
	return references, err
}

func newPostMaterializationCancelSession(
	t *testing.T,
	sources ...SourceFile,
) (*Session, *postMaterializationCancelBackend) {
	t.Helper()
	backend := &postMaterializationCancelBackend{
		transportOnlyBackend: transportOnlyBackend{source: sources[0]},
		sources:              sources,
	}
	session, err := NewSession(backend, "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })
	return session, backend
}

func regressionDemand(location Location) EntityDemand {
	return EntityDemand{
		Location:   location,
		Symbol:     true,
		References: true,
	}
}

func TestCancelledMaterializationCannotAuthenticateExactDeltaByGenerationAlone(t *testing.T) {
	const path = "/project/source.ts"
	oldLocation := Location{Path: path, StartByte: 1, EndByte: 2}
	session, backend := newPostMaterializationCancelSession(t, SourceFile{
		Path: path, Source: []byte("export const value = 1\n"),
	})

	initial := lifecycleRequest(1, LifecycleAnalyze, 1)
	initial.ResetState = true
	initial.Demands = []EntityDemand{regressionDemand(oldLocation)}
	initialResponse := session.Lifecycle(context.Background(), initial)
	if !initialResponse.OK || initialResponse.StateToken == "" {
		t.Fatalf("initial analyze = %+v", initialResponse)
	}
	base := *session.retained.table
	if base.symbolFactsCount() != 1 {
		t.Fatalf("initial symbols = %d, want 1", base.symbolFactsCount())
	}

	removed := lifecycleRequest(2, LifecycleAnalyze, 1)
	removed.StateToken = initialResponse.StateToken
	removed.RemovedDemandPaths = []string{path}
	cancelledContext, cancel := context.WithCancel(context.Background())
	backend.cancelAfterReferences = cancel
	cancelledResponse := session.Lifecycle(cancelledContext, removed)
	cancel()
	if cancelledResponse.Error == nil || cancelledResponse.Error.Code != "analysis-cancelled" {
		t.Fatalf("post-materialization cancellation = %+v", cancelledResponse)
	}
	if session.retained.table == nil || session.retained.table.symbolFactsCount() != 1 {
		t.Fatal("cancelled analyze changed the session-published base table")
	}
	if session.closure.table != nil {
		t.Fatal("cancelled materialization remained published inside the closure")
	}

	retry := removed
	retry.RequestID = 3
	retryResponse := session.Lifecycle(context.Background(), retry)
	if !retryResponse.OK || len(retryResponse.TableTransition) == 0 {
		t.Fatalf("retry analyze = %+v", retryResponse)
	}
	target := session.retained.table
	if target == nil || target.symbolFactsCount() != 0 {
		t.Fatal("retry target did not remove the demanded symbol")
	}

	mismatchedTarget := *target
	mismatchedTarget.transport = &factTableTransportChanges{
		baseGeneration: base.Generation,
		baseStateID:    base.stateID + 1,
		sourcePaths:    map[string]struct{}{},
		entityPaths:    map[string]struct{}{},
		filePaths:      map[string]struct{}{},
		symbolIDs:      map[SymbolID]struct{}{},
		exact:          true,
	}
	manifestPlan, err := (&wireTransitionEncoder{tableSchema: TypeFactsTableSchemaVersionV3}).Encode(wireTransitionInput{
		ProjectID:      session.projectID,
		BaseStateToken: initialResponse.StateToken,
		Base:           &base,
		Target:         &mismatchedTarget,
	})
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(manifestPlan.Bytes, retryResponse.TableTransition) {
		t.Fatal("test did not reproduce the transition emitted by Session")
	}
	fallbackTarget := *target
	fallbackTarget.transport = nil
	fallbackPlan, err := (&wireTransitionEncoder{tableSchema: TypeFactsTableSchemaVersionV3}).Encode(wireTransitionInput{
		ProjectID:      session.projectID,
		BaseStateToken: initialResponse.StateToken,
		Base:           &base,
		Target:         &fallbackTarget,
	})
	if err != nil {
		t.Fatal(err)
	}
	if fallbackPlan.SymbolOperations != 1 {
		t.Fatalf("fallback oracle symbol operations = %d, want one removal", fallbackPlan.SymbolOperations)
	}
	if manifestPlan.SymbolOperations != fallbackPlan.SymbolOperations {
		t.Fatalf(
			"exact manifest authenticated a different same-generation base: emitted %d symbol operations, full diff requires %d",
			manifestPlan.SymbolOperations,
			fallbackPlan.SymbolOperations,
		)
	}
}

func TestCancelledDemandContributionCannotLeakWhenAnotherPathChanges(t *testing.T) {
	const (
		firstPath  = "/project/first.ts"
		secondPath = "/project/second.ts"
	)
	firstOld := Location{Path: firstPath, StartByte: 1, EndByte: 2}
	firstCancelled := Location{Path: firstPath, StartByte: 5, EndByte: 6}
	secondOld := Location{Path: secondPath, StartByte: 1, EndByte: 2}
	secondNext := Location{Path: secondPath, StartByte: 5, EndByte: 6}
	session, backend := newPostMaterializationCancelSession(
		t,
		SourceFile{Path: firstPath, Source: []byte("export const first = 1\n")},
		SourceFile{Path: secondPath, Source: []byte("export const second = 1\n")},
	)

	initial := lifecycleRequest(1, LifecycleAnalyze, 1)
	initial.ResetState = true
	initial.Demands = []EntityDemand{
		regressionDemand(firstOld),
		regressionDemand(secondOld),
	}
	initialResponse := session.Lifecycle(context.Background(), initial)
	if !initialResponse.OK || initialResponse.StateToken == "" {
		t.Fatalf("initial analyze = %+v", initialResponse)
	}

	changeFirst := lifecycleRequest(2, LifecycleAnalyze, 1)
	changeFirst.StateToken = initialResponse.StateToken
	changeFirst.Demands = []EntityDemand{regressionDemand(firstCancelled)}
	cancelledContext, cancel := context.WithCancel(context.Background())
	backend.cancelAfterReferences = cancel
	cancelledResponse := session.Lifecycle(cancelledContext, changeFirst)
	cancel()
	if cancelledResponse.Error == nil || cancelledResponse.Error.Code != "analysis-cancelled" {
		t.Fatalf("post-materialization cancellation = %+v", cancelledResponse)
	}
	if session.closure.table != nil || session.closure.retained.get(firstPath) != nil {
		t.Fatal("cancelled demand contribution remained published inside the closure")
	}

	changeSecond := lifecycleRequest(3, LifecycleAnalyze, 1)
	changeSecond.StateToken = initialResponse.StateToken
	changeSecond.Demands = []EntityDemand{regressionDemand(secondNext)}
	accepted := session.Lifecycle(context.Background(), changeSecond)
	if !accepted.OK {
		t.Fatalf("second-path analyze = %+v", accepted)
	}

	retainedFirst := session.retained.demands.at(firstPath)
	if len(retainedFirst) != 1 || retainedFirst[0].Location != firstOld {
		t.Fatal("session demand state no longer represents the last accepted first-path run")
	}
	actualFirst := canonicalEntityPath(session.retained.table.Entities, firstPath)
	if len(actualFirst) != 1 {
		t.Fatalf("accepted table first-path entities = %#v, want one", actualFirst)
	}
	if actualFirst[0].Location != firstOld {
		t.Fatalf(
			"cancelled first-path contribution leaked into a later accepted table: got %+v, want %+v",
			actualFirst[0].Location,
			firstOld,
		)
	}
}
