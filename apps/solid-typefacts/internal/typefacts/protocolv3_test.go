package typefacts_test

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
)

func TestTypeFactsSchemaHashMatchesFrozenSchema(t *testing.T) {
	for _, schema := range []struct {
		name string
		hash string
	}{
		{"typefacts-v1.schema.json", typefacts.TypeFactsSchemaSHA256},
	} {
		data, err := os.ReadFile(filepath.Join("..", "..", "..", "..", "schema", schema.name))
		if err != nil {
			t.Fatal(err)
		}
		actual := fmt.Sprintf("sha256:%x", sha256.Sum256(data))
		if actual != schema.hash {
			t.Fatalf("%s hash = %q, handshake declares %q", schema.name, actual, schema.hash)
		}
	}
}

// The handshake refuses on protocol, digest, and build id together, so a
// producer and a client have always had to ship as a pair. This pins the two
// halves the repository owns: the protocol number the operation set implies,
// and the fact that the digest above is the schema file's. The third, the build
// id, is stamped at link time and is covered by the Rust process tests.
func TestHandshakeDeclaresTheOperationSetsProtocol(t *testing.T) {
	if typefacts.TypeFactsHandshakeProtocol != 9 {
		t.Fatalf(
			"handshake protocol = %d, want 9: protocol 9 separates callable-path local shape from subtree enumeration",
			typefacts.TypeFactsHandshakeProtocol,
		)
	}
}

func TestLifecycleInvocationsIsAValidReadOnlyGenerationOperation(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV1,
		RequestID:  1,
		Operation:  typefacts.LifecycleInvocations,
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
		InvocationDemands: []typefacts.InvocationDemand{{
			Location:      typefacts.Location{Path: "/project/source.ts", StartByte: 1, EndByte: 8},
			CallableDepth: 2,
			Census:        true,
		}},
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		t.Fatal(err)
	}
}

func TestLifecycleModulesIsAValidReadOnlyGenerationOperation(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV1,
		RequestID:  1,
		Operation:  typefacts.LifecycleModules,
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
		ModuleGraph: &typefacts.ModuleInventoryDemand{
			Imports:  true,
			Packages: true,
		},
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		t.Fatal(err)
	}
}

// A producer that predates the modules operation rejects the request outright
// rather than answering an empty graph. The handshake refuses such a pair long
// before this, but the operation validator must not be the thing that softens
// it if it ever does not.
func TestAnUnknownLifecycleOperationIsRefused(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV1,
		RequestID:  1,
		Operation:  typefacts.LifecycleOperation("moduleGraph"),
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
	}
	if err := typefacts.ValidateLifecycleRequest(request); err == nil {
		t.Fatal("an unknown operation was accepted")
	}
}

func TestLifecycleSourcesIsAValidReadOnlyGenerationOperation(t *testing.T) {
	request := typefacts.LifecycleRequest{
		Schema:     typefacts.TypeFactsSchemaVersionV1,
		RequestID:  1,
		Operation:  typefacts.LifecycleSources,
		ProjectID:  "/project/tsconfig.json",
		Generation: 1,
	}
	if err := typefacts.ValidateLifecycleRequest(request); err != nil {
		t.Fatal(err)
	}
}
