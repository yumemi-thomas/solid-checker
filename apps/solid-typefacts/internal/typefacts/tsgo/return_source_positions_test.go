package tsgo

import (
	"reflect"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestReturnSourcesStopAtArraySpread(t *testing.T) {
	for _, test := range []struct {
		name       string
		expression string
		paths      [][]int
	}{
		{"fixed", "[() => 1, () => 2]", [][]int{{0}, {1}}},
		{"hole", "[, () => 1]", [][]int{{1}}},
		{"shifted", "[...tail, () => 1]", nil},
		{"prefix", "[() => 1, ...tail, () => 2]", [][]int{{0}}},
		{"nested", "[[...tail, () => 1], () => 2]", [][]int{{1}}},
		{"emptySpreadStillOpen", "[...[], () => 1]", nil},
	} {
		t.Run(test.name, func(t *testing.T) {
			implementation := exportImplementationForSolidMake(t,
				"export function make(tail: unknown[]) { return "+test.expression+"; }\nvoid make;\n")
			if implementation.ControlFlow == nil || len(implementation.ControlFlow.Returns) != 1 {
				t.Fatal("expected one censused return")
			}
			var paths [][]int
			for _, source := range implementation.ControlFlow.Returns[0].Sources {
				if source.Kind != typefacts.ImplementationValueDirectCallable {
					t.Fatalf("unexpected source kind: %v", source.Kind)
				}
				var path []int
				for _, segment := range source.Path {
					if segment.Kind != typefacts.PathSegmentTuple || segment.Index == nil {
						t.Fatalf("expected exact tuple path: %#v", source.Path)
					}
					path = append(path, *segment.Index)
				}
				paths = append(paths, path)
			}
			if !reflect.DeepEqual(paths, test.paths) {
				t.Fatalf("return source paths = %v, want %v", paths, test.paths)
			}
		})
	}
}
