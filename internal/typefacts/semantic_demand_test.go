package typefacts

import (
	"context"
	"reflect"
	"testing"
)

type transportOnlyBackend struct {
	source SourceFile
}

func (b transportOnlyBackend) SourceFiles(context.Context) ([]SourceFile, error) {
	return []SourceFile{b.source}, nil
}
func (transportOnlyBackend) Update(context.Context, []FileChange) (AffectedSet, error) {
	return AffectedSet{}, nil
}
func (transportOnlyBackend) SymbolAt(context.Context, Location) (SymbolID, error) {
	return "", ErrNotFound
}
func (transportOnlyBackend) ResolveAlias(context.Context, SymbolID) (SymbolID, error) {
	return "", ErrNotFound
}
func (transportOnlyBackend) Declarations(context.Context, SymbolID) ([]Declaration, error) {
	return nil, ErrNotFound
}
func (transportOnlyBackend) References(context.Context, SymbolID) ([]Location, error) {
	return nil, ErrNotFound
}
func (transportOnlyBackend) TypeAt(context.Context, Location) (TypeID, error) {
	return "", ErrNotFound
}
func (transportOnlyBackend) ResolvedCall(context.Context, Location) (Call, error) {
	return Call{}, ErrNotFound
}
func (transportOnlyBackend) Close() error { return nil }
func (transportOnlyBackend) DescribeTypeAt(context.Context, Location) (TypeDescriptor, error) {
	return TypeDescriptor{}, ErrNotFound
}
func (transportOnlyBackend) SourceCalls(context.Context, string) ([]SourceCall, error) {
	return nil, nil
}
func (transportOnlyBackend) SourceBindings(context.Context, string) ([]SourceBinding, error) {
	return nil, nil
}
func (transportOnlyBackend) SourceFunctions(context.Context, string) ([]SourceFunction, error) {
	return nil, nil
}
func (transportOnlyBackend) SourceAsyncFunctions(context.Context, string) ([]AsyncFunctionFact, error) {
	return nil, nil
}

func TestSemanticDemandTableStaysTransportOnly(t *testing.T) {
	t.Parallel()

	backend := transportOnlyBackend{source: SourceFile{
		Path:   "/project/source.ts",
		Source: []byte("export const value = 1\n"),
	}}
	table, _, _, stages, err := materializeSemanticDemand(context.Background(), backend, nil, 1)
	if err != nil {
		t.Fatal(err)
	}
	if table.runtime != nil {
		t.Fatal("semantic demand table built runtime lookup indexes")
	}
	if stages.prepare != 0 {
		t.Fatalf("prepare duration = %v, want zero for transport-only table", stages.prepare)
	}

	first := tableV2(*table, "/project/tsconfig.json", 1)
	second := tableV2(*table, "/project/tsconfig.json", 1)
	if !reflect.DeepEqual(first, second) {
		t.Fatal("repeated transport conversion changed the table")
	}
}
