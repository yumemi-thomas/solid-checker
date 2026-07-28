package typefacts_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
	"github.com/yumemi-thomas/solid-ts-facts/internal/wirecbor"
)

// reportResponseWireBytes records the encoded size of the response frame the
// producer would write for the benchmark's steady-state answer. Response bytes
// are a first-class budget (ADR-0005 quotes them alongside latency), and none
// of the timing numbers can see a transport-shape regression that leaves
// latency alone on a small corpus. Encoding happens after the timed loop, so
// ns/op keeps meaning what it always has.
func reportResponseWireBytes(b *testing.B, response typefacts.LifecycleResponse) {
	b.Helper()
	encoded, err := wirecbor.Marshal(response)
	if err != nil {
		b.Fatal(err)
	}
	b.ReportMetric(float64(len(encoded)), "resp-B/op")
}

// These benchmarks price the analysis itself, against a real TypeScript program
// on the generated corpus. Interface doubles cannot stand in here: the dominant
// costs — the checker lock, the reference-index merge, symbol-chunk patching —
// all live in the tsgo backend, and a double that returns nothing prices none
// of them.
//
// Edits append to the end of the edited file so every demand location stays
// valid, which keeps demand construction out of the timed region. A real client
// holds its own locations and would not re-derive them either.

func openCorpusSession(b *testing.B) (*typefacts.Session, string, []typefacts.EntityDemand) {
	return openCorpusSessionWithTrace(b, nil)
}

func openCorpusSessionWithTrace(b *testing.B, trace typefacts.Trace) (*typefacts.Session, string, []typefacts.EntityDemand) {
	b.Helper()
	ctx := context.Background()
	root := generateCorpus(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, trace)
	if err != nil {
		b.Fatal(err)
	}
	demands := realisticDemands(b, backend.(demandSource), ctx)
	session, err := typefacts.NewSession(backend, projectID, trace)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = session.Close() })
	return session, root, demands
}

type corpusStageTrace struct {
	updateTotal, updateProgram                     int64
	updateOverlay, updateOldShape, updateNewShape  int64
	updateAffected, updateInvalidation             int64
	materialize, async, demand, symbols, transport int64
}

func (t *corpusStageTrace) Stage(name string, elapsed time.Duration) {
	switch {
	case name == "analyze-materialize":
		t.materialize += elapsed.Nanoseconds()
	case name == "analyze-async":
		t.async += elapsed.Nanoseconds()
	case name == "analyze-demand":
		t.demand += elapsed.Nanoseconds()
	case name == "analyze-symbols":
		t.symbols += elapsed.Nanoseconds()
	case strings.HasPrefix(name, "analyze-transport-"):
		t.transport += elapsed.Nanoseconds()
	}
}

func (t *corpusStageTrace) Metrics(name string, values ...typefacts.Metric) {
	if name != "update" {
		return
	}
	for _, metric := range values {
		switch metric.Key {
		case "totalNs":
			t.updateTotal += metric.Value
		case "programNs":
			t.updateProgram += metric.Value
		case "overlayNs":
			t.updateOverlay += metric.Value
		case "oldShapeNs":
			t.updateOldShape += metric.Value
		case "newShapeNs":
			t.updateNewShape += metric.Value
		case "affectedNs":
			t.updateAffected += metric.Value
		case "invalidationNs":
			t.updateInvalidation += metric.Value
		}
	}
}

func corpusRequest(id uint64, operation typefacts.LifecycleOperation, projectID string, generation uint64) typefacts.LifecycleRequest {
	return typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV5,
		RequestID:  id,
		Operation:  operation,
		ProjectID:  projectID,
		Generation: generation,
	}
}

// BenchmarkAnalyzeAfterLeafEditAtScale is the editor path and the perf gate for
// this package: one accepted update to a single leaf module, then one analyze of
// the new generation. Retention should carry every unedited file, so the cost
// here is the incremental work plus delta construction.
func BenchmarkAnalyzeAfterLeafEditAtScale(b *testing.B) {
	ctx := context.Background()
	session, root, demands := openCorpusSession(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))

	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}

	cold := corpusRequest(1, typefacts.LifecycleAnalyze, projectID, 1)
	cold.ResetState = true
	cold.Demands = demands
	response := session.Lifecycle(ctx, cold)
	if !response.OK || len(response.TableTransition) == 0 {
		b.Fatalf("initial analyze: %+v", response.Error)
	}
	token := response.StateToken

	requestID := uint64(1)
	generation := uint64(1)
	edit := 0
	var lastAnalyzed typefacts.LifecycleResponse
	var updateElapsed time.Duration
	var analyzeElapsed time.Duration

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		edit++
		generation++
		requestID++

		source := make([]byte, 0, len(original)+32)
		source = append(source, original...)
		source = append(source, fmt.Sprintf("\n// edit %d\n", edit)...)

		update := corpusRequest(requestID, typefacts.LifecycleUpdate, projectID, generation)
		update.Changes = []typefacts.FileChangeV3{{Path: editPath, Version: uint64(edit), Source: source}}
		started := time.Now()
		if updated := session.Lifecycle(ctx, update); !updated.OK {
			b.Fatalf("update %d: %+v", edit, updated.Error)
		}
		updateElapsed += time.Since(started)

		requestID++
		analyze := corpusRequest(requestID, typefacts.LifecycleAnalyze, projectID, generation)
		analyze.StateToken = token
		started = time.Now()
		analyzed := session.Lifecycle(ctx, analyze)
		if !analyzed.OK {
			b.Fatalf("analyze %d: %+v", edit, analyzed.Error)
		}
		analyzeElapsed += time.Since(started)
		token = analyzed.StateToken
		lastAnalyzed = analyzed
	}
	b.ReportMetric(float64(updateElapsed.Nanoseconds())/float64(b.N), "update-ns/op")
	b.ReportMetric(float64(analyzeElapsed.Nanoseconds())/float64(b.N), "analyze-ns/op")
	reportResponseWireBytes(b, lastAnalyzed)
}

