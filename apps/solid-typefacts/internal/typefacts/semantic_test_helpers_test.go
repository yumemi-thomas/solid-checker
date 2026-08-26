package typefacts_test

import (
	"bytes"
	"context"
	"reflect"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func assertFullWireTransitionsIdentical(
	t testing.TB,
	what string,
	step int,
	projectID string,
	got, want *typefacts.FactTable,
) {
	t.Helper()
	encoder := &typefacts.WireTransitionEncoderForTest{}
	gotBytes, err := encoder.Full(projectID, got)
	if err != nil {
		t.Fatal(err)
	}
	wantBytes, err := encoder.Full(projectID, want)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(gotBytes, wantBytes) {
		gotSymbols := typefacts.SymbolFactsForTest(got)
		wantSymbols := typefacts.SymbolFactsForTest(want)
		t.Fatalf(
			"step %d (generation %d): %s diverges from fresh materialization "+
				"(%d vs %d transition bytes; got %d sources/%d entities/%d symbols/%d files, "+
				"want %d/%d/%d/%d; equal sources=%t entities=%t symbols=%t files=%t)\n"+
				"got symbols: %#v\nwant symbols: %#v",
			step, got.Generation, what, len(gotBytes), len(wantBytes),
			len(got.Sources), len(got.Entities), len(gotSymbols), len(got.Files),
			len(want.Sources), len(want.Entities), len(wantSymbols), len(want.Files),
			reflect.DeepEqual(got.Sources, want.Sources),
			reflect.DeepEqual(got.Entities, want.Entities),
			reflect.DeepEqual(gotSymbols, wantSymbols),
			reflect.DeepEqual(got.Files, want.Files),
			gotSymbols, wantSymbols,
		)
	}
}

func resolvedCall(t *testing.T, project typefacts.Project, location typefacts.Location) typefacts.Call {
	t.Helper()
	semantic, ok := project.(typefacts.SemanticEntityLookup)
	if !ok {
		t.Fatal("project does not implement SemanticEntityLookup")
	}
	entities, err := semantic.SemanticEntities(context.Background(), []typefacts.EntityDemand{{
		Location:     location,
		ResolvedCall: true,
	}})
	if err != nil {
		t.Fatal(err)
	}
	if len(entities) != 1 {
		t.Fatalf("SemanticEntities returned %d entities, want 1", len(entities))
	}
	if entities[0].ResolvedCall == nil {
		t.Fatal("SemanticEntities returned no resolved call")
	}
	return *entities[0].ResolvedCall
}
