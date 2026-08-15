package typefacts

import (
	"context"
	"fmt"
	"os"
	"testing"
)

type sessionTestBackend struct {
	transportOnlyBackend
	closeCalls int
}

func newSessionTestBackend() *sessionTestBackend {
	return &sessionTestBackend{transportOnlyBackend: transportOnlyBackend{source: SourceFile{
		Path:   "/project/source.ts",
		Source: []byte("export const value = 1\n"),
	}}}
}

func (b *sessionTestBackend) Close() error {
	b.closeCalls++
	return nil
}

func lifecycleRequest(id uint64, operation LifecycleOperation, generation uint64) LifecycleRequest {
	return LifecycleRequest{
		Schema: TypeFactsSchemaVersionV5, RequestID: id,
		Operation: operation, ProjectID: "/project/tsconfig.json", Generation: generation,
	}
}

func BenchmarkSessionDemandPreparation(b *testing.B) {
	const paths = 1_000
	initial := make([]EntityDemand, 0, paths)
	for index := range paths {
		path := fmt.Sprintf("/project/file-%04d.ts", index)
		initial = append(initial, EntityDemand{
			Location: Location{Path: path, StartByte: 1, EndByte: 2},
			Symbol:   true,
		})
	}
	var retained retainedDemandStore
	initialTransaction := retained.begin(initial, nil, true)
	initialTransaction.commit()
	change := []EntityDemand{{
		Location: Location{Path: "/project/file-0500.ts", StartByte: 2, EndByte: 3},
		Symbol:   true,
	}}
	b.ReportAllocs()
	for b.Loop() {
		transaction := retained.begin(change, nil, false)
		if len(transaction.groups()) != paths {
			b.Fatalf("groups = %d, want %d", len(transaction.groups()), paths)
		}
		transaction.commit()
	}
}

