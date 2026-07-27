package typefacts

import (
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"strings"
)

// Packed v3 full-frame encoding. This is deliberately an opaque byte string
// at the lifecycle seam: callers either receive a validated FactTableV2 or an
// error, and none of the columnar representation leaks into analysis code.
const packedFactTableVersion = 2

// Optional-field and boolean flag bits carried inside the packed frame. The
// Rust decoder declares the same values in crates/typefacts/src/v3.rs.
const bindingFlagArray = 1 << 0

const (
	functionFlagExported = 1 << 0
	functionFlagAsync    = 1 << 1
	functionFlagArrow    = 1 << 2
)

const asyncFunctionFlagCanReturnAsync = 1 << 0

// packedWriter appends varint-coded rows and interns every string it writes
// into one shared dictionary. Rows are buffered while the dictionary fills so
// the frame can emit the dictionary ahead of the rows that index into it.
type packedWriter struct {
	bytes []byte
	dict  *stringTableV3
}

func (w *packedWriter) u64(value uint64) {
	w.bytes = binary.AppendUvarint(w.bytes, value)
}

func (w *packedWriter) signed(value int64) {
	w.u64(uint64(value<<1) ^ uint64(value>>63))
}

func (w *packedWriter) raw(value []byte) {
	w.bytes = append(w.bytes, value...)
}

// text writes a string as its dictionary index, interning on first use.
func (w *packedWriter) text(value string) {
	w.u64(w.dict.intern(value))
}

type packedLocationState struct {
	path  uint64
	start uint64
	valid bool
}

func (w *packedWriter) location(location LocationV2, state *packedLocationState) {
	path := w.dict.intern(location.Path)
	if state.valid && state.path == path {
		w.u64(1)
		w.signed(int64(location.StartByte) - int64(state.start))
	} else {
		w.u64(path << 1)
		w.u64(location.StartByte)
	}
	w.u64(location.EndByte - location.StartByte)
	state.path = path
	state.start = location.StartByte
	state.valid = true
}

func (w *packedWriter) locations(locations []LocationV2) {
	w.u64(uint64(len(locations)))
	var state packedLocationState
	for _, location := range locations {
		w.location(location, &state)
	}
}

func (w *packedWriter) declarations(declarations []DeclarationV2) {
	w.u64(uint64(len(declarations)))
	var state packedLocationState
	for _, declaration := range declarations {
		w.text(declaration.Name)
		w.text(declaration.Kind)
		w.location(declaration.Location, &state)
	}
}

func (w *packedWriter) sourceCall(call SourceCallV2) {
	var state packedLocationState
	w.location(call.Location, &state)
	w.location(call.Callee, &state)
	w.locations(call.Arguments)
	w.text(call.Target)
}

// packedEntityGroups reports the length of each run of entities that share a
// source path. Paths map one-to-one onto dictionary indexes, so grouping by
// the paths themselves groups exactly as grouping by their indexes would.
func packedEntityGroups(entities []EntityFactV2) []int {
	var groups []int
	for index, entity := range entities {
		if index == 0 || entities[index-1].Location.Path != entity.Location.Path {
			groups = append(groups, 0)
		}
		groups[len(groups)-1]++
	}
	return groups
}

