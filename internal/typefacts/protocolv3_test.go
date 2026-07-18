package typefacts_test

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-check/internal/typefacts"
)

func TestTypeFactsSchemaHashMatchesFrozenSchema(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "..", "schema", "typefacts-v2.schema.json"))
	if err != nil {
		t.Fatal(err)
	}
	actual := fmt.Sprintf("sha256:%x", sha256.Sum256(data))
	if actual != typefacts.TypeFactsSchemaSHA256 {
		t.Fatalf("schema hash = %q, handshake declares %q", actual, typefacts.TypeFactsSchemaSHA256)
	}
}

func TestLifecycleSourcesIsAValidReadOnlyGenerationOperation(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV3,
		RequestID:  1,
		Operation:  typefacts.LifecycleSources,
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		t.Fatal(err)
	}
}
