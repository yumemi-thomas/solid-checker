package typefacts_test

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/yumemi-thomas/solid-checker/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/internal/typefacts/tsgo"
)

func TestDescribeTypePreservesSolidAccessorAliasOrigin(t *testing.T) {
	root := filepath.Join("..", "engine", "testdata", "eslint-reactivity-v2")
	project, err := tsgo.OpenProject(context.Background(), filepath.Join(root, "tsconfig.json"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = project.Close() })
	describer := project.(typefacts.TypeDescriber)
	for _, fixture := range []struct{ name, call string }{{"effect-apply-parameter.tsx", "read()"}, {"effect-apply-member.tsx", "props.read()"}} {
		path := filepath.Join(root, fixture.name)
		source, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		start := strings.Index(string(source), fixture.call)
		if fixture.name == "effect-apply-member.tsx" {
			start += len("props.")
		}
		descriptor, err := describer.DescribeTypeAt(context.Background(), typefacts.Location{Path: path, StartByte: start, EndByte: start + len("read")})
		if err != nil {
			t.Fatal(err)
		}
		if len(descriptor.AliasDeclarations) == 0 || descriptor.AliasDeclarations[0].Name != "Accessor" {
			t.Errorf("%s descriptor = %#v", fixture.name, descriptor)
		}
	}
}