func TestSessionOwnsRetainedLifecycleState(t *testing.T) {
	t.Parallel()
	backend := newSessionTestBackend()
	session, err := NewSession(backend, "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	open := session.Lifecycle(context.Background(), lifecycleRequest(1, LifecycleOpen, 1))
	if !open.OK || open.Generation != 1 {
		t.Fatalf("open response = %+v", open)
	}

	firstRequest := lifecycleRequest(2, LifecycleAnalyze, 1)
	firstRequest.ResetState = true
	first := session.Lifecycle(context.Background(), firstRequest)
	if !first.OK || first.StateToken != "1" || len(first.TableTransition) == 0 {
		t.Fatalf("initial analyze response = %+v", first)
	}
	reuseRequest := lifecycleRequest(3, LifecycleAnalyze, 1)
	reuseRequest.StateToken = first.StateToken
	reuse := session.Lifecycle(context.Background(), reuseRequest)
	if !reuse.OK || len(reuse.TableTransition) != 0 || reuse.StateToken != first.StateToken {
		t.Fatalf("warm analyze response = %+v", reuse)
	}

	staleRequest := lifecycleRequest(4, LifecycleAnalyze, 1)
	staleRequest.StateToken = "stale"
	stale := session.Lifecycle(context.Background(), staleRequest)
	if stale.Error == nil || stale.Error.Code != "state-mismatch" {
		t.Fatalf("stale token response = %+v", stale)
	}

	// An accepted no-op update still advances exactly one protocol generation.
	update := session.Lifecycle(context.Background(), lifecycleRequest(5, LifecycleUpdate, 2))
	if !update.OK || update.Generation != 2 {
		t.Fatalf("no-op update response = %+v", update)
	}
	nextRequest := lifecycleRequest(6, LifecycleAnalyze, 2)
	nextRequest.StateToken = first.StateToken
	next := session.Lifecycle(context.Background(), nextRequest)
	if !next.OK || next.StateToken != "2" || len(next.TableTransition) == 0 {
		t.Fatalf("post-update analyze response = %+v", next)
	}
	// The frame's expansion and application are the Rust side's obligation,
	// pinned by the delta golden; here the producer's promise is the mode,
	// the token, and a non-empty frame for the advanced generation.
	if next.Generation != 2 {
		t.Fatalf("delta response generation = %d, want 2", next.Generation)
	}
}

func TestV6TransfersExpandedRowsAndKeepsSparseIncrementalProof(t *testing.T) {
	t.Parallel()
	projectID := "/project/tsconfig.json"
	demand := EntityDemand{
		Location: Location{Path: "/project/source.ts", StartByte: 7, EndByte: 12},
		Symbol:   true, References: true,
	}
	v5, err := NewSession(newSessionTestBackend(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer v5.Close()
	v6, err := NewSessionV6(newSessionTestBackend(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer v6.Close()

	analyze := func(session *Session, schema uint64) LifecycleResponse {
		request := LifecycleRequest{
			Schema: schema, RequestID: 1, Operation: LifecycleAnalyze,
			ProjectID: projectID, Generation: 1, ResetState: true,
			Demands: []EntityDemand{demand},
		}
		return session.Lifecycle(context.Background(), request)
	}
	v5Cold := analyze(v5, TypeFactsSchemaVersionV5)
	v6Cold := analyze(v6, TypeFactsSchemaVersionV6)
	if !v5Cold.OK || !v6Cold.OK {
		t.Fatalf("cold responses: v5=%+v v6=%+v", v5Cold.Error, v6Cold.Error)
	}
	if len(v6Cold.TableTransition) >= len(v5Cold.TableTransition) {
		t.Fatal("v6 cold transition still contains Go-materialized symbol rows")
	}
	contribution := v6.closure.retained.get("/project/source.ts")
	if contribution == nil || contribution.entities != nil || len(contribution.roots) == 0 {
		t.Fatalf("v6 retained contribution did not transfer rows: %+v", contribution)
	}
	if len(v6.closure.table.Entities) != 0 || len(v6.closure.table.Files) != 0 ||
		len(v6.closure.table.sourceDigests) != 0 {
		t.Fatal("v6 retained an expanded transport table after publication")
	}

	update := LifecycleRequest{
		Schema: TypeFactsSchemaVersionV6, RequestID: 2, Operation: LifecycleUpdate,
		ProjectID: projectID, Generation: 2,
	}
	if response := v6.Lifecycle(context.Background(), update); !response.OK {
		t.Fatalf("v6 update: %+v", response.Error)
	}
	next := LifecycleRequest{
		Schema: TypeFactsSchemaVersionV6, RequestID: 3, Operation: LifecycleAnalyze,
		ProjectID: projectID, Generation: 2, StateToken: v6Cold.StateToken,
	}
	response := v6.Lifecycle(context.Background(), next)
	if !response.OK || len(response.TableTransition) == 0 {
		t.Fatalf("v6 sparse successor: %+v", response.Error)
	}
	if contribution := v6.closure.retained.get("/project/source.ts"); contribution == nil ||
		contribution.entities != nil {
		t.Fatal("v6 sparse successor reacquired expanded retained rows")
	}
}

func TestRuntimeValueDomainIsAvailableOnlyInV7(t *testing.T) {
	projectID := "/project/tsconfig.json"
	demand := EntityDemand{
		Location:           Location{Path: "/project/source.ts", StartByte: 7, EndByte: 12},
		RuntimeValueDomain: true,
	}
	for _, open := range []struct {
		name   string
		schema uint64
		new    func(Project, string, Trace) (*Session, error)
	}{
		{"v5", TypeFactsSchemaVersionV5, NewSession},
		{"v6", TypeFactsSchemaVersionV6, NewSessionV6},
	} {
		t.Run(open.name, func(t *testing.T) {
			session, err := open.new(newSessionTestBackend(), projectID, nil)
			if err != nil {
				t.Fatal(err)
			}
			defer session.Close()
			response := session.Lifecycle(context.Background(), LifecycleRequest{
				Schema: open.schema, RequestID: 1, Operation: LifecycleAnalyze,
				ProjectID: projectID, Generation: 1, ResetState: true,
				Demands: []EntityDemand{demand},
			})
			if response.OK || response.Error == nil || response.Error.Code != "invalid-demands" {
				t.Fatalf("frozen schema accepted runtime value domain: %+v", response)
			}
			compact := CompactDemandsV3From([]EntityDemand{demand})
			response = session.Lifecycle(context.Background(), LifecycleRequest{
				Schema: open.schema, RequestID: 2, Operation: LifecycleAnalyze,
				ProjectID: projectID, Generation: 1, ResetState: true,
				CompactDemands: &compact,
			})
			if response.OK || response.Error == nil || response.Error.Code != "invalid-demands" {
				t.Fatalf("frozen schema accepted compact runtime value domain: %+v", response)
			}
		})
	}

	v7, err := NewSessionV7(newSessionTestBackend(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer v7.Close()
	response := v7.Lifecycle(context.Background(), LifecycleRequest{
		Schema: TypeFactsSchemaVersionV7, RequestID: 1, Operation: LifecycleAnalyze,
		ProjectID: projectID, Generation: 1, ResetState: true,
		Demands: []EntityDemand{demand},
	})
	if !response.OK {
		t.Fatalf("v7 rejected runtime value domain: %+v", response.Error)
	}
	if got := decodeTransitionEnvelopeForTest(t, response.TableTransition).schema; got != TypeFactsTableSchemaVersionV4 {
		t.Fatalf("v7 Wire table schema = %d, want %d", got, TypeFactsTableSchemaVersionV4)
	}

	v8, err := NewSessionV8(newSessionTestBackend(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer v8.Close()
	response = v8.Lifecycle(context.Background(), LifecycleRequest{
		Schema: TypeFactsSchemaVersionV8, RequestID: 1, Operation: LifecycleAnalyze,
		ProjectID: projectID, Generation: 1, ResetState: true,
		Demands: []EntityDemand{demand},
	})
	if !response.OK {
		t.Fatalf("v8 rejected runtime value domain: %+v", response.Error)
	}
	if got := decodeTransitionEnvelopeForTest(t, response.TableTransition).schema; got != TypeFactsTableSchemaVersionV5 {
		t.Fatalf("v8 Wire table schema = %d, want %d", got, TypeFactsTableSchemaVersionV5)
	}

	v9, err := NewSessionV9(newSessionTestBackend(), projectID, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer v9.Close()
	response = v9.Lifecycle(context.Background(), LifecycleRequest{
		Schema: TypeFactsSchemaVersionV9, RequestID: 1, Operation: LifecycleAnalyze,
		ProjectID: projectID, Generation: 1, ResetState: true,
		Demands: []EntityDemand{demand},
	})
	if !response.OK {
		t.Fatalf("v9 rejected runtime value domain: %+v", response.Error)
	}
	if got := decodeTransitionEnvelopeForTest(t, response.TableTransition).schema; got != TypeFactsTableSchemaVersion {
		t.Fatalf("v9 Wire table schema = %d, want %d", got, TypeFactsTableSchemaVersion)
	}
}

func TestSessionReturnsSourcesThroughOneSharedArena(t *testing.T) {
	t.Parallel()
	session, err := NewSession(newSessionTestBackend(), "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	initial := session.Lifecycle(context.Background(), lifecycleRequest(1, LifecycleSources, 1))
	if !initial.OK || len(initial.Sources) != 1 {
		t.Fatalf("initial sources response = %+v", initial)
	}
	initialArena, err := os.ReadFile(initial.SourceArena)
	if err != nil {
		t.Fatal(err)
	}
	if got := initialArena[:initial.SourceLengths[0]]; string(got) != "export const value = 1\n" {
		t.Fatalf("initial arena source = %q", got)
	}

	updateRequest := lifecycleRequest(2, LifecycleUpdate, 2)
	updateRequest.Changes = []FileChangeV3{{
		Path: "/project/source.ts", Version: 1, Source: []byte("export const value = 2\n"),
	}}
	update := session.Lifecycle(context.Background(), updateRequest)
	if !update.OK {
		t.Fatalf("update response = %+v", update)
	}
	updated := session.Lifecycle(context.Background(), lifecycleRequest(3, LifecycleSources, 2))
	if !updated.OK || len(updated.Sources) != 1 {
		t.Fatalf("updated sources response = %+v", updated)
	}
	updatedArena, err := os.ReadFile(updated.SourceArena)
	if err != nil {
		t.Fatal(err)
	}
	if got := updatedArena[:updated.SourceLengths[0]]; string(got) != "export const value = 1\n" {
		t.Fatalf("updated arena source = %q", got)
	}
}

func TestSessionCancellationDoesNotCommitRetainedState(t *testing.T) {
	t.Parallel()
	session, err := NewSession(newSessionTestBackend(), "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	cancelled, cancel := context.WithCancel(context.Background())
	cancel()
	request := lifecycleRequest(1, LifecycleAnalyze, 1)
	request.ResetState = true
	response := session.Lifecycle(cancelled, request)
	if response.Error == nil || response.Error.Code != "analysis-cancelled" {
		t.Fatalf("cancelled analyze response = %+v", response)
	}

	retry := session.Lifecycle(context.Background(), request)
	if !retry.OK || retry.StateToken != "1" || len(retry.TableTransition) == 0 {
		t.Fatalf("retry response = %+v", retry)
	}
}

func TestSessionOwnsProjectClosure(t *testing.T) {
	t.Parallel()
	backend := newSessionTestBackend()
	session, err := NewSession(backend, "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}

	staleClose := session.Lifecycle(context.Background(), lifecycleRequest(1, LifecycleClose, 2))
	if staleClose.Error == nil || staleClose.Error.Code != "generation-mismatch" {
		t.Fatalf("stale close response = %+v", staleClose)
	}
	if backend.closeCalls != 0 {
		t.Fatalf("stale close closed the backend %d times", backend.closeCalls)
	}

	closeRequest := lifecycleRequest(1, LifecycleClose, 1)
	first := session.Lifecycle(context.Background(), closeRequest)
	second := session.Lifecycle(context.Background(), closeRequest)
	if !first.OK || !second.OK {
		t.Fatalf("close responses = %+v, %+v", first, second)
	}
	if err := session.Close(); err != nil {
		t.Fatal(err)
	}
	if backend.closeCalls != 1 {
		t.Fatalf("backend Close called %d times, want 1", backend.closeCalls)
	}

	analyze := session.Lifecycle(context.Background(), lifecycleRequest(2, LifecycleAnalyze, 1))
	if analyze.Error == nil || analyze.Error.Code != "session-closed" {
		t.Fatalf("analyze after close = %+v, want session-closed", analyze)
	}
}

// twoFileBackend serves two sources so an update to one leaves the other
// retained, which is the only way the retention counters become meaningful.
type twoFileBackend struct {
	transportOnlyBackend
	second SourceFile
}

func (b twoFileBackend) SourceFiles(context.Context) ([]SourceFile, error) {
	return []SourceFile{b.second, b.transportOnlyBackend.source}, nil
}

// TestSessionAnalysisTraversesTheRetainedPath pins that the in-package doubles
// drive the branches the producer actually ships. Before the doubles gained the
// production capability quartet they only satisfied the unscoped surface, so
// every fast unit test exercised a fallback materializer that no release ever
// runs — and the retention machinery this asserts on went untested outside one
// integration test.
func TestSessionAnalysisTraversesTheRetainedPath(t *testing.T) {
	t.Parallel()
	const firstPath = "/project/source.ts"
	const secondPath = "/project/other.ts"
	backend := twoFileBackend{
		transportOnlyBackend: transportOnlyBackend{source: SourceFile{
			Path: firstPath, Source: []byte("export const value = 1\n"),
		}},
		second: SourceFile{Path: secondPath, Source: []byte("export const other = 2\n")},
	}
	session, err := NewSession(backend, "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })

	demands := []EntityDemand{
		{Location: Location{Path: firstPath, StartByte: 13, EndByte: 18}, Symbol: true, References: true},
		{Location: Location{Path: secondPath, StartByte: 13, EndByte: 18}, Symbol: true, References: true},
	}

	cold := lifecycleRequest(1, LifecycleAnalyze, 1)
	cold.ResetState = true
	cold.Demands = demands
	if response := session.Lifecycle(context.Background(), cold); !response.OK || len(response.TableTransition) == 0 {
		t.Fatalf("cold analyze = %+v", response)
	}
	token := "1"

	update := lifecycleRequest(2, LifecycleUpdate, 2)
	update.Changes = []FileChangeV3{{Path: firstPath, Version: 1, Source: []byte("export const value = 11\n")}}
	if response := session.Lifecycle(context.Background(), update); !response.OK {
		t.Fatalf("update = %+v", response)
	}

	warm := lifecycleRequest(3, LifecycleAnalyze, 2)
	warm.StateToken = token
	warmResponse := session.Lifecycle(context.Background(), warm)
	if !warmResponse.OK {
		t.Fatalf("warm analyze = %+v", warmResponse)
	}

	retention := session.closure.Stats().Retention
	if retention.RetainedFiles == 0 {
		t.Fatalf("no file was retained across the update; retention = %+v", retention)
	}
	if retention.CachedSymbolFacts == 0 {
		t.Fatalf("no durable symbol fact was reused; retention = %+v", retention)
	}
	// SharedSymbolChunks is the patch path's signature: only Patch shares
	// chunks with the preceding store. PatchedSymbolRows cannot stand in for
	// it — this edit leaves every declaration span in place, so the
	// recomputed rows are identical and correctly patch nothing.
	if retention.SharedSymbolChunks == 0 {
		t.Fatalf("the canonical symbol store was rebuilt rather than patched, so the exact-delta fast path went untested; retention = %+v", retention)
	}
	if retention.PatchedSymbolRows != 0 {
		t.Fatalf("a span-stable edit patched %d identical rows; retention = %+v", retention.PatchedSymbolRows, retention)
	}
	if len(warmResponse.TableTransition) == 0 {
		t.Fatalf("warm analyze omitted its generation-advancing transition; retention = %+v", retention)
	}
}
