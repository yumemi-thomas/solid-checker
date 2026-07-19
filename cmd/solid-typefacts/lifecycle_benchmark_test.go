package main

import (
	"context"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
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
func (lifecycleBenchmarkBackend) TypeAt(context.Context, typefacts.Location) (typefacts.TypeID, error) {
	return "", typefacts.ErrNotFound
}
func (lifecycleBenchmarkBackend) ResolvedCall(context.Context, typefacts.Location) (typefacts.Call, error) {
	return typefacts.Call{}, typefacts.ErrNotFound
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

var lifecycleBenchmarkResponse typefacts.LifecycleResponse

func BenchmarkLifecycleWarmReuse(b *testing.B) {
	ctx := context.Background()
	session, err := typefacts.NewSession(lifecycleBenchmarkBackend{}, "/project/tsconfig.json", false)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { _ = session.Close() })
	projectID := "/project/tsconfig.json"
	generation := uint64(1)
	first := session.Lifecycle(ctx, typefacts.LifecycleRequest{
		Schema: typefacts.TypeFactsSchemaVersionV3, RequestID: 1,
		Operation: typefacts.LifecycleAnalyze, ProjectID: projectID, Generation: generation,
		ResetState: true,
	})
	if !first.OK || first.TableMode != typefacts.TableModeFull {
		b.Fatalf("initialize retained state: %+v", first)
	}
	request := typefacts.LifecycleRequest{
		Schema: typefacts.TypeFactsSchemaVersionV3, RequestID: 2,
		Operation: typefacts.LifecycleAnalyze, ProjectID: projectID, Generation: generation,
		StateToken: first.StateToken,
	}

	b.ReportAllocs()
	b.ResetTimer()
	for b.Loop() {
		lifecycleBenchmarkResponse = session.Lifecycle(ctx, request)
	}
	if !lifecycleBenchmarkResponse.OK || lifecycleBenchmarkResponse.TableMode != typefacts.TableModeReuse {
		b.Fatalf("warm retained response: %+v", lifecycleBenchmarkResponse)
	}
}