// BenchmarkWarmLeafStageBreakdownAtScale runs the primary gate with tracing
// enabled and reports the private stage timings that bound each architectural
// candidate's maximum possible contribution. The ordinary leaf benchmark
// keeps tracing nil so these diagnostics never tax its acceptance number.
func BenchmarkWarmLeafStageBreakdownAtScale(b *testing.B) {
	ctx := context.Background()
	trace := &corpusStageTrace{}
	session, root, demands := openCorpusSessionWithTrace(b, trace)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}
	cold := corpusRequest(1, typefacts.LifecycleAnalyze, projectID, 1)
	cold.ResetState = true
	cold.Demands = demands
	response := session.Lifecycle(ctx, cold)
	if !response.OK {
		b.Fatalf("initial analyze: %+v", response.Error)
	}
	token := response.StateToken
	*trace = corpusStageTrace{}
	requestID, generation := uint64(1), uint64(1)
	edit := 0
	var assemblyNs, closeNs uint64

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		edit++
		generation++
		requestID++
		source := append(append(make([]byte, 0, len(original)+32), original...),
			fmt.Sprintf("\n// edit %d\n", edit)...)
		update := corpusRequest(requestID, typefacts.LifecycleUpdate, projectID, generation)
		update.Changes = []typefacts.FileChangeV3{{Path: editPath, Version: uint64(edit), Source: source}}
		if updated := session.Lifecycle(ctx, update); !updated.OK {
			b.Fatalf("update %d: %+v", edit, updated.Error)
		}
		requestID++
		analyze := corpusRequest(requestID, typefacts.LifecycleAnalyze, projectID, generation)
		analyze.StateToken = token
		analyzed := session.Lifecycle(ctx, analyze)
		if !analyzed.OK {
			b.Fatalf("analyze %d: %+v", edit, analyzed.Error)
		}
		assemblyNs += analyzed.Timings.AssemblyNs
		closeNs += analyzed.Timings.CloseSymbolsNs
		token = analyzed.StateToken
	}
	divisor := float64(b.N)
	b.ReportMetric(float64(trace.updateTotal)/divisor, "trace-update-ns/op")
	b.ReportMetric(float64(trace.updateProgram)/divisor, "program-ns/op")
	b.ReportMetric(float64(trace.updateOverlay)/divisor, "overlay-ns/op")
	b.ReportMetric(float64(trace.updateOldShape)/divisor, "old-shape-ns/op")
	b.ReportMetric(float64(trace.updateNewShape)/divisor, "new-shape-ns/op")
	b.ReportMetric(float64(trace.updateAffected)/divisor, "affected-ns/op")
	b.ReportMetric(float64(trace.updateInvalidation)/divisor, "invalidation-ns/op")
	b.ReportMetric(float64(trace.materialize)/divisor, "materialize-ns/op")
	b.ReportMetric(float64(trace.async)/divisor, "async-ns/op")
	b.ReportMetric(float64(trace.demand)/divisor, "demand-ns/op")
	b.ReportMetric(float64(trace.symbols)/divisor, "symbols-ns/op")
	b.ReportMetric(float64(trace.transport)/divisor, "transport-ns/op")
	b.ReportMetric(float64(assemblyNs)/divisor, "assembly-ns/op")
	b.ReportMetric(float64(closeNs)/divisor, "close-ns/op")
}

