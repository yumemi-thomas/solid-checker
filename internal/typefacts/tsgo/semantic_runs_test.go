package tsgo

import (
	"context"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

type semanticDemandRunner interface {
	SemanticDemandRuns(
		context.Context,
		[]typefacts.SemanticDemandRun,
		typefacts.SemanticScope,
	) ([]typefacts.SemanticDemandRunResult, error)
}

func TestSemanticDemandRunsAlignsResultsAndReportsRetentionEvidence(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"strict":true,"module":"esnext","moduleResolution":"bundler","target":"esnext"},"include":["*.ts"]}`)
	typesPath := write("types.ts", `export type Handler = (value: number) => string;`)
	libraryPath := write("library.ts", `import type { Handler } from "./types";
export function invoke(callback: Handler): string { return callback(1); }
`)
	alphaSource := `import { invoke } from "./library";
export const alpha = invoke(value => String(value));
export const alphaAgain = invoke(value => String(value));
`
	alphaPath := write("alpha.ts", alphaSource)
	betaSource := `type Calls<T extends string> = { [K in T]: (value: number) => number };
declare const mapped: Calls<"run">;
export const beta = mapped.run(1);
`
	betaPath := write("beta.ts", betaSource)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	runner, ok := opened.(semanticDemandRunner)
	if !ok {
		t.Fatal("project does not implement SemanticDemandRuns")
	}

	callDemand := func(path, needle string, start int) typefacts.EntityDemand {
		t.Helper()
		if start < 0 {
			t.Fatalf("%q not found in %s", needle, path)
		}
		return typefacts.EntityDemand{
			Location: typefacts.Location{
				Path:      path,
				StartByte: start,
				EndByte:   start + len(needle),
			},
			ResolvedCall: true,
		}
	}
	betaNeedle := "mapped.run(1)"
	alphaNeedle := "invoke(value => String(value))"
	firstAlpha := strings.Index(alphaSource, alphaNeedle)
	secondAlpha := strings.LastIndex(alphaSource, alphaNeedle)
	runs := []typefacts.SemanticDemandRun{
		{
			Path: betaPath,
			Demands: []typefacts.EntityDemand{
				callDemand(betaPath, betaNeedle, strings.Index(betaSource, betaNeedle)),
			},
		},
		{
			Path: alphaPath,
			Demands: []typefacts.EntityDemand{
				callDemand(alphaPath, alphaNeedle, firstAlpha),
				callDemand(alphaPath, alphaNeedle, secondAlpha),
			},
		},
	}
	results, err := runner.SemanticDemandRuns(context.Background(), runs, typefacts.SemanticScope{})
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != len(runs) {
		t.Fatalf("results = %d, want %d", len(results), len(runs))
	}
	for runIndex := range runs {
		result := results[runIndex]
		if len(result.Entities) != len(runs[runIndex].Demands) ||
			len(result.Structural) != len(runs[runIndex].Demands) {
			t.Fatalf("run %d alignment = %d entities, %d structural; want %d each",
				runIndex, len(result.Entities), len(result.Structural), len(runs[runIndex].Demands))
		}
		for demandIndex := range runs[runIndex].Demands {
			if got, want := result.Entities[demandIndex].Location, runs[runIndex].Demands[demandIndex].Location; got != want {
				t.Errorf("run %d demand %d location = %+v, want %+v", runIndex, demandIndex, got, want)
			}
		}
	}

	alpha := results[1]
	if want := []string{filepath.Clean(libraryPath), filepath.Clean(typesPath)}; !reflect.DeepEqual(alpha.Dependencies, want) {
		t.Fatalf("alpha dependencies = %v, want %v", alpha.Dependencies, want)
	}
	call := alpha.Entities[0].ResolvedCall
	if call == nil || call.Declaration == nil || len(call.Arguments) != 1 ||
		call.Arguments[0].Parameter == nil || call.Arguments[0].Parameter.Declaration == nil ||
		call.Arguments[0].Parameter.TypeDescriptor == nil {
		t.Fatalf("resolved call lacks declaration, parameter, or parameter type evidence: %+v", call)
	}
	if got := filepath.Clean(call.Declaration.Location.Path); got != filepath.Clean(libraryPath) {
		t.Errorf("resolved declaration path = %q, want %q", got, libraryPath)
	}
	if got := filepath.Clean(call.Arguments[0].Parameter.Declaration.Location.Path); got != filepath.Clean(libraryPath) {
		t.Errorf("parameter declaration path = %q, want %q", got, libraryPath)
	}
	aliasPaths := make([]string, 0, len(call.Arguments[0].Parameter.TypeDescriptor.AliasDeclarations))
	for _, declaration := range call.Arguments[0].Parameter.TypeDescriptor.AliasDeclarations {
		aliasPaths = append(aliasPaths, filepath.Clean(declaration.Location.Path))
	}
	if !reflect.DeepEqual(aliasPaths, []string{filepath.Clean(typesPath)}) {
		t.Errorf("parameter type alias paths = %v, want [%s]", aliasPaths, typesPath)
	}

	for resultIndex := range results {
		ids := embeddedSemanticIDs(results[resultIndex])
		wantDurable := true
		for _, id := range ids {
			if !typefacts.DurableSymbolID(id) {
				wantDurable = false
				break
			}
		}
		if results[resultIndex].Durable != wantDurable {
			t.Errorf("result %d Durable = %t, embedded IDs %v imply %t",
				resultIndex, results[resultIndex].Durable, ids, wantDurable)
		}
		if resultIndex == 0 && wantDurable {
			t.Fatalf("mapped call embedded only durable IDs %v; non-durable coverage is vacuous", ids)
		}
		if resultIndex == 1 && !wantDurable {
			t.Fatalf("source-backed call embedded non-durable IDs %v; durable coverage is vacuous", ids)
		}
	}
}

func TestSemanticDemandRunsRejectsCrossFileQueryLocation(t *testing.T) {
	dir := t.TempDir()
	write := writeProject(t, dir)
	write("tsconfig.json", `{"compilerOptions":{"strict":true},"include":["*.ts"]}`)
	runPath := write("run.ts", `export const value = 1;`)
	otherPath := write("other.ts", `export const other = 2;`)

	opened, err := OpenProject(context.Background(), filepath.Join(dir, "tsconfig.json"), nil)
	if err != nil {
		t.Fatal(err)
	}
	defer opened.Close()
	runner, ok := opened.(semanticDemandRunner)
	if !ok {
		t.Fatal("project does not implement SemanticDemandRuns")
	}

	query := typefacts.Location{Path: otherPath}
	_, err = runner.SemanticDemandRuns(context.Background(), []typefacts.SemanticDemandRun{{
		Path: runPath,
		Demands: []typefacts.EntityDemand{{
			Location:      typefacts.Location{Path: runPath},
			QueryLocation: &query,
			Symbol:        true,
		}},
	}}, typefacts.SemanticScope{})
	if err == nil || !strings.Contains(err.Error(), "query location") ||
		!strings.Contains(err.Error(), runPath) || !strings.Contains(err.Error(), otherPath) {
		t.Fatalf("SemanticDemandRuns error = %v, want cross-file query-location rejection", err)
	}
}

func embeddedSemanticIDs(result typefacts.SemanticDemandRunResult) []typefacts.SymbolID {
	ids := append([]typefacts.SymbolID(nil), result.Structural...)
	for _, entity := range result.Entities {
		ids = append(ids, entity.Symbol)
		call := entity.ResolvedCall
		if call == nil {
			continue
		}
		ids = append(ids, call.Target)
		if call.Declaration != nil {
			ids = append(ids, call.Declaration.Symbol)
			for _, owner := range call.Declaration.Owners {
				ids = append(ids, owner.Symbol)
			}
		}
		for _, mapping := range call.Arguments {
			if mapping.Parameter != nil {
				ids = append(ids, mapping.Parameter.Symbol)
			}
		}
	}
	return ids
}
