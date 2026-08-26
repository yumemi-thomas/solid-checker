package main

import (
	"context"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

type lifecycleBenchmarkBackend struct{}

func (lifecycleBenchmarkBackend) SourceFiles(context.Context) ([]typefacts.SourceFile, error) {
	return []typefacts.SourceFile{{Path: "/project/source.ts", Source: []byte("export const value = 1\n")}}, nil
}
func (lifecycleBenchmarkBackend) Update(context.Context, []typefacts.FileChange) (typefacts.AffectedSet, error) {
	return typefacts.AffectedSet{}, nil
}
func (lifecycleBenchmarkBackend) SymbolAt(context.Context, typefacts.Location) (typefacts.SymbolID, error) {
	return "", typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) ResolveAlias(context.Context, typefacts.SymbolID) (typefacts.SymbolID, error) {
	return "", typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) Declarations(context.Context, typefacts.SymbolID) ([]typefacts.Declaration, error) {
	return nil, typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) References(context.Context, typefacts.SymbolID) ([]typefacts.Location, error) {
	return nil, typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) Close() error { return nil }
func (lifecycleBenchmarkBackend) DescribeTypeAt(context.Context, typefacts.Location) (typefacts.TypeDescriptor, error) {
	return typefacts.TypeDescriptor{}, typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) SourceCalls(context.Context, string) ([]typefacts.SourceCall, error) {
	return nil, nil
}
func (lifecycleBenchmarkBackend) SourceBindings(context.Context, string) ([]typefacts.SourceBinding, error) {
	return nil, nil
}
func (lifecycleBenchmarkBackend) SourceFunctions(context.Context, string) ([]typefacts.SourceFunction, error) {
	return nil, nil
}
func (lifecycleBenchmarkBackend) SourceAsyncFunctions(context.Context, string) ([]typefacts.AsyncFunctionFact, error) {
	return nil, nil
}

// The production capability quartet. Without these the double only satisfies
// the unscoped surface, and the benchmark would drive a materializer no release
// runs.
func (lifecycleBenchmarkBackend) SemanticDemandRuns(
	_ context.Context,
	runs []typefacts.SemanticDemandRun,
	_ typefacts.SemanticScope,
) ([]typefacts.SemanticDemandRunResult, error) {
	results := make([]typefacts.SemanticDemandRunResult, len(runs))
	for runIndex := range runs {
		run := &runs[runIndex]
		results[runIndex].Entities = make([]typefacts.EntityFact, len(run.Demands))
		results[runIndex].Structural = make([]typefacts.SymbolID, len(run.Demands))
		results[runIndex].Durable = true
		for demandIndex := range run.Demands {
			results[runIndex].Entities[demandIndex] = typefacts.EntityFact{
				Location: run.Demands[demandIndex].Location,
			}
		}
	}
	return results, nil
}

func (lifecycleBenchmarkBackend) AsyncFunctionsAt(context.Context, []typefacts.Location) ([]typefacts.AsyncFunctionFact, error) {
	return nil, nil
}

func (lifecycleBenchmarkBackend) ReferencesBatch(context.Context, []typefacts.SymbolID) (map[typefacts.SymbolID][]typefacts.Location, error) {
	return nil, nil
}

func (lifecycleBenchmarkBackend) ChangedReferences(context.Context) ([]typefacts.SymbolID, bool, error) {
	return nil, true, nil
}
func (lifecycleBenchmarkBackend) ReleaseAnalysisState() {}

var lifecycleBenchmarkResponse typefacts.LifecycleResponse

// BenchmarkLifecycleWarmReuse measures the reuse short-circuit alone: the timed
// request presents a matching state token with no demand changes, so
// session.lifecycle answers from retained state and never reaches
// DemandTableForGroups. It prices a token compare and a map lookup — not
// materialization. For the analysis cost see the corpus benchmarks in
// internal/typefacts.
func BenchmarkLifecycleWarmReuse(b *testing.B) {
	ctx := context.Background()
	session, err := typefacts.NewSession(lifecycleBenchmarkBackend{}, "/project/tsconfig.json", nil)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = session.Close() })
	projectID := "/project/tsconfig.json"
	generation := uint64(1)
	first := session.Lifecycle(ctx, typefacts.LifecycleRequest{
		Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: 1,
		Operation: typefacts.LifecycleAnalyze, ProjectID: projectID, Generation: generation,
		ResetState: true,
	})
	if !first.OK || len(first.TableTransition) == 0 {
		b.Fatalf("initialize retained state: %+v", first)
	}
	request := typefacts.LifecycleRequest{
		Schema: typefacts.TypeFactsSchemaVersionV1, RequestID: 2,
		Operation: typefacts.LifecycleAnalyze, ProjectID: projectID, Generation: generation,
		StateToken: first.StateToken,
	}

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		lifecycleBenchmarkResponse = session.Lifecycle(ctx, request)
	}
	if !lifecycleBenchmarkResponse.OK || len(lifecycleBenchmarkResponse.TableTransition) != 0 {
		b.Fatalf("warm retained response: %+v", lifecycleBenchmarkResponse)
	}
}