// BenchmarkAnalyzeAfterRootChangeAtScale prices the general Symbol closure
// path: every generation changes one file's retained Semantic demand run, so
// the raw root and Full-tier seed sets alternate and the stable-universe proof
// cannot apply. The source edit itself remains a leaf comment edit, keeping the
// benchmark focused on root ownership rather than declaration invalidation.
func BenchmarkAnalyzeAfterRootChangeAtScale(b *testing.B) {
	ctx := context.Background()
	session, root, demands := openCorpusSession(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	editPath := filepath.Clean(filepath.Join(root, "mod00.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}
	var fullRun []typefacts.EntityDemand
	for _, demand := range demands {
		if filepath.Clean(demand.Location.Path) == editPath {
			fullRun = append(fullRun, demand)
		}
	}
	if len(fullRun) < 2 {
		b.Fatalf("root-change run has %d demands, want at least 2", len(fullRun))
	}
	shortRun := fullRun[:len(fullRun)-1]

	cold := corpusRequest(1, typefacts.LifecycleAnalyze, projectID, 1)
	cold.ResetState = true
	cold.Demands = demands
	response := session.Lifecycle(ctx, cold)
	if !response.OK {
		b.Fatalf("initial analyze: %+v", response.Error)
	}
	token := response.StateToken
	requestID, generation := uint64(1), uint64(1)
	edit := 0
	var lastAnalyzed typefacts.LifecycleResponse

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		edit++
		generation++
		requestID++
		source := append(append(make([]byte, 0, len(original)+32), original...),
			fmt.Sprintf("\n// root edit %d\n", edit)...)
		update := corpusRequest(requestID, typefacts.LifecycleUpdate, projectID, generation)
		update.Changes = []typefacts.FileChangeV3{{Path: editPath, Version: uint64(edit), Source: source}}
		if updated := session.Lifecycle(ctx, update); !updated.OK {
			b.Fatalf("update %d: %+v", edit, updated.Error)
		}

		requestID++
		analyze := corpusRequest(requestID, typefacts.LifecycleAnalyze, projectID, generation)
		analyze.StateToken = token
		if edit%2 == 0 {
			analyze.Demands = fullRun
		} else {
			analyze.Demands = shortRun
		}
		analyzed := session.Lifecycle(ctx, analyze)
		if !analyzed.OK {
			b.Fatalf("analyze %d: %+v", edit, analyzed.Error)
		}
		token = analyzed.StateToken
		lastAnalyzed = analyzed
	}
	reportResponseWireBytes(b, lastAnalyzed)
}

// BenchmarkAnalyzeAfterShapeChangeAtScale alternates an exported shared-module
// parameter type between two equal-width spellings. The byte-stable edit keeps
// every precomputed Demand location valid while forcing declaration-shape
// invalidation through all importers.
func BenchmarkAnalyzeAfterShapeChangeAtScale(b *testing.B) {
	ctx := context.Background()
	session, root, demands := openCorpusSession(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	editPath := filepath.Clean(filepath.Join(root, "shared.ts"))
	original, err := os.ReadFile(editPath)
	if err != nil {
		b.Fatal(err)
	}
	changedText := strings.Replace(string(original), "value: number): number", "value: bigint): number", 1)
	if changedText == string(original) || len(changedText) != len(original) {
		b.Fatal("shape-change fixture replacement failed or shifted Demand locations")
	}
	changed := []byte(changedText)

	cold := corpusRequest(1, typefacts.LifecycleAnalyze, projectID, 1)
	cold.ResetState = true
	cold.Demands = demands
	response := session.Lifecycle(ctx, cold)
	if !response.OK {
		b.Fatalf("initial analyze: %+v", response.Error)
	}
	token := response.StateToken
	requestID, generation := uint64(1), uint64(1)
	edit := 0
	var lastAnalyzed typefacts.LifecycleResponse

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		edit++
		generation++
		requestID++
		source := changed
		if edit%2 == 0 {
			source = original
		}
		update := corpusRequest(requestID, typefacts.LifecycleUpdate, projectID, generation)
		update.Changes = []typefacts.FileChangeV3{{Path: editPath, Version: uint64(edit), Source: source}}
		if updated := session.Lifecycle(ctx, update); !updated.OK {
			b.Fatalf("update %d: %+v", edit, updated.Error)
		}

		requestID++
		analyze := corpusRequest(requestID, typefacts.LifecycleAnalyze, projectID, generation)
		analyze.StateToken = token
		analyzed := session.Lifecycle(ctx, analyze)
		if !analyzed.OK {
			b.Fatalf("analyze %d: %+v", edit, analyzed.Error)
		}
		token = analyzed.StateToken
		lastAnalyzed = analyzed
	}
	reportResponseWireBytes(b, lastAnalyzed)
}

// BenchmarkFullTableAnalyzeAtScale prices the cold path a client pays on its
// first analyze and after any retained-state desync: a reset-state analyze,
// which packs the whole table instead of emitting a delta.
func BenchmarkFullTableAnalyzeAtScale(b *testing.B) {
	ctx := context.Background()
	session, root, demands := openCorpusSession(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))

	requestID := uint64(0)
	var lastResponse typefacts.LifecycleResponse
	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		requestID++
		request := corpusRequest(requestID, typefacts.LifecycleAnalyze, projectID, 1)
		request.ResetState = true
		request.Demands = demands
		response := session.Lifecycle(ctx, request)
		if !response.OK || len(response.TableTransition) == 0 {
			b.Fatalf("reset-state analyze: %+v", response.Error)
		}
		lastResponse = response
	}
	reportResponseWireBytes(b, lastResponse)
}
