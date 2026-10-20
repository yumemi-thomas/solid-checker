// Command typefacts-reactor is a throwaway WASI reactor used to prove that
// the real TypeScript-Go Type Facts producer can run inside a browser worker.
//
// Built for wasip1 it is a reactor: the four exports below are its whole
// interface and main is empty (main_wasip1.go). Built for any other target it
// is a stdin/stdout command (main_native.go) that answers one request with the
// same analysis and the same response encoder, so the two builds can be
// compared byte for byte on the same project. Neither build is a maintained
// producer or a source of certification authority.
package main

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"sort"
	"unsafe"

	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts"
	"github.com/yumemi-thomas/solid-checker/apps/solid-typefacts/internal/typefacts/tsgo"
)

type request struct {
	ProjectID  string                   `json:"projectId"`
	Generation uint64                   `json:"generation"`
	Demands    []typefacts.EntityDemand `json:"demands"`
	// ModuleGraph, when present, asks for the resolved module graph of the
	// same program that answers Demands. It is passed to the canonical
	// DemandClosure.ModuleGraph verbatim: the zero demand is the inventory
	// alone, and Imports with no ImportPaths covers every program file,
	// exactly as that owner defines them.
	ModuleGraph *typefacts.ModuleInventoryDemand `json:"moduleGraph,omitempty"`
}

type response struct {
	OK    bool            `json:"ok"`
	Table json.RawMessage `json:"table,omitempty"`
	// ModuleGraph is the canonical inventory marshalled as the producer
	// returned it, present exactly when the request asked for it. It is a
	// sibling of Table and never inside it: Rust's TypeScriptSnapshot denies
	// unknown fields.
	ModuleGraph *typefacts.ModuleInventory `json:"moduleGraph,omitempty"`
	Error       string                     `json:"error,omitempty"`
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
	var status uint32
	output, status = respond(input)
	return status
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

// respond answers one encoded request. It is the single response encoder of
// both builds.
func respond(encoded []byte) ([]byte, uint32) {
	table, graph, err := analyze(encoded)
	return encode(table, graph, err)
}

// encode writes a failure with no table and no graph, or a success carrying
// both answers of one program. Status 0 is success.
func encode(table json.RawMessage, graph *typefacts.ModuleInventory, err error) ([]byte, uint32) {
	if err == nil {
		encoded, marshalErr := json.Marshal(response{OK: true, Table: table, ModuleGraph: graph})
		if marshalErr == nil {
			return encoded, 0
		}
		err = marshalErr
	}
	encoded, _ := json.Marshal(response{Error: err.Error()})
	return encoded, 1
}

// closureAnswers is what the response is assembled from: one opened
// DemandClosure. Every answer in a success comes from the same value, so from
// the same program.
type closureAnswers interface {
	DemandTableForGroups(
		ctx context.Context,
		generation uint64,
		groups []typefacts.DemandGroup,
		changedPaths []string,
	) (*typefacts.FactTable, error)
	SourceFiles(ctx context.Context) ([]typefacts.SourceFile, error)
	ModuleGraph(ctx context.Context, demand typefacts.ModuleInventoryDemand) (typefacts.ModuleInventory, error)
}

var _ closureAnswers = (*typefacts.DemandClosure)(nil)

func analyze(encoded []byte) (json.RawMessage, *typefacts.ModuleInventory, error) {
	var request request
	if err := json.Unmarshal(encoded, &request); err != nil {
		return nil, nil, err
	}
	backend, err := tsgo.OpenProject(context.Background(), request.ProjectID, nil)
	if err != nil {
		return nil, nil, err
	}
	closure, err := typefacts.NewDemandClosure(backend, nil)
	if err != nil {
		return nil, nil, err
	}
	defer closure.Close()
	return assemble(context.Background(), request, closure)
}

// assemble answers request from closure. A failure of any answer fails the
// whole response, so no success ever carries a partial table or graph.
func assemble(
	ctx context.Context,
	request request,
	closure closureAnswers,
) (json.RawMessage, *typefacts.ModuleInventory, error) {
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
		ctx,
		request.Generation,
		groups,
		paths,
	)
	if err != nil {
		return nil, nil, err
	}
	digests, err := sourceDigests(ctx, closure)
	if err != nil {
		return nil, nil, err
	}
	// The retained table keeps its entity rows in the closure's own per-file
	// runs, not in the public Entities slice, so they are read through the
	// owner's canonical projection. The collected values borrow the table's
	// nested storage: they are marshalled at once, with no closure call in
	// between, and only the marshalled bytes outlive this block. Symbols stay
	// as the owner leaves them (nil): Rust performs the symbol closure.
	var entities []typefacts.EntityFact
	table.RangeEntities(func(entity typefacts.EntityFact) {
		entities = append(entities, entity)
	})
	encodedTable, err := json.Marshal(wireTable{
		Schema:     table.Schema,
		Generation: table.Generation,
		// The project the reactor actually opened above, echoed from the
		// request the checker planned against.
		ProjectID: request.ProjectID,
		Sources:   digests,
		Entities:  entities,
		Symbols:   table.Symbols,
		Files:     table.Files,
	})
	if err != nil {
		return nil, nil, err
	}
	if request.ModuleGraph == nil {
		return encodedTable, nil, nil
	}
	inventory, err := closure.ModuleGraph(ctx, *request.ModuleGraph)
	if err != nil {
		return nil, nil, err
	}
	return encodedTable, &inventory, nil
}

// sourceDigests hashes the bytes the producer's own TypeScript project
// resolved, so ProjectFacts::join compares two independently derived views of
// source identity rather than the host's single view against itself.
func sourceDigests(ctx context.Context, closure closureAnswers) ([]sourceDigest, error) {
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
