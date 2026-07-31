package typefacts

import (
	"errors"
	"fmt"
)

const wireTransitionVersion uint64 = 1
const maxRetainedWireTransitionBuffer = 1 << 20
const streamedWireTransitionChunk = 64 << 10

var errSparseTransitionUnavailable = errors.New("sparse transition requires an exact manifest")

type wireTransitionMode uint64

const (
	wireTransitionFull  wireTransitionMode = 0
	wireTransitionDelta wireTransitionMode = 1
)

func (m wireTransitionMode) String() string {
	switch m {
	case wireTransitionFull:
		return "full"
	case wireTransitionDelta:
		return "delta"
	default:
		return fmt.Sprintf("unknown(%d)", m)
	}
}

// wireTransitionInput is the complete state needed to encode one table
// transition. Base is nil for a full transition; delta transitions identify
// the retained client state with BaseStateToken.
type wireTransitionInput struct {
	ProjectID      string
	BaseStateToken string
	Base           *FactTable
	Target         *FactTable
	// Sparse emits unconditional upserts/removals for exact manifest keys.
	// The client owns the base table, so Go need not retain expanded base rows.
	Sparse bool
}

type encodedWireTransition struct {
	Bytes            []byte
	Mode             wireTransitionMode
	PathOperations   int
	SymbolOperations int
}

// TransitionArena receives one packed transition without taking ownership of
// Go memory. AppendRows receives bounded temporary chunks, and Finish prepends
// the compact header and dictionary logically before those rows.
type TransitionArena interface {
	AppendRows([]byte) error
	Finish([]byte) error
}

// wireTransitionEncoder is owned by one serialized Session. Its buffers and
// dictionary are reusable, but Encode returns a detached byte slice that the
// next call cannot overwrite.
type wireTransitionEncoder struct {
	tableSchema uint64
	dict        *stringTableV3

	rows  []byte
	frame []byte

	pathOps   []wireTransitionPathOp
	symbolOps []wireTransitionSymbolOp

	paths     []string
	symbolIDs []SymbolID
}

func (e *wireTransitionEncoder) Encode(input wireTransitionInput) (encodedWireTransition, error) {
	return e.encode(input, nil)
}

func (e *wireTransitionEncoder) EncodeInto(
	input wireTransitionInput,
	arena TransitionArena,
) (encodedWireTransition, error) {
	if arena == nil {
		return encodedWireTransition{}, errors.New("transition arena is nil")
	}
	return e.encode(input, arena)
}

