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

type packedWriter struct {
	bytes []byte
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

type packedLocationState struct {
	path  uint64
	start uint64
	valid bool
}

func (w *packedWriter) location(location CompactLocationV3, state *packedLocationState) {
	samePath := state.valid && state.path == location.Path
	if samePath {
		w.u64(1)
		w.signed(int64(location.StartByte) - int64(state.start))
	} else {
		w.u64(location.Path << 1)
		w.u64(location.StartByte)
	}
	w.u64(location.EndByte - location.StartByte)
	state.path = location.Path
	state.start = location.StartByte
	state.valid = true
}

func (w *packedWriter) locations(locations []CompactLocationV3) {
	w.u64(uint64(len(locations)))
	var state packedLocationState
	for _, location := range locations {
		w.location(location, &state)
	}
}

func (w *packedWriter) declaration(declaration CompactDeclarationV3, state *packedLocationState) {
	w.u64(declaration.Name)
	w.u64(declaration.Kind)
	w.location(declaration.Location, state)
}

func (w *packedWriter) declarations(declarations []CompactDeclarationV3) {
	w.u64(uint64(len(declarations)))
	var state packedLocationState
	for _, declaration := range declarations {
		w.declaration(declaration, &state)
	}
}

func (w *packedWriter) sourceCall(call CompactSourceCallV3) {
	var state packedLocationState
	w.location(call.Location, &state)
	w.location(call.Callee, &state)
	w.locations(call.Arguments)
	w.u64(call.Target)
}

// PackedFactTableV3From encodes a full table into a validated, versioned
// columnar frame. Locations use per-list path elision, delta-coded starts,
// and lengths; optional fields use flags; source hashes use raw 32-byte
// digests; repeated strings use prefix coding.
func PackedFactTableV3From(table FactTableV2) ([]byte, error) {
	compact := CompactFactTableV3From(table)
	w := packedWriter{bytes: make([]byte, 0, 1<<20)}
	w.u64(packedFactTableVersion)
	w.u64(compact.Schema)
	w.u64(compact.Generation)

	w.u64(uint64(len(compact.Strings)))
	previous := ""
	for _, value := range compact.Strings {
		if digest, ok := packedHashedSymbol(value); ok {
			w.u64(1)
			w.raw(digest)
		} else {
			w.u64(0)
			prefix := commonStringPrefix(previous, value)
			suffix := value[prefix:]
			w.u64(uint64(prefix))
			w.u64(uint64(len(suffix)))
			w.raw([]byte(suffix))
			previous = value
		}
	}

	w.u64(uint64(len(compact.Sources)))
	for _, source := range compact.Sources {
		w.u64(source.Path)
		digest := strings.TrimPrefix(source.SHA256, "sha256:")
		if len(digest) != 64 {
			return nil, fmt.Errorf("packed source digest is not canonical: %q", source.SHA256)
		}
		raw := make([]byte, 32)
		if _, err := hex.Decode(raw, []byte(digest)); err != nil {
			return nil, fmt.Errorf("decode packed source digest: %w", err)
		}
		w.raw(raw)
	}

	w.u64(uint64(len(compact.EntityFiles)))
	for _, file := range compact.EntityFiles {
		w.u64(file.Path)
		w.u64(uint64(len(file.Entities)))
		var previousStart uint64
		for _, entity := range file.Entities {
			w.signed(int64(entity.StartByte) - int64(previousStart))
			w.u64(entity.EndByte - entity.StartByte)
			w.u64(entity.Symbol)
			flags := uint64(0)
			if len(entity.TypeDescriptor) != 0 {
				flags |= 1
			}
			if len(entity.ResolvedCall) != 0 {
				flags |= 2
			}
			w.u64(flags)
			if len(entity.TypeDescriptor) > 1 || len(entity.ResolvedCall) > 1 {
				return nil, fmt.Errorf("packed entity optional field has multiple rows")
			}
			if len(entity.TypeDescriptor) == 1 {
				descriptor := entity.TypeDescriptor[0]
				w.u64(descriptor.Text)
				w.u64(descriptor.OriginModule)
				w.declarations(descriptor.AliasDeclarations)
			}
			if len(entity.ResolvedCall) == 1 {
				call := entity.ResolvedCall[0]
				w.u64(call.Target)
				w.u64(call.ReturnTypeText)
			}
			previousStart = entity.StartByte
		}
	}

	w.u64(uint64(len(compact.Symbols)))
	for _, symbol := range compact.Symbols {
		w.u64(symbol.ID)
		w.u64(symbol.AliasTarget)
		w.declarations(symbol.Declarations)
		w.locations(symbol.References)
	}

	w.u64(uint64(len(compact.Files)))
	for _, file := range compact.Files {
		w.u64(file.Path)
		w.u64(uint64(len(file.Calls)))
		for _, call := range file.Calls {
			w.sourceCall(call)
		}
		w.u64(uint64(len(file.Bindings)))
		for _, binding := range file.Bindings {
			w.u64(binding.Flags)
			w.locations(binding.Names)
			w.sourceCall(binding.Initializer)
		}
		w.u64(uint64(len(file.Functions)))
		for _, function := range file.Functions {
			var state packedLocationState
			w.location(function.Name, &state)
			w.location(function.Body, &state)
			w.locations(function.Parameters)
			w.u64(function.Flags)
		}
		w.u64(uint64(len(file.AsyncFunctions)))
		for _, function := range file.AsyncFunctions {
			var state packedLocationState
			w.location(function.Expression, &state)
			w.u64(function.Symbol)
			w.u64(function.Target)
			w.u64(function.Flags)
			w.locations(function.CallsAfterAwait)
		}
	}
	return w.bytes, nil
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
