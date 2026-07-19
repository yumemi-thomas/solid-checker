package typefacts

import (
	"reflect"
	"testing"
)

func TestFactTableDeltaMatchesFreshTablesAcrossEditDeleteAndDemandShrink(t *testing.T) {
	base := FactTableV2{
		Schema: 2, Generation: 1, ProjectID: "project",
		Sources: []SourceDigestV2{{Path: "a.ts", SHA256: "a"}, {Path: "b.ts", SHA256: "b"}},
		Entities: []EntityFactV2{
			{Location: LocationV2{Path: "a.ts", StartByte: 1, EndByte: 2}, Symbol: "a"},
			{Location: LocationV2{Path: "b.ts", StartByte: 1, EndByte: 2}, Symbol: "b"},
		},
		Symbols: []SymbolFactV2{{ID: "a"}, {ID: "b"}},
		Files:   []FileFactV2{{Path: "a.ts"}, {Path: "b.ts"}},
	}
	edit := FactTableV2{
		Schema: 2, Generation: 2, ProjectID: "project",
		Sources: []SourceDigestV2{{Path: "a.ts", SHA256: "a2"}, {Path: "b.ts", SHA256: "b"}},
		Entities: []EntityFactV2{
			{Location: LocationV2{Path: "a.ts", StartByte: 3, EndByte: 4}, Symbol: "a2"},
			{Location: LocationV2{Path: "b.ts", StartByte: 1, EndByte: 2}, Symbol: "b"},
		},
		Symbols: []SymbolFactV2{{ID: "a2"}, {ID: "b"}},
		Files:   []FileFactV2{{Path: "a.ts", Calls: []SourceCallV2{{Target: "a2"}}}, {Path: "b.ts"}},
	}
	deleted := FactTableV2{
		Schema: 2, Generation: 3, ProjectID: "project",
		Sources:  []SourceDigestV2{{Path: "a.ts", SHA256: "a2"}},
		Entities: []EntityFactV2{{Location: LocationV2{Path: "a.ts", StartByte: 3, EndByte: 4}, Symbol: "a2"}},
		Symbols:  []SymbolFactV2{{ID: "a2"}},
		Files:    []FileFactV2{{Path: "a.ts", Calls: []SourceCallV2{{Target: "a2"}}}},
	}
	shrunk := FactTableV2{
		Schema: 2, Generation: 4, ProjectID: "project",
		Sources:  []SourceDigestV2{{Path: "a.ts", SHA256: "a2"}},
		Entities: []EntityFactV2{},
		Symbols:  []SymbolFactV2{},
		Files:    []FileFactV2{{Path: "a.ts", Calls: []SourceCallV2{{Target: "a2"}}}},
	}

	retained := base
	for _, fresh := range []FactTableV2{edit, deleted, shrunk} {
		retained = ApplyFactTableDeltaV3(retained, DiffFactTablesV3(retained, fresh))
		if !reflect.DeepEqual(retained, fresh) {
			t.Fatalf("delta-applied retained table differs from fresh generation %d\nretained: %#v\nfresh: %#v", fresh.Generation, retained, fresh)
		}
	}
}

