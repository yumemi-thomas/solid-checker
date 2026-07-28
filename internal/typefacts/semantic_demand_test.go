package typefacts

import (
	"context"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"
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
func (transportOnlyBackend) Declarations(_ context.Context, id SymbolID) ([]Declaration, error) {
	location, ok := doubleSymbolLocation(id)
	if !ok {
		return nil, ErrNotFound
	}
	// Only declaration-backed identities are eligible for cross-generation
	// reuse, so a double that answers ErrNotFound here would silently disable
	// the symbol-fact memo it is meant to exercise.
	return []Declaration{{Name: "value", Kind: "variable", Location: location}}, nil
}
func (transportOnlyBackend) References(context.Context, SymbolID) ([]Location, error) {
	return nil, ErrNotFound
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

// The capabilities below are the ones the production path actually uses, so
// tests built on this double traverse the same branches the producer ships:
// SemanticDemandRuns, AsyncFunctionsAt, ReferencesBatch and an exact
// ChangedReferences.

// doubleSymbolPrefix keeps minted identities durable in the sense of ADR 0001
// (DurableSymbolID only requires this prefix), which is what lets retained
// contributions survive a generation. The location is encoded in the identity
// rather than hashed so the double can answer Declarations without holding any
// state — it stays usable by value, as every construction site expects.
const doubleSymbolPrefix = "symbol:h:"

func doubleSymbolID(location Location) SymbolID {
	return SymbolID(fmt.Sprintf("%s%s:%d:%d", doubleSymbolPrefix, location.Path, location.StartByte, location.EndByte))
}

func doubleSymbolLocation(id SymbolID) (Location, bool) {
	rest, ok := strings.CutPrefix(string(id), doubleSymbolPrefix)
	if !ok {
		return Location{}, false
	}
	// Split from the right: the encoded path may itself contain separators.
	lastColon := strings.LastIndex(rest, ":")
	if lastColon < 0 {
		return Location{}, false
	}
	firstColon := strings.LastIndex(rest[:lastColon], ":")
	if firstColon < 0 {
		return Location{}, false
	}
	start, err := strconv.Atoi(rest[firstColon+1 : lastColon])
	if err != nil {
		return Location{}, false
	}
	end, err := strconv.Atoi(rest[lastColon+1:])
	if err != nil {
		return Location{}, false
	}
	return Location{Path: rest[:firstColon], StartByte: start, EndByte: end}, true
}

// SemanticDemandRuns returns one aligned answer per per-file run. The scope is
// deliberately ignored: honouring it here would be a second implementation of
// a rule the tsgo adapter already owns, and the retained-versus-fresh oracle
// checks that rule against a real compiler.
func (transportOnlyBackend) SemanticDemandRuns(
	_ context.Context,
	runs []SemanticDemandRun,
	_ SemanticScope,
) ([]SemanticDemandRunResult, error) {
	results := make([]SemanticDemandRunResult, len(runs))
	for runIndex := range runs {
		run := &runs[runIndex]
		result := &results[runIndex]
		result.Entities = make([]EntityFact, len(run.Demands))
		result.Structural = make([]SymbolID, len(run.Demands))
		result.Durable = true
		for demandIndex := range run.Demands {
			demand := &run.Demands[demandIndex]
			location := demand.Location
			location.Path = filepath.Clean(location.Path)
			result.Entities[demandIndex] = EntityFact{Location: location}
			if demand.Symbol {
				result.Entities[demandIndex].Symbol = doubleSymbolID(location)
			}
			if demand.StructuralAccessor {
				result.Structural[demandIndex] = result.Entities[demandIndex].Symbol
			}
		}
	}
	return results, nil
}

func (transportOnlyBackend) AsyncFunctionsAt(context.Context, []Location) ([]AsyncFunctionFact, error) {
	return nil, nil
}

func (transportOnlyBackend) ReferencesBatch(_ context.Context, ids []SymbolID) (map[SymbolID][]Location, error) {
	result := make(map[SymbolID][]Location, len(ids))
	for _, id := range ids {
		if location, ok := doubleSymbolLocation(id); ok {
			result[id] = []Location{location}
		}
	}
	return result, nil
}

// ChangedReferences reports an exact, empty delta. Exactness is what lets the
// closure reach patchCanonicalSymbolStore and the retained reference path.
func (transportOnlyBackend) ChangedReferences(context.Context) ([]SymbolID, bool, error) {
	return nil, true, nil
}

func (transportOnlyBackend) ReleaseAnalysisState() {}

func TestRetainedContributionSharesCanonicalEntityBacking(t *testing.T) {
	path := filepath.Clean("/project/source.ts")
	closure, err := NewDemandClosure(transportOnlyBackend{
		source: SourceFile{Path: path, Source: []byte("const value = 1\n")},
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer closure.Close()
	demand := EntityDemand{
		Location: Location{Path: path, StartByte: 6, EndByte: 11},
		Symbol:   true,
	}
	table, err := closure.DemandTableForGroups(
		context.Background(),
		1,
		[]DemandGroup{{Path: path, Demands: []EntityDemand{demand}}},
		[]string{path},
	)
	if err != nil {
		t.Fatal(err)
	}
	contribution := closure.retained.get(path)
	if contribution == nil || len(contribution.entities) != 1 || len(table.Entities) != 1 {
		t.Fatalf("unexpected retained state: contribution=%+v entities=%d", contribution, len(table.Entities))
	}
	if &contribution.entities[0] != &table.Entities[0] {
		t.Fatal("retained contribution duplicates the canonical entity backing")
	}
}
