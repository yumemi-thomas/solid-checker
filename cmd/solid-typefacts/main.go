// Command solid-typefacts exposes a retained TypeScript-Go project through
// the TypeFacts v3 length-prefixed deterministic-CBOR lifecycle protocol.
package main

import (
	"bufio"
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime/debug"
	"runtime/pprof"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-ts-facts/internal/wirecbor"
)

var buildID = "dev"

// stderrTrace is the only adapter at the typefacts.Trace seam, and the only
// place in the producer that reads the environment or writes to stderr. It is
// installed once at startup, or not at all: a nil Trace means every gated
// payload computation below the seam is skipped rather than computed and
// discarded.
type stderrTrace struct{}

// newTrace resolves the tracing decision once, so no hot path repeats an
// environment lookup per analysis.
func newTrace() typefacts.Trace {
	if os.Getenv("SOLID_TYPEFACTS_TIMINGS") == "" {
		return nil
	}
	return stderrTrace{}
}

func (stderrTrace) Stage(name string, elapsed time.Duration) {
	fmt.Fprintf(os.Stderr, "{\"typefactsStage\":%q,\"elapsedNs\":%d}\n", name, elapsed.Nanoseconds())
}

func (stderrTrace) Metrics(name string, values ...typefacts.Metric) {
	var builder strings.Builder
	fmt.Fprintf(&builder, "{%q:{", "typefacts:"+name)
	for index, value := range values {
		if index > 0 {
			builder.WriteByte(',')
		}
		fmt.Fprintf(&builder, "%q:%d", value.Key, value.Value)
	}
	builder.WriteString("}}\n")
	_, _ = os.Stderr.WriteString(builder.String())
}

