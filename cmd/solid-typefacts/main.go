// Command solid-typefacts exposes a retained TypeScript-Go project through
// the frozen TypeFacts v2 length-prefixed deterministic-CBOR protocol.
package main

import (
	"bufio"
	"cmp"
	"context"
	"encoding/binary"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime/pprof"
	"slices"
	"sort"
	"strconv"
	"sync"
	"time"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
	"github.com/yumemi-thomas/solid-check/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-check/internal/wirecbor"
)

var buildID = "dev"

// stageTrace reports a service-side stage duration on stderr when
// SOLID_TYPEFACTS_TIMINGS is set, mirroring the CLI's SOLID_CHECK_TIMINGS.
func stageTrace(stage string, elapsed time.Duration) {
	if os.Getenv("SOLID_TYPEFACTS_TIMINGS") == "" {
		return
	}
	fmt.Fprintf(os.Stderr, "{\"typefactsStage\":%q,\"elapsedNs\":%d}\n", stage, elapsed.Nanoseconds())
}

func main() {
	if err := run(context.Background(), os.Args[1:], os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, "solid-typefacts:", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, args []string, input io.Reader, output io.Writer) error {
	started := time.Now()
	flags := flag.NewFlagSet("solid-typefacts", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	project := flags.String("project", "", "path to tsconfig.json")
	cpuProfile := flags.String("cpuprofile", "", "write a CPU profile to this path")
	if err := flags.Parse(args); err != nil {
		return err
	}
	if *project == "" {
		return errors.New("-project is required")
	}
	if *cpuProfile != "" {
		profile, err := os.Create(*cpuProfile)
		if err != nil {
			return fmt.Errorf("create cpu profile: %w", err)
		}
		defer profile.Close()
		if err := pprof.StartCPUProfile(profile); err != nil {
			return fmt.Errorf("start cpu profile: %w", err)
		}
		defer pprof.StopCPUProfile()
	}
	projectID, err := filepath.Abs(*project)
	if err != nil {
		return fmt.Errorf("resolve project: %w", err)
	}
	projectID = filepath.Clean(projectID)

	// The startup handshake carries only the protocol version, schema hash,
	// and build id — nothing derived from the project — so write and flush it
	// before opening the TypeScript program. A client that blocks on the
	// handshake (the Rust checker's TypeFactsSidecar::spawn) is released as
	// soon as the process is live and can overlap its own cold-start work with
	// the program build below. Early client frames simply queue in the pipe
	// until the reader starts; the ordered worker preserves arrival order.
	writer := bufio.NewWriter(output)
	handshake, err := wirecbor.Marshal(typefacts.ServiceHandshake{
		Protocol:   typefacts.TypeFactsHandshakeProtocol,
		SchemaHash: typefacts.TypeFactsSchemaSHA256,
		BuildID:    buildID,
	})
	if err != nil {
		return fmt.Errorf("encode startup handshake: %w", err)
	}
	if err := writeFrame(writer, handshake); err != nil {
		return fmt.Errorf("write startup handshake: %w", err)
	}
	if err := writer.Flush(); err != nil {
		return fmt.Errorf("flush startup handshake: %w", err)
	}
	stageTrace("handshake-written", time.Since(started))

	backend, err := tsgo.OpenProject(ctx, projectID)
	if err != nil {
		return fmt.Errorf("open TS-Go project: %w", err)
	}
	stageTrace("open", time.Since(started))
	closure, err := typefacts.NewClosureProject(backend, false)
	if err != nil {
		_ = backend.Close()
		return err
	}
	defer closure.Close()

	reader := bufio.NewReader(input)
	responder := &closureResponder{ctx: ctx, closure: closure, projectID: projectID, generation: 1}
	return serve(ctx, responder, reader, writer)
}

// responder answers decoded requests; serve owns framing, arrival-order
// dispatch, and cancellation. A v2 error is fatal (matching the frozen
// protocol's behavior); a v3 request always yields a response frame.
type responder interface {
	v2(ctx context.Context, request typefacts.ClosureRequest) (typefacts.ClosureResponse, error)
	v3(ctx context.Context, request typefacts.LifecycleRequest) typefacts.LifecycleResponse
}

type closureResponder struct {
	ctx        context.Context
	closure    *typefacts.ClosureProject
	projectID  string
	generation uint64
	retained   retainedProtocolState
}

type retainedProtocolState struct {
	token   uint64
	demands map[string][]typefacts.EntityDemand
	table   *typefacts.FactTable
}

func (r *closureResponder) v2(ctx context.Context, request typefacts.ClosureRequest) (typefacts.ClosureResponse, error) {
	if filepath.Clean(request.ProjectID) != r.projectID {
		return typefacts.ClosureResponse{}, typefacts.ErrGenerationMismatch
	}
	value, err := r.closure.ClosureResponseFor(ctx, request)
	if err != nil {
		return typefacts.ClosureResponse{}, fmt.Errorf("materialize closure: %w", err)
	}
	return value, nil
}

func (r *closureResponder) v3(ctx context.Context, request typefacts.LifecycleRequest) typefacts.LifecycleResponse {
	switch request.Operation {
	case typefacts.LifecycleUpdate:
		crashOnMarker("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE")
	case typefacts.LifecycleAnalyze:
		crashOnMarker("SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE")
	}
	return lifecycleResponse(ctx, r.closure, r.projectID, &r.generation, &r.retained, request)
}

// crashOnMarker terminates the service when the named environment variable
// points at an existing marker file, consuming the marker so a restarted
// service runs normally. Test-only fault injection for client crash-recovery
// coverage, following the SOLID_TYPEFACTS_BAD_FRAME precedent.
func crashOnMarker(name string) {
	path := os.Getenv(name)
	if path == "" {
		return
	}
	if err := os.Remove(path); err == nil {
		os.Exit(1)
	}
}

// job is one generation-scoped request awaiting the ordered worker. cancel is
// non-nil for cancellable v3 operations and is released after dispatch.
type job struct {
	v2            *typefacts.ClosureRequest
	v3            *typefacts.LifecycleRequest
	ctx           context.Context
	requestDecode time.Duration
	release       func()
}

// serve reads frames and dispatches them in arrival order through a single
// worker, so a client may pipeline generation-scoped requests (update, then
// analyze of the new generation) without awaiting intermediate responses.
// Cancel frames bypass the queue: the reader fires the target's context
// immediately, and the acknowledgement is ordered like any other response.
// Responses are encoded and written on a dedicated writer goroutine so a
// large table encode never delays the next request's compute.
func serve(ctx context.Context, respond responder, input io.Reader, output *bufio.Writer) error {
	var cancelMu sync.Mutex
	cancels := make(map[uint64]context.CancelFunc)

	jobs := newQueue[job]()
	responses := newQueue[any]()
	fatal := make(chan error, 2)
	var pipeline sync.WaitGroup

	pipeline.Add(1)
	go func() { // worker: computes responses in arrival order
		defer pipeline.Done()
		defer responses.close()
		for {
			next, ok := jobs.pop()
			if !ok {
				return
			}
			if next.v2 != nil {
				value, err := respond.v2(ctx, *next.v2)
				if err != nil {
					fatal <- err
					return
				}
				responses.push(value)
				continue
			}
			value := respond.v3(next.ctx, *next.v3)
			if value.Timings != nil {
				value.Timings.RequestDecodeNs = uint64(next.requestDecode)
			}
			if next.release != nil {
				next.release()
			}
			responses.push(value)
		}
	}()

	pipeline.Add(1)
	go func() { // writer: encodes and flushes responses
		defer pipeline.Done()
		for {
			value, ok := responses.pop()
			if !ok {
				return
			}
			encodeStarted := time.Now()
			encoded, err := wirecbor.Marshal(value)
			if err != nil {
				fatal <- fmt.Errorf("encode response: %w", err)
				return
			}
			stageTrace("encode-response", time.Since(encodeStarted))
			writeStarted := time.Now()
			if err := writeFrame(output, encoded); err != nil {
				fatal <- err
				return
			}
			if err := output.Flush(); err != nil {
				fatal <- err
				return
			}
			stageTrace("write-response", time.Since(writeStarted))
		}
	}()

	type frame struct {
		payload []byte
		err     error
	}
	frames := make(chan frame)
	go func() { // reader: frames must keep arriving while the worker computes
		for {
			payload, err := readFrame(input)
			frames <- frame{payload: payload, err: err}
			if err != nil {
				return
			}
		}
	}()

	drain := func(readErr error) error {
		jobs.close()
		pipeline.Wait()
		select {
		case err := <-fatal:
			return err
		default:
		}
		if readErr != nil {
			return readErr
		}
		return output.Flush()
	}

	for {
		var next frame
		select {
		case err := <-fatal:
			jobs.close()
			pipeline.Wait()
			return err
		case next = <-frames:
		}
		if errors.Is(next.err, io.EOF) {
			return drain(nil)
		}
		if next.err != nil {
			return drain(next.err)
		}
		payload := next.payload
		decodeStarted := time.Now()
		var envelope map[string]any
		if err := wirecbor.Unmarshal(payload, &envelope); err != nil {
			return drain(fmt.Errorf("decode request envelope: %w", err))
		}
		schema, _ := envelope["schema"].(uint64)
		switch schema {
		case typefacts.TypeFactsSchemaVersionV2:
			var request typefacts.ClosureRequest
			if err := wirecbor.Unmarshal(payload, &request); err != nil {
				return drain(fmt.Errorf("decode v2 request: %w", err))
			}
			jobs.push(job{v2: &request, ctx: ctx})
		case typefacts.TypeFactsSchemaVersionV3:
			var request typefacts.LifecycleRequest
			if err := wirecbor.Unmarshal(payload, &request); err != nil {
				return drain(fmt.Errorf("decode v3 request: %w", err))
			}
			if request.Operation == typefacts.LifecycleCancel {
				cancelMu.Lock()
				cancel := cancels[request.CancelRequestID]
				cancelMu.Unlock()
				if cancel != nil {
					cancel()
				}
				jobs.push(job{v3: &request, ctx: ctx})
				continue
			}
			requestCtx, cancel := context.WithCancel(ctx)
			cancelMu.Lock()
			cancels[request.RequestID] = cancel
			cancelMu.Unlock()
			requestID := request.RequestID
			jobs.push(job{v3: &request, ctx: requestCtx, requestDecode: time.Since(decodeStarted), release: func() {
				cancel()
				cancelMu.Lock()
				delete(cancels, requestID)
				cancelMu.Unlock()
			}})
		default:
			return drain(fmt.Errorf("unsupported TypeFacts schema %d", schema))
		}
	}
}

// queue is an unbounded FIFO. The reader must never block on enqueue — a full
// bounded queue would stop it from reading cancel frames.
type queue[T any] struct {
	mu     sync.Mutex
	cond   *sync.Cond
	items  []T
	closed bool
}

func newQueue[T any]() *queue[T] {
	q := &queue[T]{}
	q.cond = sync.NewCond(&q.mu)
	return q
}

func (q *queue[T]) push(item T) {
	q.mu.Lock()
	defer q.mu.Unlock()
	if q.closed {
		return
	}
	q.items = append(q.items, item)
	q.cond.Signal()
}

func (q *queue[T]) pop() (T, bool) {
	q.mu.Lock()
	defer q.mu.Unlock()
	for len(q.items) == 0 && !q.closed {
		q.cond.Wait()
	}
	var zero T
	if len(q.items) == 0 {
		return zero, false
	}
	item := q.items[0]
	q.items[0] = zero
	q.items = q.items[1:]
	return item, true
}

func (q *queue[T]) close() {
	q.mu.Lock()
	defer q.mu.Unlock()
	q.closed = true
	q.cond.Broadcast()
}

func lifecycleResponse(ctx context.Context, closure *typefacts.ClosureProject, projectID string, generation *uint64, retained *retainedProtocolState, request typefacts.LifecycleRequest) typefacts.LifecycleResponse {
	response := typefacts.LifecycleResponse{
		Schema: typefacts.TypeFactsSchemaVersionV3, RequestID: request.RequestID,
		ProjectID: projectID, Generation: *generation,
	}
	fail := func(code string, err error) typefacts.LifecycleResponse {
		response.Error = &typefacts.LifecycleError{Code: code, Message: err.Error()}
		return response
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		return fail("invalid-request", err)
	}
	if filepath.Clean(request.ProjectID) != projectID {
		return fail("project-mismatch", typefacts.ErrGenerationMismatch)
	}
	switch request.Operation {
	case typefacts.LifecycleOpen:
		if request.Generation != *generation {
			return fail("generation-mismatch", typefacts.ErrGenerationMismatch)
		}
	case typefacts.LifecycleUpdate:
		if request.Generation != *generation+1 {
			return fail("generation-mismatch", typefacts.ErrGenerationMismatch)
		}
		updateStarted := time.Now()
		changes := make([]typefacts.FileChange, 0, len(request.Changes))
		for _, change := range request.Changes {
			changes = append(changes, typefacts.FileChange{
				Path: change.Path, Version: change.Version, Source: change.Source, Deleted: change.Deleted,
			})
		}
		affected, err := closure.Update(ctx, changes)
		if err != nil {
			return fail("update-failed", err)
		}
		stageTrace("update", time.Since(updateStarted))
		*generation = request.Generation
		response.Generation = *generation
		response.Affected = affected.Files
	case typefacts.LifecycleAnalyze:
		if request.Generation != *generation {
			return fail("generation-mismatch", typefacts.ErrGenerationMismatch)
		}
		stateful := request.ResetState || request.StateToken != "" || len(request.RemovedDemandPaths) != 0
		nextDemands := retained.demands
		if stateful {
			expected := ""
			if retained.token != 0 {
				expected = strconv.FormatUint(retained.token, 10)
			}
			if !request.ResetState && request.StateToken != expected {
				return fail("state-mismatch", typefacts.ErrGenerationMismatch)
			}
			nextDemands = applyDemandChanges(retained.demands, request.Demands, request.RemovedDemandPaths, request.ResetState)
			if !request.ResetState &&
				len(request.Demands) == 0 &&
				len(request.RemovedDemandPaths) == 0 &&
				retained.table != nil &&
				retained.table.Generation == *generation {
				response.TableMode = typefacts.TableModeReuse
				response.StateToken = expected
				response.Timings = &typefacts.LifecycleTimings{}
				response.OK = true
				return response
			}
		}
		analyzeStarted := time.Now()
		buildSequence := closure.Stats().BuildSequence
		var analyzed typefacts.ClosureResponse
		var analyzedTable *typefacts.FactTable
		var err error
		if stateful {
			analyzedTable, err = closure.DemandTableForGroups(
				ctx,
				*generation,
				demandGroups(nextDemands),
				demandChangedPaths(request.Demands, request.RemovedDemandPaths),
			)
		} else if len(request.Demands) != 0 {
			analyzed, err = closure.DemandResponseFor(ctx, projectID, *generation, request.Demands)
		} else {
			spans := append(append([]typefacts.LocationV2(nil), request.StructuralSpans...), request.CompilerSpans...)
			slices.SortFunc(spans, func(a, b typefacts.LocationV2) int {
				return cmp.Or(cmp.Compare(a.Path, b.Path), cmp.Compare(a.StartByte, b.StartByte), cmp.Compare(a.EndByte, b.EndByte))
			})
			v2 := typefacts.ClosureRequest{Schema: 2, ProjectID: projectID, Generation: *generation, RulesetVersion: 1, CompilerSpans: spans}
			analyzed, err = closure.ClosureResponseFor(ctx, v2)
		}
		if err != nil {
			return fail("analysis-failed", err)
		}
		if err := ctx.Err(); err != nil {
			return fail("analysis-cancelled", err)
		}
		stageTrace("analyze", time.Since(analyzeStarted))
		stats := closure.Stats()
		materialized := stats.BuildSequence != buildSequence
		response.Timings = &typefacts.LifecycleTimings{
			AnalyzeNs:    uint64(time.Since(analyzeStarted)),
			Materialized: materialized,
		}
		if materialized {
			response.Timings.AsyncNs = uint64(stats.AsyncDuration)
			response.Timings.DemandNs = uint64(stats.DemandDuration)
			response.Timings.AssemblyNs = uint64(stats.AssemblyDuration)
			response.Timings.SortNs = uint64(stats.SortDuration)
			response.Timings.CloseSymbolsNs = uint64(stats.CloseDuration)
			response.Timings.PrepareNs = uint64(stats.PrepareDuration)
			response.Timings.RetainedFiles = uint64(stats.Retention.RetainedFiles)
			response.Timings.RecomputedFiles = uint64(stats.Retention.RecomputedFiles)
			response.Timings.NonDurableFiles = uint64(stats.Retention.NonDurableFiles)
		}
		stageTrace("analyze-materialize", stats.BuildDuration)
		stageTrace("analyze-async", stats.AsyncDuration)
		stageTrace("analyze-demand", stats.DemandDuration)
		stageTrace("analyze-symbols", stats.SymbolDuration)
		if os.Getenv("SOLID_TYPEFACTS_TIMINGS") != "" {
			fmt.Fprintf(os.Stderr, "{\"typefactsRetention\":{\"retained\":%d,\"recomputed\":%d,\"suppressionRecompute\":%t}}\n",
				stats.Retention.RetainedFiles, stats.Retention.RecomputedFiles, stats.Retention.SuppressionRecompute)
		}
		if stateful {
			nextToken := retained.token + 1
			response.StateToken = strconv.FormatUint(nextToken, 10)
			if request.ResetState || retained.table == nil || stats.Retention.NonDurableFiles != 0 {
				response.TableMode = typefacts.TableModeFull
				table := typefacts.FactTableV2From(*analyzedTable, projectID, *generation)
				response.Table = &table
			} else {
				delta := typefacts.DiffFactTablesV3FromInternal(*retained.table, *analyzedTable, *generation)
				if retained.table.Generation == analyzedTable.Generation && delta.Empty() {
					response.TableMode = typefacts.TableModeReuse
				} else {
					response.TableMode = typefacts.TableModeDelta
					response.TableDelta = &delta
				}
			}
			retained.token = nextToken
			retained.demands = nextDemands
			table := *analyzedTable
			retained.table = &table
		} else {
			response.Table = &analyzed.Table
		}
	case typefacts.LifecycleSources:
		if request.Generation != *generation {
			return fail("generation-mismatch", typefacts.ErrGenerationMismatch)
		}
		sources, err := closure.SourceFiles(ctx)
		if err != nil {
			return fail("sources-failed", err)
		}
		response.Sources = make([]typefacts.SourceFileV3, 0, len(sources))
		for _, source := range sources {
			response.Sources = append(response.Sources, typefacts.SourceFileV3{
				Path: source.Path, Source: source.Source,
			})
		}
	case typefacts.LifecycleCancel:
		// Requests are currently processed serially; cancellation is
		// acknowledged for an already completed or not-yet-started request.
	case typefacts.LifecycleClose:
		response.OK = true
		return response
	}
	response.OK = true
	return response
}

func applyDemandChanges(previous map[string][]typefacts.EntityDemand, changes []typefacts.EntityDemand, removed []string, reset bool) map[string][]typefacts.EntityDemand {
	next := make(map[string][]typefacts.EntityDemand)
	if !reset {
		for path, demands := range previous {
			next[path] = demands
		}
	}
	changed := make(map[string][]typefacts.EntityDemand)
	for _, demand := range changes {
		path := filepath.Clean(demand.Location.Path)
		changed[path] = append(changed[path], demand)
	}
	for path, demands := range changed {
		next[path] = demands
	}
	for _, path := range removed {
		delete(next, filepath.Clean(path))
	}
	return next
}

func demandGroups(grouped map[string][]typefacts.EntityDemand) []typefacts.DemandGroup {
	paths := make([]string, 0, len(grouped))
	for path := range grouped {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	result := make([]typefacts.DemandGroup, 0, len(paths))
	for _, path := range paths {
		result = append(result, typefacts.DemandGroup{
			Path:    path,
			Demands: grouped[path],
		})
	}
	return result
}

func demandChangedPaths(changes []typefacts.EntityDemand, removed []string) []string {
	changed := make(map[string]struct{}, len(removed)+1)
	for _, demand := range changes {
		changed[filepath.Clean(demand.Location.Path)] = struct{}{}
	}
	for _, path := range removed {
		changed[filepath.Clean(path)] = struct{}{}
	}
	paths := make([]string, 0, len(changed))
	for path := range changed {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	return paths
}

func readFrame(reader io.Reader) ([]byte, error) {
	var prefix [4]byte
	if _, err := io.ReadFull(reader, prefix[:]); err != nil {
		return nil, err
	}
	size := binary.LittleEndian.Uint32(prefix[:])
	if size > wirecbor.MaxMessageBytes {
		return nil, fmt.Errorf("request is %d bytes, limit is %d", size, wirecbor.MaxMessageBytes)
	}
	payload := make([]byte, size)
	if _, err := io.ReadFull(reader, payload); err != nil {
		return nil, err
	}
	return payload, nil
}

func writeFrame(writer io.Writer, payload []byte) error {
	if len(payload) > wirecbor.MaxMessageBytes {
		return fmt.Errorf("response is %d bytes, limit is %d", len(payload), wirecbor.MaxMessageBytes)
	}
	var prefix [4]byte
	binary.LittleEndian.PutUint32(prefix[:], uint32(len(payload)))
	if _, err := writer.Write(prefix[:]); err != nil {
		return err
	}
	_, err := writer.Write(payload)
	return err
}
