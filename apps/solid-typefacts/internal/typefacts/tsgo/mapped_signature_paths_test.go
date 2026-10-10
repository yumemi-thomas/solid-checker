package tsgo

import (
	"context"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

// A feature-dependent mapped type must not borrow a member from one possible
// instantiation. A concrete signature supplies that premise independently.
func TestMappedSignaturePathsRequireConcreteMemberPremise(t *testing.T) {
	dir := t.TempDir()
	source := `type State<F> = F extends { grouping: true } ? { grouping: string[] } : {};
type Atoms<F> = { [K in keyof State<F>]-?: { get(): State<F>[K] } };
type Cell<F> = { column: { table: { atoms: Atoms<F> } } };
declare function generic<F>(cell: Cell<F>): boolean;
declare function concrete(cell: Cell<{ grouping: true }>): boolean;
declare function empty(cell: Cell<{}>): boolean;
void generic;
void concrete;
void empty;
`
	writeInvocationProject(t, dir, map[string]string{"facts.ts": source})
	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	for _, name := range []string{"generic", "concrete", "empty"} {
		t.Run(name, func(t *testing.T) {
			start := strings.LastIndex(source, name)
			answer, err := opened.(typefacts.ExportValueAnalyzer).ExportValueTranscripts(context.Background(), []typefacts.ExportValueDemand{{
				Location: typefacts.Location{Path: filepath.Join(dir, "facts.ts"), StartByte: start, EndByte: start + len(name)}, CallableDepth: 5,
			}})
			if err != nil {
				t.Fatal(err)
			}
			if len(answer.Transcripts) != 1 || answer.Transcripts[0].CallSignature == nil {
				t.Fatal("exact declared signature unavailable")
			}
			signature := answer.Transcripts[0].CallSignature
			if len(signature.Parameters) != 1 {
				t.Fatal("expected one parameter")
			}
			var atoms, getter *typefacts.CallablePathFact
			for i := range signature.Parameters[0].CallablePaths {
				fact := &signature.Parameters[0].CallablePaths[i]
				names := []string{}
				for _, segment := range fact.Path {
					names = append(names, segment.Property)
				}
				switch strings.Join(names, ".") {
				case "column.table.atoms":
					atoms = fact
				case "column.table.atoms.grouping.get":
					getter = fact
				}
			}
			if atoms == nil {
				t.Fatal("expected exact atoms prefix")
			}
			if name == "concrete" {
				if getter == nil || !getter.Complete || getter.Presence != typefacts.PathRequired || getter.Callability != typefacts.CallabilityCallable {
					t.Fatalf("concrete getter premise = %+v", getter)
				}
			} else if getter != nil {
				t.Fatalf("%s signature invented getter premise: %+v", name, getter)
			}
		})
	}
}
