package typefacts

import (
	"context"
	"strings"
	"testing"
)

type unorderedSourceBackend struct {
	transportOnlyBackend
	sources []SourceFile
}

func (b unorderedSourceBackend) SourceFiles(context.Context) ([]SourceFile, error) {
	return append([]SourceFile(nil), b.sources...), nil
}

func TestDemandClosureRejectsNoncanonicalAdapterSources(t *testing.T) {
	a := SourceFile{Path: "/project/a.ts", Source: []byte("a")}
	b := SourceFile{Path: "/project/b.ts", Source: []byte("b")}
	closure, err := NewDemandClosure(unorderedSourceBackend{
		transportOnlyBackend: transportOnlyBackend{source: a},
		sources:              []SourceFile{b, a},
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	_, err = closure.DemandTableForGroups(context.Background(), 1, []DemandGroup{
		{
			Path: a.Path,
			Demands: []EntityDemand{
				{Location: Location{Path: a.Path, StartByte: 4, EndByte: 5}, Symbol: true},
			},
		},
	}, nil)
	if err == nil || !strings.Contains(err.Error(), "strictly path-ordered") {
		t.Fatalf("noncanonical adapter sources error = %v", err)
	}
}

func TestDemandClosureCanonicalizesDemandAtInputBoundary(t *testing.T) {
	source := SourceFile{Path: "/project/a.ts", Source: []byte("a")}
	closure, err := NewDemandClosure(transportOnlyBackend{source: source}, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	table, err := closure.DemandTableForGroups(context.Background(), 1, []DemandGroup{{
		Path: source.Path,
		Demands: []EntityDemand{
			{Location: Location{Path: source.Path, StartByte: 10, EndByte: 11}},
			{Location: Location{Path: source.Path, StartByte: 1, EndByte: 2}},
		},
	}}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(table.entityRuns) != 1 || len(table.entityRuns[0].entities) != 2 ||
		table.entityRuns[0].entities[0].Location.StartByte != 1 ||
		table.entityRuns[0].entities[1].Location.StartByte != 10 {
		t.Fatalf("canonicalized entity runs = %#v", table.entityRuns)
	}
}

func TestDemandClosureRejectsDuplicateDemandGroupPaths(t *testing.T) {
	source := SourceFile{Path: "/project/a.ts", Source: []byte("a")}
	closure, err := NewDemandClosure(transportOnlyBackend{source: source}, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = closure.Close() })

	_, err = closure.DemandTableForGroups(context.Background(), 1, []DemandGroup{
		{Path: source.Path},
		{Path: source.Path},
	}, nil)
	if err == nil || !strings.Contains(err.Error(), "each path may appear once") {
		t.Fatalf("duplicate groups error = %v", err)
	}
}

func TestPrepareRetainedContributionRejectsMisalignedEntity(t *testing.T) {
	path := "/project/a.ts"
	demand := EntityDemand{Location: Location{Path: path, StartByte: 1, EndByte: 2}}
	_, err := prepareRetainedContribution(path, 1, []EntityDemand{demand}, SemanticDemandRunResult{
		Entities:   []EntityFact{{Location: Location{Path: path, StartByte: 2, EndByte: 3}}},
		Structural: []SymbolID{""},
		Durable:    true,
	})
	if err == nil || !strings.Contains(err.Error(), "entity 0 location") {
		t.Fatalf("misaligned entity error = %v", err)
	}
}

func TestPrepareRetainedContributionRejectsNoncanonicalDependencies(t *testing.T) {
	path := "/project/a.ts"
	demand := EntityDemand{Location: Location{Path: path, StartByte: 1, EndByte: 2}}
	_, err := prepareRetainedContribution(path, 1, []EntityDemand{demand}, SemanticDemandRunResult{
		Entities:     []EntityFact{{Location: demand.Location}},
		Structural:   []SymbolID{""},
		Dependencies: []string{"/project/z.ts", "/project/b.ts"},
		Durable:      true,
	})
	if err == nil || !strings.Contains(err.Error(), "not strictly ordered") {
		t.Fatalf("noncanonical dependencies error = %v", err)
	}
}
