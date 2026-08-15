package typefacts_test

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
)

func TestTypeFactsSchemaHashMatchesFrozenSchema(t *testing.T) {
	for _, schema := range []struct {
		name string
		hash string
	}{
		{"typefacts-v5.schema.json", typefacts.TypeFactsSchemaSHA256},
		{"typefacts-v6.schema.json", typefacts.TypeFactsSchemaV6SHA256},
		{"typefacts-v7.schema.json", typefacts.TypeFactsSchemaV7SHA256},
		{"typefacts-v8.schema.json", typefacts.TypeFactsSchemaV8SHA256},
		{"typefacts-v9.schema.json", typefacts.TypeFactsSchemaV9SHA256},
	} {
		data, err := os.ReadFile(filepath.Join("..", "..", "schema", schema.name))
		if err != nil {
			t.Fatal(err)
		}
		actual := fmt.Sprintf("sha256:%x", sha256.Sum256(data))
		if actual != schema.hash {
			t.Fatalf("%s hash = %q, handshake declares %q", schema.name, actual, schema.hash)
		}
	}
}

func TestLifecycleSourcesIsAValidReadOnlyGenerationOperation(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV5,
		RequestID:  1,
		Operation:  typefacts.LifecycleSources,
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		t.Fatal(err)
	}
}
