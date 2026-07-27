package typefacts

import (
	"context"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"
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

// The capabilities below are the ones the production path actually uses, so
// tests built on this double traverse the same branches the producer ships:
// SemanticEntitiesScoped, AsyncFunctionsAt, ReferencesBatch and an exact
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

// SemanticEntitiesScoped returns one entity per demand, index-aligned as the
// contract requires. The suppression set and descriptor seed are deliberately
// ignored: honouring them here would be a second implementation of a rule the
// tsgo backend already owns, and the retained-versus-fresh oracle checks that
// rule against a real compiler.
func (transportOnlyBackend) SemanticEntitiesScoped(
	_ context.Context,
	demands []EntityDemand,
	_ map[SymbolID]struct{},
	_ map[SymbolID]*TypeDescriptor,
) ([]EntityFact, []SymbolID, error) {
	entities := make([]EntityFact, len(demands))
	structural := make([]SymbolID, len(demands))
	for index := range demands {
		demand := &demands[index]
		location := demand.Location
		location.Path = filepath.Clean(location.Path)
		entities[index] = EntityFact{Location: location}
		if demand.Symbol {
			entities[index].Symbol = doubleSymbolID(location)
		}
		if demand.StructuralAccessor {
			structural[index] = entities[index].Symbol
		}
	}
	return entities, structural, nil
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
