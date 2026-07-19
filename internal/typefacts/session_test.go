package typefacts

import (
	"context"
	"errors"
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
		Schema: TypeFactsSchemaVersionV3, RequestID: id,
		Operation: operation, ProjectID: "/project/tsconfig.json", Generation: generation,
	}
}

func TestSessionOwnsRetainedLifecycleState(t *testing.T) {
	t.Parallel()
	backend := newSessionTestBackend()
	session, err := NewSession(backend, "/project/tsconfig.json", false)
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
	if !first.OK || first.TableMode != TableModeFull || first.StateToken != "1" || first.CompactTable == nil || first.Table != nil {
		t.Fatalf("initial analyze response = %+v", first)
	}
	firstTable, err := first.CompactTable.Expand()
	if err != nil {
		t.Fatal(err)
	}

	reuseRequest := lifecycleRequest(3, LifecycleAnalyze, 1)
	reuseRequest.StateToken = first.StateToken
	reuse := session.Lifecycle(context.Background(), reuseRequest)
	if !reuse.OK || reuse.TableMode != TableModeReuse || reuse.StateToken != first.StateToken {
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
	if !next.OK || next.TableMode != TableModeDelta || next.StateToken != "2" || next.TableDelta == nil {
		t.Fatalf("post-update analyze response = %+v", next)
	}
	applied := ApplyFactTableDeltaV3(firstTable, *next.TableDelta)
	if applied.Generation != 2 {
		t.Fatalf("delta generation = %d, want 2", applied.Generation)
	}
}

func TestSessionCancellationDoesNotCommitRetainedState(t *testing.T) {
	t.Parallel()
	session, err := NewSession(newSessionTestBackend(), "/project/tsconfig.json", false)
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
	if !retry.OK || retry.StateToken != "1" || retry.TableMode != TableModeFull {
		t.Fatalf("retry response = %+v", retry)
	}
}

func TestSessionOwnsProjectClosure(t *testing.T) {
	t.Parallel()
	backend := newSessionTestBackend()
	session, err := NewSession(backend, "/project/tsconfig.json", false)
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

	_, err = session.Closure(context.Background(), ClosureRequest{
		Schema: TypeFactsSchemaVersionV2, ProjectID: "/project/tsconfig.json",
		Generation: 1, RulesetVersion: 1,
	})
	if !errors.Is(err, ErrSessionClosed) {
		t.Fatalf("v2 request after close error = %v, want ErrSessionClosed", err)
	}
}