// PackedFactTableV3From encodes a full table into a validated, versioned
// columnar frame. Locations use per-list path elision, delta-coded starts,
// and lengths; optional fields use flags; source hashes use raw 32-byte
// digests; repeated strings use prefix coding.
//
// Rows are written straight from the v2 table while the string dictionary
// fills, so no intermediate row representation is materialized.
func PackedFactTableV3From(table FactTableV2) ([]byte, error) {
	rows := packedWriter{bytes: make([]byte, 0, 1<<20), dict: newStringTableV3()}

	rows.u64(uint64(len(table.Sources)))
	for _, source := range table.Sources {
		rows.text(source.Path)
		digest := strings.TrimPrefix(source.SHA256, "sha256:")
		if len(digest) != 64 {
			return nil, fmt.Errorf("packed source digest is not canonical: %q", source.SHA256)
		}
		raw := make([]byte, 32)
		if _, err := hex.Decode(raw, []byte(digest)); err != nil {
			return nil, fmt.Errorf("decode packed source digest: %w", err)
		}
		rows.raw(raw)
	}

	groups := packedEntityGroups(table.Entities)
	rows.u64(uint64(len(groups)))
	offset := 0
	for _, length := range groups {
		group := table.Entities[offset : offset+length]
		offset += length
		rows.text(group[0].Location.Path)
		rows.u64(uint64(length))
		var previousStart uint64
		for _, entity := range group {
			rows.signed(int64(entity.Location.StartByte) - int64(previousStart))
			rows.u64(entity.Location.EndByte - entity.Location.StartByte)
			rows.text(entity.Symbol)
			flags := uint64(0)
			if entity.TypeDescriptor != nil {
				flags |= 1
			}
			if entity.ResolvedCall != nil {
				flags |= 2
			}
			rows.u64(flags)
			if entity.TypeDescriptor != nil {
				rows.text(entity.TypeDescriptor.Text)
				rows.text(entity.TypeDescriptor.OriginModule)
				rows.declarations(entity.TypeDescriptor.AliasDeclarations)
			}
			if entity.ResolvedCall != nil {
				rows.text(entity.ResolvedCall.Target)
				rows.text(entity.ResolvedCall.ReturnTypeText)
			}
			previousStart = entity.Location.StartByte
		}
	}

	rows.u64(uint64(len(table.Symbols)))
	for _, symbol := range table.Symbols {
		rows.text(symbol.ID)
		rows.text(symbol.AliasTarget)
		rows.declarations(symbol.Declarations)
		rows.locations(symbol.References)
	}

	rows.u64(uint64(len(table.Files)))
	for _, file := range table.Files {
		// The dictionary is ordered by first use and a file interns its own
		// path after the rows it contains, so buffer the rows and emit the
		// path index in front of them. Reordering this changes the frame.
		body := packedWriter{dict: rows.dict}
		body.u64(uint64(len(file.Calls)))
		for _, call := range file.Calls {
			body.sourceCall(call)
		}
		body.u64(uint64(len(file.Bindings)))
		for _, binding := range file.Bindings {
			flags := uint64(0)
			if binding.Array {
				flags |= bindingFlagArray
			}
			body.u64(flags)
			body.locations(binding.Names)
			body.sourceCall(binding.Initializer)
		}
		body.u64(uint64(len(file.Functions)))
		for _, function := range file.Functions {
			var state packedLocationState
			body.location(function.Name, &state)
			body.location(function.Body, &state)
			body.locations(function.Parameters)
			flags := uint64(0)
			if function.Exported {
				flags |= functionFlagExported
			}
			if function.Async {
				flags |= functionFlagAsync
			}
			if function.Arrow {
				flags |= functionFlagArrow
			}
			body.u64(flags)
		}
		body.u64(uint64(len(file.AsyncFunctions)))
		for _, function := range file.AsyncFunctions {
			var state packedLocationState
			body.location(function.Expression, &state)
			body.text(function.Symbol)
			body.text(function.Target)
			flags := uint64(0)
			if function.CanReturnAsync {
				flags |= asyncFunctionFlagCanReturnAsync
			}
			body.u64(flags)
			body.locations(function.CallsAfterAwait)
		}
		rows.text(file.Path)
		rows.raw(body.bytes)
	}

	frame := packedWriter{bytes: make([]byte, 0, len(rows.bytes)+4096)}
	frame.u64(packedFactTableVersion)
	frame.u64(table.Schema)
	frame.u64(table.Generation)
	frame.u64(uint64(len(rows.dict.values)))
	previous := ""
	for _, value := range rows.dict.values {
		if digest, ok := packedHashedSymbol(value); ok {
			frame.u64(1)
			frame.raw(digest)
			continue
		}
		frame.u64(0)
		prefix := commonStringPrefix(previous, value)
		suffix := value[prefix:]
		frame.u64(uint64(prefix))
		frame.u64(uint64(len(suffix)))
		frame.raw([]byte(suffix))
		previous = value
	}
	frame.raw(rows.bytes)
	return frame.bytes, nil
}

func packedHashedSymbol(symbol string) ([]byte, bool) {
	const prefix = "symbol:h:"
	if !strings.HasPrefix(symbol, prefix) || len(symbol) != len(prefix)+24 {
		return nil, false
	}
	raw := make([]byte, 12)
	if _, err := hex.Decode(raw, []byte(symbol[len(prefix):])); err != nil {
		return nil, false
	}
	return raw, true
}

func commonStringPrefix(left, right string) int {
	limit := min(len(left), len(right))
	index := 0
	for index < limit && left[index] == right[index] {
		index++
	}
	return index
}
