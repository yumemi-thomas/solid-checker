package typefacts

import (
	"context"
	"testing"
	"unicode/utf8"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/wirecbor"
)

// internalNameBackend answers every declaration with a TypeScript-internal
// symbol name (0xFE prefix, invalid UTF-8) and reports one changed reference.
type internalNameBackend struct {
	transportOnlyBackend
	id SymbolID
}

func (b internalNameBackend) Declarations(_ context.Context, id SymbolID) ([]Declaration, error) {
	location, ok := doubleSymbolLocation(id)
	if !ok {
		return nil, ErrNotFound
	}
	return []Declaration{{Name: "\xfeindex", Kind: "property", Location: location}}, nil
}

func (b internalNameBackend) ChangedReferences(context.Context) ([]SymbolID, bool, error) {
	return []SymbolID{b.id}, true, nil
}

// Every symbol name in a plain-CBOR response section must be valid UTF-8 and
// decode to the spelling the packed transition path produces.
func TestAnalyzeEvidenceEscapesInternalSymbolNames(t *testing.T) {
	t.Parallel()
	id := doubleSymbolID(Location{Path: "/project/source.ts", StartByte: 1, EndByte: 2})
	backend := internalNameBackend{
		transportOnlyBackend: newSessionTestBackend().transportOnlyBackend,
		id:                   id,
	}
	session, err := NewSession(backend, "/project/tsconfig.json", nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = session.Close() })
	ctx := context.Background()
	if open := session.Lifecycle(ctx, lifecycleRequest(1, LifecycleOpen, 1)); !open.OK {
		t.Fatalf("open = %+v", open)
	}
	request := lifecycleRequest(2, LifecycleAnalyze, 1)
	request.ResetState = true
	request.ReferenceChanges = true
	request.SymbolQueries = []SymbolQueryV6{{ID: id}}
	response := session.Lifecycle(ctx, request)
	if !response.OK {
		t.Fatalf("analyze = %+v", response)
	}
	for section, facts := range map[string][]SymbolFact{
		"symbolEvidence":    response.SymbolEvidence,
		"referenceEvidence": response.ReferenceEvidence,
	} {
		if len(facts) == 0 {
			t.Fatalf("%s is empty", section)
		}
		for _, fact := range facts {
			for _, declaration := range fact.Declarations {
				if !utf8.ValidString(declaration.Name) || declaration.Name != "__index" {
					t.Errorf("%s declaration name = %q, want \"__index\"", section, declaration.Name)
				}
			}
		}
	}
	encoded, err := wirecbor.Marshal(response)
	if err != nil {
		t.Fatal(err)
	}
	var decoded LifecycleResponse
	if err := wirecbor.Unmarshal(encoded, &decoded); err != nil {
		t.Fatalf("response does not decode: %v", err)
	}
}

func TestWireSymbolFactsDoesNotMutateRetainedFacts(t *testing.T) {
	t.Parallel()
	facts := []SymbolFact{{ID: "a", Declarations: []Declaration{{Name: "\xfecall"}}}, {ID: "b"}}
	got := wireSymbolFacts(facts)
	if got[0].Declarations[0].Name != "__call" {
		t.Fatalf("name = %q", got[0].Declarations[0].Name)
	}
	if facts[0].Declarations[0].Name != "\xfecall" {
		t.Fatal("retained fact was mutated")
	}
}