func (e *wireTransitionEncoder) encode(
	input wireTransitionInput,
	arena TransitionArena,
) (encodedWireTransition, error) {
	mode, baseGeneration, err := validateWireTransitionInput(input)
	if err != nil {
		return encodedWireTransition{}, err
	}
	e.releasePlan()
	if mode == wireTransitionFull {
		e.planFull(input.Target)
	} else if input.Sparse {
		if input.Target.transport == nil || !input.Target.transport.exact {
			return encodedWireTransition{}, errSparseTransitionUnavailable
		}
		e.planSparseDelta(input.Base, input.Target)
	} else {
		e.planDelta(input.Base, input.Target)
	}
	pathOperationCount := len(e.pathOps)
	symbolOperationCount := len(e.symbolOps)
	if mode == wireTransitionFull {
		pathOperationCount = len(e.paths)
		symbolOperationCount = input.Target.symbolFactsCount()
	}
	defer e.releasePlan()
	if mode == wireTransitionDelta &&
		baseGeneration == input.Target.Generation &&
		pathOperationCount == 0 &&
		symbolOperationCount == 0 {
		return encodedWireTransition{Mode: mode}, nil
	}
	e.resetDictionary()
	defer e.resetDictionary()

	rows := packedWriter{bytes: e.rows[:0], dict: e.dict}
	if arena != nil {
		rows.bytes = make([]byte, 0, streamedWireTransitionChunk)
		rows.flush = arena.AppendRows
		rows.flushLimit = streamedWireTransitionChunk
	}
	rows.text(input.ProjectID)
	rows.text(input.BaseStateToken)
	if mode == wireTransitionFull {
		e.internFullOperationKeys(input.Target)
	} else {
		e.internOperationKeys()
	}
	rows.u64(uint64(pathOperationCount))
	if mode == wireTransitionFull {
		for index, path := range e.paths {
			operation, changed := wireTransitionPathDifference(
				nil,
				input.Target,
				path,
				true,
				true,
				true,
			)
			if !changed {
				e.rows = rows.bytes[:0]
				return encodedWireTransition{}, fmt.Errorf(
					"encode path operation %d: full path %q has no rows",
					index,
					path,
				)
			}
			if err := writeWireTransitionPathOp(&rows, &operation); err != nil {
				e.rows = rows.bytes[:0]
				return encodedWireTransition{}, fmt.Errorf("encode path operation %d: %w", index, err)
			}
		}
	} else {
		for index := range e.pathOps {
			if err := writeWireTransitionPathOp(&rows, &e.pathOps[index]); err != nil {
				e.rows = rows.bytes[:0]
				return encodedWireTransition{}, fmt.Errorf("encode path operation %d: %w", index, err)
			}
		}
	}
	rows.u64(uint64(symbolOperationCount))
	if mode == wireTransitionFull {
		index := 0
		var encodeErr error
		input.Target.rangeSymbolFacts(func(fact SymbolFact) {
			if encodeErr != nil {
				return
			}
			operation := wireTransitionSymbolOp{
				id: fact.ID, tag: wireTransitionReplaceSymbol, fact: fact,
			}
			if err := writeWireTransitionSymbolOp(&rows, mode, &operation); err != nil {
				encodeErr = fmt.Errorf("encode symbol operation %d: %w", index, err)
				return
			}
			index++
		})
		if encodeErr != nil {
			e.rows = rows.bytes[:0]
			return encodedWireTransition{}, encodeErr
		}
		if index != symbolOperationCount {
			e.rows = rows.bytes[:0]
			return encodedWireTransition{}, fmt.Errorf(
				"encoded %d full symbol operations, want %d",
				index,
				symbolOperationCount,
			)
		}
	} else {
		for index := range e.symbolOps {
			if err := writeWireTransitionSymbolOp(&rows, mode, &e.symbolOps[index]); err != nil {
				e.rows = rows.bytes[:0]
				return encodedWireTransition{}, fmt.Errorf("encode symbol operation %d: %w", index, err)
			}
		}
	}
	if err := rows.finish(); err != nil {
		e.rows = nil
		return encodedWireTransition{}, fmt.Errorf("stream transition rows: %w", err)
	}
	if arena == nil {
		e.rows = rows.bytes
	} else {
		e.rows = nil
	}

	frame := packedWriter{bytes: e.frame[:0]}
	frame.u64(wireTransitionVersion)
	frame.u64(uint64(mode))
	tableSchema := e.tableSchema
	if tableSchema == 0 {
		tableSchema = TypeFactsTableSchemaVersion
	}
	frame.u64(tableSchema)
	frame.u64(baseGeneration)
	frame.u64(input.Target.Generation)
	appendPackedDictionary(&frame, e.dict)
	if arena != nil {
		if err := arena.Finish(frame.bytes); err != nil {
			e.frame = frame.bytes[:0]
			return encodedWireTransition{}, fmt.Errorf("finish transition arena: %w", err)
		}
		if rows.flushed+len(frame.bytes) >= maxRetainedWireTransitionBuffer {
			// Match detachFrame's cold-lifecycle policy. The streamed adapter
			// never owns the complete frame, but its one-shot dictionary and
			// planning buckets are just as oversized for incremental work.
			e.dict = nil
			e.pathOps = nil
			e.symbolOps = nil
			e.paths = nil
			e.symbolIDs = nil
		}
		e.frame = frame.bytes[:0]
		return encodedWireTransition{
			Mode:             mode,
			PathOperations:   pathOperationCount,
			SymbolOperations: symbolOperationCount,
		}, nil
	}
	frame.raw(rows.bytes)

	owned := e.detachFrame(frame.bytes, rows.bytes)
	return encodedWireTransition{
		Bytes:            owned,
		Mode:             mode,
		PathOperations:   pathOperationCount,
		SymbolOperations: symbolOperationCount,
	}, nil
}

func (e *wireTransitionEncoder) planSparseDelta(base, target *FactTable) {
	manifest := target.transport
	for path := range manifest.sourcePaths {
		e.paths = append(e.paths, path)
	}
	for path := range manifest.entityPaths {
		e.paths = append(e.paths, path)
	}
	for path := range manifest.filePaths {
		e.paths = append(e.paths, path)
	}
	e.paths = sortUniqueWireTransitionPaths(e.paths)
	for _, path := range e.paths {
		rows := wireTransitionPathRowsAt(target, path)
		operation := wireTransitionPathOp{path: path}
		if _, candidate := manifest.sourcePaths[path]; candidate {
			if rows.hasSource {
				operation.sourceOp, operation.source = wireTransitionReplace, rows.source
			} else {
				operation.sourceOp = wireTransitionRemove
			}
		}
		if _, candidate := manifest.entityPaths[path]; candidate {
			if len(rows.entities) != 0 {
				operation.entityOp, operation.entities = wireTransitionReplace, rows.entities
			} else {
				operation.entityOp = wireTransitionRemove
			}
		}
		if _, candidate := manifest.filePaths[path]; candidate {
			if rows.hasFile {
				operation.fileOp, operation.file = wireTransitionReplace, rows.file
			} else {
				operation.fileOp = wireTransitionRemove
			}
		}
		e.pathOps = append(e.pathOps, operation)
	}
	// Expanded path rows moved to Rust, but Go deliberately retains the much
	// smaller canonical symbol store and reference runs. Preserve v5's exact
	// reference-path patches; replacing a high-fanout symbol wholesale turns a
	// one-file edit into a six-figure response.
	e.planExactSymbolDelta(base, target, manifest)
}

