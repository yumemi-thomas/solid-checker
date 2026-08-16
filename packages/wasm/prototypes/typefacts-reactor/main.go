// Command typefacts-reactor is a throwaway WASI reactor used to prove that
// the real TypeScript-Go Type Facts producer can run inside a browser worker.
package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"sort"
	"unsafe"

	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts"
	"github.com/yumemi-thomas/solid-ts-facts/internal/typefacts/tsgo"
)

type request struct {
	ProjectID  string                   `json:"projectId"`
	Generation uint64                   `json:"generation"`
	Demands    []typefacts.EntityDemand `json:"demands"`
}

type response struct {
	OK    bool            `json:"ok"`
	Table json.RawMessage `json:"table,omitempty"`
	Error string          `json:"error,omitempty"`
}

// sourceDigest is the source identity Rust's TypeScriptSnapshot expects. The
// producer's own SourceDigest is deliberately outside the public FactTable
// schema (internal/typefacts/model.go), so the reactor derives it here from
// the file bytes its TypeScript project resolved.
type sourceDigest struct {
	Path   string `json:"path"`
	SHA256 string `json:"sha256"`
}

// wireTable is what Rust deserializes. FactTable's JSON form cannot be used
// verbatim: the semantic path leaves FactTable.Sources nil and keeps the real
// digests in the unexported wire layer, so marshalling the table alone would
// leave the checker with no source identity at all. The reactor is the
// producer, so it emits that identity itself rather than letting the host
// fabricate it and certify its own answer.
type wireTable struct {
	Schema     uint64                 `json:"schema"`
	Generation uint64                 `json:"generation"`
	ProjectID  string                 `json:"projectId"`
	Sources    []sourceDigest         `json:"sources"`
	Entities   []typefacts.EntityFact `json:"entities"`
	Symbols    []typefacts.SymbolFact `json:"symbols"`
	Files      []typefacts.FileFact   `json:"files"`
}

var input []byte
var output []byte

//go:wasmexport allocate_input
func allocateInput(size uint32) uint32 {
	input = make([]byte, size)
	if len(input) == 0 {
		return 0
	}
	return uint32(uintptr(unsafe.Pointer(unsafe.SliceData(input))))
}

//go:wasmexport run_typefacts
func runTypeFacts() uint32 {
	result, err := analyze(input)
	if err != nil {
		output, _ = json.Marshal(response{Error: err.Error()})
		return 1
	}
	output, _ = json.Marshal(response{OK: true, Table: result})
	return 0
}

//go:wasmexport output_pointer
func outputPointer() uint32 {
	if len(output) == 0 {
		return 0
	}
	return uint32(uintptr(unsafe.Pointer(unsafe.SliceData(output))))
}

//go:wasmexport output_length
func outputLength() uint32 {
	return uint32(len(output))
}

func analyze(encoded []byte) ([]byte, error) {
	var request request
	if err := json.Unmarshal(encoded, &request); err != nil {
		return nil, err
	}
	backend, err := tsgo.OpenProject(context.Background(), request.ProjectID, nil)
	if err != nil {
		return nil, err
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		return nil, err
	}
	defer closure.Close()

	byPath := make(map[string][]typefacts.EntityDemand)
	for _, demand := range request.Demands {
		byPath[demand.Location.Path] = append(byPath[demand.Location.Path], demand)
	}
	paths := make([]string, 0, len(byPath))
	for path := range byPath {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	groups := make([]typefacts.DemandGroup, 0, len(paths))
	for _, path := range paths {
		groups = append(groups, typefacts.DemandGroup{Path: path, Demands: byPath[path]})
	}
	table, err := closure.DemandTableForGroups(
		context.Background(),
		request.Generation,
		groups,
		paths,
	)
	if err != nil {
		return nil, err
	}
	digests, err := sourceDigests(context.Background(), closure)
	if err != nil {
		return nil, err
	}
	return json.Marshal(wireTable{
		Schema:     table.Schema,
		Generation: table.Generation,
		// The project the reactor actually opened above, echoed from the
		// request the checker planned against.
		ProjectID: request.ProjectID,
		Sources:   digests,
		Entities:  table.Entities,
		Symbols:   table.Symbols,
		Files:     table.Files,
	})
}

// sourceDigests hashes the bytes the producer's own TypeScript project
// resolved, so ProjectFacts::join compares two independently derived views of
// source identity rather than the host's single view against itself.
func sourceDigests(ctx context.Context, closure *typefacts.DemandClosure) ([]sourceDigest, error) {
	sources, err := closure.SourceFiles(ctx)
	if err != nil {
		return nil, err
	}
	digests := make([]sourceDigest, 0, len(sources))
	for _, file := range sources {
		sum := sha256.Sum256(file.Source)
		digests = append(digests, sourceDigest{
			Path:   file.Path,
			SHA256: "sha256:" + hex.EncodeToString(sum[:]),
		})
	}
	sort.Slice(digests, func(left, right int) bool {
		return digests[left].Path < digests[right].Path
	})
	return digests, nil
}

func main() {}
