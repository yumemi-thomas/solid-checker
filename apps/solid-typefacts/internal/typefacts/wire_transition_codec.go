package typefacts

import (
	"encoding/binary"
	"encoding/hex"
	"strings"
	"unicode/utf8"
)

// Optional-field and boolean flag bits shared with the Rust decoder.
const bindingFlagArray = 1 << 0

const (
	functionFlagExported = 1 << 0
	functionFlagAsync    = 1 << 1
	functionFlagArrow    = 1 << 2
)

const asyncFunctionFlagCanReturnAsync = 1 << 0

// TypeScript uses the invalid UTF-8 byte 0xfe as an unambiguous prefix for
// synthetic symbol names. Encode its public escaped spelling at the wire seam.
func wireSymbolName(name string) string {
	const internalSymbolNamePrefix = "\xfe"
	if strings.HasPrefix(name, internalSymbolNamePrefix) {
		name = "__" + strings.TrimPrefix(name, internalSymbolNamePrefix)
	}
	if !utf8.ValidString(name) {
		return strings.ToValidUTF8(name, "\uFFFD")
	}
	return name
}

// packedWriter appends varint-coded rows and interns every string into the
// transition's shared dictionary.
type packedWriter struct {
	bytes      []byte
	dict       *stringTableV3
	flush      func([]byte) error
	flushLimit int
	flushed    int
	err        error
}

func (w *packedWriter) u64(value uint64) {
	w.bytes = binary.AppendUvarint(w.bytes, value)
	w.maybeFlush()
}

func (w *packedWriter) signed(value int64) {
	w.u64(uint64(value<<1) ^ uint64(value>>63))
}

func (w *packedWriter) raw(value []byte) {
	w.bytes = append(w.bytes, value...)
	w.maybeFlush()
}

func (w *packedWriter) text(value string) {
	w.u64(w.dict.intern(value))
}

func (w *packedWriter) maybeFlush() {
	if w.flush == nil || w.err != nil || len(w.bytes) < w.flushLimit {
		return
	}
	length := len(w.bytes)
	w.err = w.flush(w.bytes)
	if w.err == nil {
		w.flushed += length
	}
	w.bytes = w.bytes[:0]
}

func (w *packedWriter) finish() error {
	if w.err == nil && w.flush != nil && len(w.bytes) != 0 {
		length := len(w.bytes)
		w.err = w.flush(w.bytes)
		if w.err == nil {
			w.flushed += length
		}
		w.bytes = w.bytes[:0]
	}
	return w.err
}

type packedLocationState struct {
	path  uint64
	start uint64
	valid bool
}

func (w *packedWriter) internalLocation(location Location, state *packedLocationState) {
	path := w.dict.intern(location.Path)
	start := uint64(location.StartByte)
	if state.valid && state.path == path {
		w.u64(1)
		w.signed(int64(start) - int64(state.start))
	} else {
		w.u64(path << 1)
		w.u64(start)
	}
	w.u64(uint64(location.EndByte - location.StartByte))
	state.path = path
	state.start = start
	state.valid = true
}

func (w *packedWriter) internalLocations(locations []Location) {
	w.u64(uint64(len(locations)))
	var state packedLocationState
	for _, location := range locations {
		w.internalLocation(location, &state)
	}
}

func (w *packedWriter) internalDeclarations(declarations []Declaration) {
	w.u64(uint64(len(declarations)))
	var state packedLocationState
	for _, declaration := range declarations {
		w.text(wireSymbolName(declaration.Name))
		w.text(declaration.Kind)
		w.internalLocation(declaration.Location, &state)
	}
}

func (w *packedWriter) internalTypeDescriptor(descriptor *TypeDescriptor) {
	w.text(descriptor.Text)
	w.text(descriptor.OriginModule)
	w.internalDeclarations(descriptor.AliasDeclarations)
}

func (w *packedWriter) internalResolvedDeclaration(declaration *ResolvedDeclaration) {
	w.text(string(declaration.Symbol))
	w.text(wireSymbolName(declaration.Name))
	w.text(declaration.Kind)
	var state packedLocationState
	w.internalLocation(declaration.Location, &state)
	w.u64(uint64(len(declaration.Owners)))
	for _, owner := range declaration.Owners {
		w.text(string(owner.Symbol))
		w.text(wireSymbolName(owner.Name))
		w.text(owner.Kind)
		w.internalLocation(owner.Location, &state)
	}
	w.text(declaration.QualifiedName)
	w.text(declaration.OriginModule)
	w.text(declaration.SourceFile)
	w.u64(boolBit(declaration.StandardLibrary))
}

func (w *packedWriter) internalSourceCall(call SourceCall) {
	var state packedLocationState
	w.internalLocation(call.Location, &state)
	w.internalLocation(call.Callee, &state)
	w.internalLocations(call.Arguments)
	w.text(string(call.Target))
}

func boolBit(value bool) uint64 {
	if value {
		return 1
	}
	return 0
}

// appendPackedDictionary emits hashed symbol IDs as raw digests and all other
// entries prefix-coded against the preceding non-hashed string.
func appendPackedDictionary(frame *packedWriter, dict *stringTableV3) {
	frame.u64(uint64(len(dict.values)))
	previous := ""
	for _, value := range dict.values {
		if digest, ok := packedHashedSymbol(value); ok {
			frame.u64(1)
			frame.raw(digest[:])
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
}

func packedHashedSymbol(symbol string) ([12]byte, bool) {
	const prefix = "symbol:h:"
	var raw [12]byte
	if !strings.HasPrefix(symbol, prefix) || len(symbol) != len(prefix)+24 {
		return raw, false
	}
	if _, err := hex.Decode(raw[:], []byte(symbol[len(prefix):])); err != nil {
		return raw, false
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
