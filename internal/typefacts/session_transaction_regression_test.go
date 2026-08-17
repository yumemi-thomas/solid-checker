package typefacts

import (
	"context"
	"errors"
	"testing"
)

// postMaterializationCancelBackend cancels after a successful reference batch.
// Session.lifecycle must not publish the closure's partially built state when
// cancellation arrives between materialization and response publication.
type postMaterializationCancelBackend struct {
	transportOnlyBackend
	sources                []SourceFile
	cancelAfterReferences  context.CancelFunc
	released               bool
	referenceBeforeRelease bool
}

func (b *postMaterializationCancelBackend) SourceFiles(context.Context) ([]SourceFile, error) {
	return append([]SourceFile(nil), b.sources...), nil
}

func (b *postMaterializationCancelBackend) ReferencesBatch(
	ctx context.Context,
	ids []SymbolID,
) (map[SymbolID][]Location, error) {
	if b.released {
		return nil, errors.New("reference evidence requested after analysis release")
	}
	b.referenceBeforeRelease = true
	references, err := b.transportOnlyBackend.ReferencesBatch(ctx, ids)
	if err == nil && b.cancelAfterReferences != nil {
		cancel := b.cancelAfterReferences
		b.cancelAfterReferences = nil
		cancel()
	}
	return references, err
}

func (b *postMaterializationCancelBackend) ReleaseAnalysisState() {
	b.released = true
}

func evidenceQuery(location Location) SymbolQueryV6 {
	return SymbolQueryV6{ID: doubleSymbolID(location), References: true}
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
	return EntityDemand{Location: location, Symbol: true, References: true}
}

func TestCancelledMaterializationCannotPublishAClosureGeneration(t *testing.T) {
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

	removed := lifecycleRequest(2, LifecycleAnalyze, 1)
	removed.StateToken = initialResponse.StateToken
	removed.RemovedDemandPaths = []string{path}
	removed.SymbolQueries = []SymbolQueryV6{evidenceQuery(oldLocation)}
	cancelledContext, cancel := context.WithCancel(context.Background())
	backend.cancelAfterReferences = cancel
	cancelledResponse := session.Lifecycle(cancelledContext, removed)
	cancel()
	if cancelledResponse.Error == nil || cancelledResponse.Error.Code != "analysis-cancelled" {
		t.Fatalf("post-materialization cancellation = %+v", cancelledResponse)
	}
	if session.retained.table == nil || session.retained.tokenText != initialResponse.StateToken {
		t.Fatal("cancelled analyze changed the session-published state")
	}
	if session.closure.table != nil || session.closure.retained.get(path) != nil {
		t.Fatal("cancelled materialization remained published inside the closure")
	}

	retry := removed
	retry.RequestID = 3
	retryResponse := session.Lifecycle(context.Background(), retry)
	if !retryResponse.OK || retryResponse.StateToken == initialResponse.StateToken {
		t.Fatalf("retry analyze = %+v", retryResponse)
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
	initial.Demands = []EntityDemand{regressionDemand(firstOld), regressionDemand(secondOld)}
	initialResponse := session.Lifecycle(context.Background(), initial)
	if !initialResponse.OK || initialResponse.StateToken == "" {
		t.Fatalf("initial analyze = %+v", initialResponse)
	}

	changeFirst := lifecycleRequest(2, LifecycleAnalyze, 1)
	changeFirst.StateToken = initialResponse.StateToken
	changeFirst.Demands = []EntityDemand{regressionDemand(firstCancelled)}
	changeFirst.SymbolQueries = []SymbolQueryV6{evidenceQuery(firstOld)}
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
	retainedSecond := session.retained.demands.at(secondPath)
	if len(retainedSecond) != 1 || retainedSecond[0].Location != secondNext {
		t.Fatal("accepted second-path demand was not published")
	}
}

func TestSymbolEvidencePrecedesAnalysisRelease(t *testing.T) {
	const path = "/project/source.ts"
	location := Location{Path: path, StartByte: 1, EndByte: 2}
	session, backend := newPostMaterializationCancelSession(t, SourceFile{
		Path: path, Source: []byte("export const value = 1\n"),
	})
	request := lifecycleRequest(1, LifecycleAnalyze, 1)
	request.ResetState = true
	request.Demands = []EntityDemand{regressionDemand(location)}
	request.SymbolQueries = []SymbolQueryV6{evidenceQuery(location)}
	request.ReleaseAnalysis = true
	response := session.Lifecycle(context.Background(), request)
	if !response.OK {
		t.Fatalf("analysis with symbol evidence = %+v", response)
	}
	if !backend.referenceBeforeRelease || !backend.released {
		t.Fatalf("evidence/release order = beforeRelease:%t released:%t", backend.referenceBeforeRelease, backend.released)
	}
}