func main() {
	if err := run(context.Background(), os.Args[1:], os.Stdin, os.Stdout); err != nil {
		fmt.Fprintln(os.Stderr, "solid-typefacts:", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, args []string, input io.Reader, output io.Writer) error {
	started := time.Now()
	trace := newTrace()
	flags := flag.NewFlagSet("solid-typefacts", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	project := flags.String("project", "", "path to tsconfig.json")
	schema := flags.Uint64("schema", typefacts.TypeFactsSchemaVersionV5, "lifecycle schema version")
	cpuProfile := flags.String("cpuprofile", "", "write a CPU profile to this path")
	transitionArenaPath := flags.String("transition-arena", "", "Rust-owned transition arena path")
	if err := flags.Parse(args); err != nil {
		return err
	}
	if *project == "" {
		return errors.New("-project is required")
	}
	if *schema != typefacts.TypeFactsSchemaVersionV5 && *schema != typefacts.TypeFactsSchemaVersionV6 && *schema != typefacts.TypeFactsSchemaVersionV7 && *schema != typefacts.TypeFactsSchemaVersionV8 && *schema != typefacts.TypeFactsSchemaVersionV9 {
		return fmt.Errorf("unsupported TypeFacts schema %d", *schema)
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
	transitionArena, err := openTransitionFileArena(*transitionArenaPath)
	if err != nil {
		return err
	}
	if transitionArena != nil {
		defer transitionArena.Close()
	}

	// The startup handshake carries only the protocol version, schema hash,
	// and build id — nothing derived from the project — so write and flush it
	// before opening the TypeScript program. A client that blocks on the
	// handshake (the Rust checker's TypeFactsSidecar::spawn) is released as
	// soon as the process is live and can overlap its own cold-start work with
	// the program build below. Early client frames simply queue in the pipe
	// until the reader starts; the ordered worker preserves arrival order.
	writer := bufio.NewWriter(output)
	schemaHash := typefacts.TypeFactsSchemaSHA256
	if *schema == typefacts.TypeFactsSchemaVersionV6 {
		schemaHash = typefacts.TypeFactsSchemaV6SHA256
	} else if *schema == typefacts.TypeFactsSchemaVersionV7 {
		schemaHash = typefacts.TypeFactsSchemaV7SHA256
	} else if *schema == typefacts.TypeFactsSchemaVersionV8 {
		schemaHash = typefacts.TypeFactsSchemaV8SHA256
	} else if *schema == typefacts.TypeFactsSchemaVersionV9 {
		schemaHash = typefacts.TypeFactsSchemaV9SHA256
	}
	handshake, err := wirecbor.Marshal(typefacts.ServiceHandshake{
		Protocol:   typefacts.TypeFactsHandshakeProtocol,
		SchemaHash: schemaHash,
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
	if trace != nil {
		trace.Stage("handshake-written", time.Since(started))
	}

	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		return fmt.Errorf("open TS-Go project: %w", err)
	}
	if trace != nil {
		trace.Stage("open", time.Since(started))
	}
	var session *typefacts.Session
	if *schema == typefacts.TypeFactsSchemaVersionV9 {
		session, err = typefacts.NewSessionV9(backend, projectID, trace)
	} else if *schema == typefacts.TypeFactsSchemaVersionV8 {
		session, err = typefacts.NewSessionV8(backend, projectID, trace)
	} else if *schema == typefacts.TypeFactsSchemaVersionV7 {
		session, err = typefacts.NewSessionV7(backend, projectID, trace)
	} else if *schema == typefacts.TypeFactsSchemaVersionV6 {
		session, err = typefacts.NewSessionV6(backend, projectID, trace)
	} else {
		session, err = typefacts.NewSession(backend, projectID, trace)
	}
	if err != nil {
		return err
	}
	defer session.Close()

	reader := bufio.NewReader(input)
	responder := &lifecycleResponder{
		session:            session,
		trace:              trace,
		transitionArena:    transitionArena,
		crashBeforeUpdate:  os.Getenv("SOLID_TYPEFACTS_CRASH_BEFORE_UPDATE"),
		crashBeforeAnalyze: os.Getenv("SOLID_TYPEFACTS_CRASH_BEFORE_ANALYZE"),
	}
	return serve(ctx, responder, reader, writer, trace)
}

// responder answers decoded requests; serve owns framing, arrival-order
// dispatch, and cancellation. Every request yields exactly one response frame.
type responder interface {
	lifecycle(ctx context.Context, request typefacts.LifecycleRequest) typefacts.LifecycleResponse
}

type lifecycleResponder struct {
	session         *typefacts.Session
	trace           typefacts.Trace
	transitionArena *transitionFileArena
	// Marker paths for test-only fault injection, resolved once at startup so
	// no environment lookup sits on the per-request path.
	crashBeforeUpdate  string
	crashBeforeAnalyze string
}

func (r *lifecycleResponder) lifecycle(ctx context.Context, request typefacts.LifecycleRequest) typefacts.LifecycleResponse {
	switch request.Operation {
	case typefacts.LifecycleUpdate:
		crashOnMarker(r.crashBeforeUpdate)
	case typefacts.LifecycleAnalyze:
		crashOnMarker(r.crashBeforeAnalyze)
	}
	started := time.Now()
	var response typefacts.LifecycleResponse
	if r.transitionArena != nil &&
		(request.Operation == typefacts.LifecycleAnalyze ||
			request.Operation == typefacts.LifecycleSymbols) {
		if err := r.transitionArena.Begin(request.RequestID); err == nil {
			response = r.session.LifecycleInto(ctx, request, r.transitionArena)
			if path, descriptor, ok := r.transitionArena.descriptor(); ok && response.OK {
				response.SourceArena = path
				response.SourceLengths = descriptor
			}
		} else {
			response = r.session.Lifecycle(ctx, request)
		}
	} else {
		response = r.session.Lifecycle(ctx, request)
	}
	if r.trace == nil {
		return response
	}
	switch {
	case request.Operation == typefacts.LifecycleUpdate:
		r.trace.Stage("update", time.Since(started))
	case response.Timings != nil:
		// The closure reports its own stage breakdown through the seam; the
		// request-level duration is the only number the adapter owns.
		r.trace.Stage("analyze", time.Duration(response.Timings.AnalyzeNs))
	}
	return response
}

// crashOnMarker terminates the service when path names an existing marker file,
// consuming the marker so a restarted service runs normally. Test-only fault
// injection for client crash-recovery coverage, following the
// SOLID_TYPEFACTS_BAD_FRAME precedent.
func crashOnMarker(path string) {
	if path == "" {
		return
	}
	if err := os.Remove(path); err == nil {
		os.Exit(1)
	}
}

// job is one generation-scoped request awaiting the ordered worker. release is
// non-nil for cancellable operations and is called after dispatch.
type job struct {
	request       *typefacts.LifecycleRequest
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
func serve(ctx context.Context, respond responder, input io.Reader, output *bufio.Writer, trace typefacts.Trace) error {
	var cancelMu sync.Mutex
	cancels := make(map[uint64]context.CancelFunc)

	jobs := newQueue[job]()
	responses := newQueue[typefacts.LifecycleResponse]()
	fatal := make(chan error, 1)
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
			value := respond.lifecycle(next.ctx, *next.request)
			if value.Timings != nil {
				value.Timings.RequestDecodeNs = uint64(next.requestDecode)
			}
			if next.release != nil {
				next.release()
			}
			// Ownership has moved to the response queue. Clear the worker's
			// request (including the decoded compact demand frame) and its
			// response copy before blocking for the next job; otherwise this
			// idle goroutine pins both complete cold sides of the exchange.
			next = job{}
			responses.push(value)
			value = typefacts.LifecycleResponse{}
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
			logicalResponseBytes := uint64(len(value.TableTransition))
			if value.SourceArena != "" && len(value.SourceLengths) == 3 {
				logicalResponseBytes = max(
					logicalResponseBytes,
					value.SourceLengths[2],
				)
			}
			responseBytes, err := writeLifecycleResponse(output, value, trace)
			if err != nil {
				fatal <- err
				return
			}
			// A cold materialization transfers ownership of a large packed
			// frame to the client and releases checker query state. Drop both
			// complete payload owners before reclaiming pages. Encoding lives
			// in a helper frame so its full-size CBOR buffer is unreachable
			// here; clearing value drops the semantic frame. Ordinary cached
			// and incremental responses stay off this path.
			value = typefacts.LifecycleResponse{}
			if max(uint64(responseBytes), logicalResponseBytes) >= 1<<20 {
				debug.FreeOSMemory()
			}
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
		decodeStarted := time.Now()
		var request typefacts.LifecycleRequest
		if err := wirecbor.Unmarshal(next.payload, &request); err != nil {
			return drain(fmt.Errorf("decode request: %w", err))
		}
		if request.Operation == typefacts.LifecycleCancel {
			cancelMu.Lock()
			cancel := cancels[request.CancelRequestID]
			cancelMu.Unlock()
			if cancel != nil {
				cancel()
			}
			jobs.push(job{request: &request, ctx: ctx})
			continue
		}
		requestCtx, cancel := context.WithCancel(ctx)
		cancelMu.Lock()
		cancels[request.RequestID] = cancel
		cancelMu.Unlock()
		requestID := request.RequestID
		jobs.push(job{request: &request, ctx: requestCtx, requestDecode: time.Since(decodeStarted), release: func() {
			cancel()
			cancelMu.Lock()
			delete(cancels, requestID)
			cancelMu.Unlock()
		}})
	}
}

func writeLifecycleResponse(
	output *bufio.Writer,
	value typefacts.LifecycleResponse,
	trace typefacts.Trace,
) (int, error) {
	encodeStarted := time.Now()
	if len(value.TableTransition) >= 1<<20 {
		chunks, encodedSize, err := lifecycleResponseChunks(value)
		if err != nil {
			return 0, fmt.Errorf("encode response: %w", err)
		}
		if trace != nil {
			trace.Stage("encode-response", time.Since(encodeStarted))
		}
		writeStarted := time.Now()
		if err := writeFrameHeader(output, encodedSize); err != nil {
			return 0, err
		}
		for _, chunk := range chunks {
			if _, err := output.Write(chunk); err != nil {
				return 0, err
			}
		}
		if err := output.Flush(); err != nil {
			return 0, err
		}
		if trace != nil {
			trace.Stage("write-response", time.Since(writeStarted))
		}
		return encodedSize, nil
	}
	encoded, err := wirecbor.Marshal(value)
	if err != nil {
		return 0, fmt.Errorf("encode response: %w", err)
	}
	if trace != nil {
		trace.Stage("encode-response", time.Since(encodeStarted))
	}
	writeStarted := time.Now()
	if err := writeFrame(output, encoded); err != nil {
		return 0, err
	}
	if err := output.Flush(); err != nil {
		return 0, err
	}
	if trace != nil {
		trace.Stage("write-response", time.Since(writeStarted))
	}
	return len(encoded), nil
}

type lifecycleResponseChunk struct {
	key   []byte
	value []byte
}

// lifecycleResponseChunks encodes a response as deterministic-CBOR map
// fragments, keeping tableTransition as a borrowed byte slice. This is the
// only large field in a cold response, so the outer frame can be written
// incrementally without a second complete 5+ MiB payload.
func lifecycleResponseChunks(value typefacts.LifecycleResponse) ([][]byte, int, error) {
	fields := make([]lifecycleResponseChunk, 0, 13)
	add := func(key string, field any) error {
		encodedKey, err := wirecbor.Marshal(key)
		if err != nil {
			return err
		}
		encodedValue, err := wirecbor.Marshal(field)
		if err != nil {
			return err
		}
		fields = append(fields, lifecycleResponseChunk{key: encodedKey, value: encodedValue})
		return nil
	}
	for _, field := range []struct {
		key   string
		value any
	}{
		{"schema", value.Schema},
		{"requestId", value.RequestID},
		{"projectId", value.ProjectID},
		{"generation", value.Generation},
		{"ok", value.OK},
	} {
		if err := add(field.key, field.value); err != nil {
			return nil, 0, err
		}
	}
	optional := []struct {
		present bool
		key     string
		value   any
	}{
		{value.StateToken != "", "stateToken", value.StateToken},
		{len(value.Affected) != 0, "affected", value.Affected},
		{len(value.Sources) != 0, "sources", value.Sources},
		{value.SourceArena != "", "sourceArena", value.SourceArena},
		{len(value.SourceLengths) != 0, "sourceLengths", value.SourceLengths},
		{len(value.SymbolEvidence) != 0, "symbolEvidence", value.SymbolEvidence},
		{len(value.ReferenceEvidence) != 0, "referenceEvidence", value.ReferenceEvidence},
		{len(value.ChangedReferenceSymbols) != 0, "changedReferenceSymbols", value.ChangedReferenceSymbols},
		{value.ReferenceChangesExact, "referenceChangesExact", value.ReferenceChangesExact},
		{value.Timings != nil, "timings", value.Timings},
		{value.Error != nil, "error", value.Error},
	}
	for _, field := range optional {
		if field.present {
			if err := add(field.key, field.value); err != nil {
				return nil, 0, err
			}
		}
	}
	transitionKey, err := wirecbor.Marshal("tableTransition")
	if err != nil {
		return nil, 0, err
	}
	fields = append(fields, lifecycleResponseChunk{
		key:   transitionKey,
		value: appendCBORByteStringHeader(nil, len(value.TableTransition)),
	})
	sort.Slice(fields, func(i, j int) bool {
		return bytes.Compare(fields[i].key, fields[j].key) < 0
	})
	chunks := make([][]byte, 0, 1+len(fields)*2+1)
	chunks = append(chunks, []byte{0xa0 | byte(len(fields))})
	total := 1
	for _, field := range fields {
		chunks = append(chunks, field.key, field.value)
		total += len(field.key) + len(field.value)
		if bytes.Equal(field.key, transitionKey) {
			chunks = append(chunks, value.TableTransition)
			total += len(value.TableTransition)
		}
	}
	return chunks, total, nil
}

func appendCBORByteStringHeader(target []byte, size int) []byte {
	switch {
	case size < 24:
		return append(target, 0x40|byte(size))
	case size <= 0xff:
		return append(target, 0x58, byte(size))
	case size <= 0xffff:
		return binary.BigEndian.AppendUint16(append(target, 0x59), uint16(size))
	case uint64(size) <= uint64(^uint32(0)):
		return binary.BigEndian.AppendUint32(append(target, 0x5a), uint32(size))
	default:
		return binary.BigEndian.AppendUint64(append(target, 0x5b), uint64(size))
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
	if err := writeFrameHeader(writer, len(payload)); err != nil {
		return err
	}
	_, err := writer.Write(payload)
	return err
}

func writeFrameHeader(writer io.Writer, size int) error {
	if size > wirecbor.MaxMessageBytes {
		return fmt.Errorf("response is %d bytes, limit is %d", size, wirecbor.MaxMessageBytes)
	}
	var prefix [4]byte
	binary.LittleEndian.PutUint32(prefix[:], uint32(size))
	_, err := writer.Write(prefix[:])
	return err
}