// detachFrame gives a large cold frame directly to the response pipeline.
// Retaining it would require a same-sized detached copy and pin both the frame
// and row scratch for the rest of the session. Small ordinary deltas keep the
// reusable-copy path, where avoiding fresh growth is faster.
func (e *wireTransitionEncoder) detachFrame(frame, rows []byte) []byte {
	if len(frame) >= maxRetainedWireTransitionBuffer {
		e.frame = nil
		if cap(rows) >= maxRetainedWireTransitionBuffer {
			e.rows = nil
		} else {
			e.rows = rows[:0]
		}
		// The cold dictionary is much larger than ordinary deltas. Clearing
		// entries retains its hash buckets and value backing, so discard the
		// whole table alongside the detached cold frame; the deferred reset
		// installs a fresh, minimal dictionary for incremental responses.
		e.dict = nil
		e.pathOps = nil
		e.symbolOps = nil
		e.paths = nil
		e.symbolIDs = nil
		return frame
	}
	owned := append([]byte(nil), frame...)
	e.frame = frame[:0]
	e.rows = rows[:0]
	return owned
}

func validateWireTransitionInput(input wireTransitionInput) (wireTransitionMode, uint64, error) {
	if input.Target == nil {
		return 0, 0, fmt.Errorf("wire transition target is nil")
	}
	if input.ProjectID == "" {
		return 0, 0, fmt.Errorf("wire transition project ID is empty")
	}
	if input.Target.Schema != TypeFactsSchemaVersion {
		return 0, 0, fmt.Errorf(
			"wire transition target schema = %d, want %d",
			input.Target.Schema,
			TypeFactsSchemaVersion,
		)
	}
	if input.Target.Generation == 0 {
		return 0, 0, fmt.Errorf("wire transition target generation is zero")
	}
	if input.Base == nil {
		if input.BaseStateToken != "" {
			return 0, 0, fmt.Errorf("full wire transition carries a base state token")
		}
		return wireTransitionFull, 0, nil
	}
	if input.Base.Schema != TypeFactsSchemaVersion {
		return 0, 0, fmt.Errorf(
			"wire transition base schema = %d, want %d",
			input.Base.Schema,
			TypeFactsSchemaVersion,
		)
	}
	if input.BaseStateToken == "" {
		return 0, 0, fmt.Errorf("delta wire transition has no base state token")
	}
	if input.Base.Generation == 0 {
		return 0, 0, fmt.Errorf("delta wire transition base generation is zero")
	}
	if input.Target.Generation < input.Base.Generation {
		return 0, 0, fmt.Errorf(
			"wire transition target generation %d precedes base generation %d",
			input.Target.Generation,
			input.Base.Generation,
		)
	}
	return wireTransitionDelta, input.Base.Generation, nil
}

func (e *wireTransitionEncoder) resetDictionary() {
	if e.dict == nil {
		e.dict = newStringTableV3()
		return
	}
	clear(e.dict.indexes)
	clear(e.dict.values)
	e.dict.values = e.dict.values[:1]
	e.dict.values[0] = ""
	e.dict.indexes[""] = 0
}

// internOperationKeys puts canonical keys next to one another in the prefix
// dictionary before replacement bodies introduce less-local strings.
func (e *wireTransitionEncoder) internOperationKeys() {
	for index := range e.pathOps {
		e.dict.intern(e.pathOps[index].path)
	}
	for index := range e.symbolOps {
		e.dict.intern(string(e.symbolOps[index].id))
	}
	for index := range e.symbolOps {
		if e.symbolOps[index].tag == wireTransitionReplaceReferencePath {
			e.dict.intern(e.symbolOps[index].referencePath)
		}
	}
}

func (e *wireTransitionEncoder) internFullOperationKeys(target *FactTable) {
	for _, path := range e.paths {
		e.dict.intern(path)
	}
	target.rangeSymbolFacts(func(fact SymbolFact) {
		e.dict.intern(string(fact.ID))
	})
}

// releasePlan preserves reusable backing arrays without retaining any table
// rows, source buffers, or strings through pointerful scratch entries.
func (e *wireTransitionEncoder) releasePlan() {
	clear(e.pathOps)
	e.pathOps = e.pathOps[:0]
	clear(e.symbolOps)
	e.symbolOps = e.symbolOps[:0]
	clear(e.paths)
	e.paths = e.paths[:0]
	clear(e.symbolIDs)
	e.symbolIDs = e.symbolIDs[:0]
}
