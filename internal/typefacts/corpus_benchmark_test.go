package typefacts_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

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
	b.Helper()
	ctx := context.Background()
	root := generateCorpus(b)
	projectID := filepath.Clean(filepath.Join(root, "tsconfig.json"))
	backend, err := tsgo.OpenProject(ctx, projectID, nil)
	if err != nil {
		b.Fatal(err)
	}
	demands := realisticDemands(b, backend.(demandSource), ctx)
	session, err := typefacts.NewSession(backend, projectID, nil)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = session.Close() })
	return session, root, demands
}

func corpusRequest(id uint64, operation typefacts.LifecycleOperation, projectID string, generation uint64) typefacts.LifecycleRequest {
	return typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV3,
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
	if !response.OK || response.TableMode != typefacts.TableModeFull {
		b.Fatalf("initial analyze: %+v", response.Error)
	}
	token := response.StateToken

	requestID := uint64(1)
	generation := uint64(1)
	edit := 0
	var lastAnalyzed typefacts.LifecycleResponse

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
		if !response.OK || response.TableMode != typefacts.TableModeFull {
			b.Fatalf("reset-state analyze: %+v", response.Error)
		}
		lastResponse = response
	}
	reportResponseWireBytes(b, lastResponse)
}
