package main

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"sync"
	"testing"
	"time"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
	"github.com/yumemi-thomas/solid-check/internal/wirecbor"
)

// orderedResponder mimics the generation discipline of the real service: an
// update must arrive at generation+1 and an analyze at the current
// generation. Its update is deliberately slow so a pipelined analyze reaches
// the dispatcher while the update is still computing — the exact interleaving
// the arrival-order contract must survive.
type orderedResponder struct {
	generation  uint64
	updateDelay time.Duration
	analyzeWait time.Duration
}

func (r *orderedResponder) v2(context.Context, typefacts.ClosureRequest) (typefacts.ClosureResponse, error) {
	return typefacts.ClosureResponse{}, fmt.Errorf("v2 is not exercised by this test")
}

func (r *orderedResponder) v3(ctx context.Context, request typefacts.LifecycleRequest) typefacts.LifecycleResponse {
	response := typefacts.LifecycleResponse{
		Schema: typefacts.TypeFactsSchemaVersionV3, RequestID: request.RequestID,
		ProjectID: request.ProjectID, Generation: r.generation,
	}
	fail := func(code string) typefacts.LifecycleResponse {
		response.Error = &typefacts.LifecycleError{Code: code, Message: code}
		return response
	}
	switch request.Operation {
	case typefacts.LifecycleUpdate:
		if request.Generation != r.generation+1 {
			return fail("generation-mismatch")
		}
		time.Sleep(r.updateDelay)
		r.generation = request.Generation
		response.Generation = r.generation
	case typefacts.LifecycleAnalyze:
		if request.Generation != r.generation {
			return fail("generation-mismatch")
		}
		if r.analyzeWait > 0 {
			select {
			case <-ctx.Done():
				return fail("cancelled")
			case <-time.After(r.analyzeWait):
			}
		}
	case typefacts.LifecycleCancel:
	default:
		return fail("unsupported-operation")
	}
	response.OK = true
	return response
}

type servedResponder struct {
	requests  io.WriteCloser
	responses *bufio.Reader
	done      chan error
}

func startServe(t *testing.T, respond responder) *servedResponder {
	t.Helper()
	requestReader, requestWriter := io.Pipe()
	responseReader, responseWriter := io.Pipe()
	done := make(chan error, 1)
	go func() {
		writer := bufio.NewWriter(responseWriter)
		done <- serve(context.Background(), respond, requestReader, writer)
		responseWriter.Close()
	}()
	return &servedResponder{
		requests:  requestWriter,
		responses: bufio.NewReader(responseReader),
		done:      done,
	}
}

func (s *servedResponder) send(t *testing.T, request typefacts.LifecycleRequest) {
	t.Helper()
	encoded, err := wirecbor.Marshal(request)
	if err != nil {
		t.Fatalf("encode request %d: %v", request.RequestID, err)
	}
	if err := writeFrame(s.requests, encoded); err != nil {
		t.Fatalf("write request %d: %v", request.RequestID, err)
	}
}

func (s *servedResponder) receive(t *testing.T) typefacts.LifecycleResponse {
	t.Helper()
	payload, err := readFrame(s.responses)
	if err != nil {
		t.Fatalf("read response: %v", err)
	}
	var response typefacts.LifecycleResponse
	if err := wirecbor.Unmarshal(payload, &response); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	return response
}

func lifecycleRequestV3(id uint64, operation typefacts.LifecycleOperation, generation uint64) typefacts.LifecycleRequest {
	return typefacts.LifecycleRequest{
		Schema: typefacts.TypeFactsSchemaVersionV3, RequestID: id,
		Operation: operation, ProjectID: "/project/tsconfig.json", Generation: generation,
	}
}

// TestPipelinedUpdateAnalyzeAreDispatchedInArrivalOrder floods the service
// with update+analyze pairs written back-to-back, never awaiting the update
// response first. The protocol documents that generation-changing work is
// ordered; before the single ordered worker, each request raced for a mutex
// from its own goroutine and a pipelined analyze could overtake its update.
func TestPipelinedUpdateAnalyzeAreDispatchedInArrivalOrder(t *testing.T) {
	t.Parallel()
	respond := &orderedResponder{generation: 1, updateDelay: 2 * time.Millisecond}
	served := startServe(t, respond)

	const generations = 25
	var writers sync.WaitGroup
	writers.Add(1)
	go func() {
		defer writers.Done()
		id := uint64(1)
		for generation := uint64(2); generation < 2+generations; generation++ {
			served.send(t, lifecycleRequestV3(id, typefacts.LifecycleUpdate, generation))
			id++
			served.send(t, lifecycleRequestV3(id, typefacts.LifecycleAnalyze, generation))
			id++
		}
	}()

	for received := 0; received < 2*generations; received++ {
		response := served.receive(t)
		if response.Error != nil {
			t.Fatalf("response %d failed: %s (%s)", response.RequestID, response.Error.Code, response.Error.Message)
		}
	}
	writers.Wait()
	served.requests.Close()
	if err := <-served.done; err != nil {
		t.Fatalf("serve: %v", err)
	}
}

// TestCancelBypassesTheOrderedQueue proves the reader stays available to
// cancel while the worker is busy: an analyze that would block for ten
// seconds returns promptly once the cancel frame fires its context, and the
// cancel acknowledgement is ordered after the cancelled response.
func TestCancelBypassesTheOrderedQueue(t *testing.T) {
	t.Parallel()
	respond := &orderedResponder{generation: 1, analyzeWait: 10 * time.Second}
	served := startServe(t, respond)

	started := time.Now()
	served.send(t, lifecycleRequestV3(7, typefacts.LifecycleAnalyze, 1))
	cancel := lifecycleRequestV3(8, typefacts.LifecycleCancel, 1)
	cancel.CancelRequestID = 7
	served.send(t, cancel)

	analyze := served.receive(t)
	if analyze.RequestID != 7 {
		t.Fatalf("expected the analyze response first, got request %d", analyze.RequestID)
	}
	if analyze.Error == nil || analyze.Error.Code != "cancelled" {
		t.Fatalf("expected the analyze to be cancelled, got %+v", analyze)
	}
	if elapsed := time.Since(started); elapsed > 5*time.Second {
		t.Fatalf("cancellation took %s; the reader did not bypass the busy worker", elapsed)
	}
	acknowledgement := served.receive(t)
	if acknowledgement.RequestID != 8 || !acknowledgement.OK {
		t.Fatalf("expected the cancel acknowledgement, got %+v", acknowledgement)
	}
	served.requests.Close()
	if err := <-served.done; err != nil {
		t.Fatalf("serve: %v", err)
	}
}