func TestInternalFactTableDeltaMatchesFullConversion(t *testing.T) {
	location := func(path string, start int) Location {
		return Location{Path: path, StartByte: start, EndByte: start + 1}
	}
	base := FactTable{
		Schema: 1, Generation: 1, ProjectID: "semantic-demand",
		Sources: []SourceFile{{Path: "a.ts", Source: []byte("a")}, {Path: "b.ts", Source: []byte("b")}},
		Entities: []EntityFact{
			{Location: location("a.ts", 1), Symbol: "a"},
			{Location: location("b.ts", 1), Symbol: "b"},
		},
		Symbols: []SymbolFact{{ID: "a"}, {ID: "b"}},
		Files:   []FileFact{{Path: "a.ts"}, {Path: "b.ts"}},
	}
	edit := FactTable{
		Schema: 1, Generation: 2, ProjectID: "semantic-demand",
		Sources: []SourceFile{{Path: "a.ts", Source: []byte("a2")}, {Path: "b.ts", Source: []byte("b")}},
		Entities: []EntityFact{
			{Location: location("a.ts", 3), Symbol: "a2"},
			{Location: location("b.ts", 1), Symbol: "b"},
		},
		Symbols: []SymbolFact{{ID: "a2"}, {ID: "b"}},
		Files:   []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}, {Path: "b.ts"}},
	}
	deleted := FactTable{
		Schema: 1, Generation: 3, ProjectID: "semantic-demand",
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{{Location: location("a.ts", 3), Symbol: "a2"}},
		Symbols:  []SymbolFact{{ID: "a2"}},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
	}
	shrunk := FactTable{
		Schema: 1, Generation: 4, ProjectID: "semantic-demand",
		Sources:  []SourceFile{{Path: "a.ts", Source: []byte("a2")}},
		Entities: []EntityFact{},
		Symbols:  []SymbolFact{},
		Files:    []FileFact{{Path: "a.ts", Calls: []SourceCall{{Target: "a2"}}}},
	}
	exactBuilder := &closureBuilder{
		referenceChangesExact: true,
		changedSymbolIDs:      map[SymbolID]struct{}{"a": {}, "a2": {}, "b": {}},
	}
	edit.transport = transportManifest(&base, &edit, exactBuilder, map[string]struct{}{"a.ts": {}})
	deleted.transport = transportManifest(&edit, &deleted, exactBuilder, map[string]struct{}{"b.ts": {}})
	shrunk.transport = transportManifest(&deleted, &shrunk, exactBuilder, map[string]struct{}{"a.ts": {}})

	previous := base
	retained := FactTableV2From(base, "project", 1)
	for _, freshInternal := range []FactTable{edit, deleted, shrunk} {
		delta := DiffFactTablesV3FromInternal(previous, freshInternal, freshInternal.Generation)
		retained = ApplyFactTableDeltaV3(retained, delta)
		fresh := FactTableV2From(freshInternal, "project", freshInternal.Generation)
		if !reflect.DeepEqual(retained, fresh) {
			t.Fatalf("direct delta differs from full conversion at generation %d\nretained: %#v\nfresh: %#v", fresh.Generation, retained, fresh)
		}
		previous = freshInternal
	}
}

func TestManifestDeltaDoesNotReplaceSharedReferenceRow(t *testing.T) {
	reference := func(path string, start int) Location {
		return Location{Path: path, StartByte: start, EndByte: start + 1}
	}
	base := FactTable{
		Schema: 1, Generation: 1, ProjectID: "shared-references",
		Symbols: []SymbolFact{{
			ID: "shared",
			References: []Location{
				reference("a.ts", 1),
				reference("b.ts", 1),
			},
		}},
	}
	edit := FactTable{
		Schema: 1, Generation: 2, ProjectID: "shared-references",
		Symbols: []SymbolFact{{
			ID: "shared",
			References: []Location{
				reference("a.ts", 3),
				reference("b.ts", 1),
			},
		}},
	}
	builder := &closureBuilder{
		referenceChangesExact: true,
		changedSymbolIDs:      map[SymbolID]struct{}{"shared": {}},
	}
	edit.transport = transportManifest(&base, &edit, builder, map[string]struct{}{"a.ts": {}})

	delta := DiffFactTablesV3FromInternal(base, edit, edit.Generation)
	if len(delta.Symbols) != 0 {
		t.Fatalf("delta replaced %d complete shared symbol rows", len(delta.Symbols))
	}
	if len(delta.SymbolReferenceFiles) != 1 {
		t.Fatalf("reference file deltas = %d, want 1", len(delta.SymbolReferenceFiles))
	}
	retained := ApplyFactTableDeltaV3(FactTableV2From(base, base.ProjectID, base.Generation), delta)
	fresh := FactTableV2From(edit, edit.ProjectID, edit.Generation)
	if !reflect.DeepEqual(retained, fresh) {
		t.Fatalf("reference-file delta differs from fresh table\nretained: %#v\nfresh: %#v", retained, fresh)
	}
}
