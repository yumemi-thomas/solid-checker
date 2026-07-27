package typefacts

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"strings"
)

// Packed v3 full-frame encoding. This is deliberately an opaque byte string
// at the lifecycle seam: callers either receive a validated FactTableV2 or an
// error, and none of the columnar representation leaks into analysis code.
const packedFactTableVersion = 4

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

func (w *packedWriter) typeDescriptor(descriptor *TypeDescriptorV2) {
	w.text(descriptor.Text)
	w.text(descriptor.OriginModule)
	w.declarations(descriptor.AliasDeclarations)
}

func (w *packedWriter) resolvedDeclaration(declaration *ResolvedDeclarationV2) {
	w.text(declaration.Symbol)
	w.text(declaration.Name)
	w.text(declaration.Kind)
	var state packedLocationState
	w.location(declaration.Location, &state)
	w.u64(uint64(len(declaration.Owners)))
	for _, owner := range declaration.Owners {
		w.text(owner.Symbol)
		w.text(owner.Name)
		w.text(owner.Kind)
		w.location(owner.Location, &state)
	}
	w.text(declaration.QualifiedName)
	w.text(declaration.OriginModule)
	w.text(declaration.SourceFile)
	w.u64(boolBit(declaration.StandardLibrary))
}

func (w *packedWriter) resolvedCall(call *CallV2) {
	w.text(call.Target)
	w.text(call.ReturnTypeText)
	w.text(string(call.Validity))
	w.text(string(call.Kind))
	w.u64(boolBit(call.Declaration != nil))
	if call.Declaration != nil {
		w.resolvedDeclaration(call.Declaration)
	}
	w.u64(uint64(len(call.Arguments)))
	for _, mapping := range call.Arguments {
		w.u64(mapping.ArgumentIndex)
		w.text(string(mapping.Status))
		w.text(string(mapping.Unresolved))
		w.u64(boolBit(mapping.Parameter != nil))
		if mapping.Parameter == nil {
			continue
		}
		parameter := mapping.Parameter
		w.u64(parameter.Index)
		w.text(parameter.Symbol)
		flags := uint64(0)
		if parameter.Declaration != nil {
			flags |= 1
		}
		if parameter.Rest {
			flags |= 2
		}
		if parameter.Optional {
			flags |= 4
		}
		if parameter.TypeDescriptor != nil {
			flags |= 8
		}
		w.u64(flags)
		if parameter.Declaration != nil {
			w.text(parameter.Declaration.Name)
			w.text(parameter.Declaration.Kind)
			var state packedLocationState
			w.location(parameter.Declaration.Location, &state)
		}
		w.text(string(parameter.Callability))
		if parameter.TypeDescriptor != nil {
			w.typeDescriptor(parameter.TypeDescriptor)
		}
	}
}

func boolBit(value bool) uint64 {
	if value {
		return 1
	}
	return 0
}

func (w *packedWriter) sourceCall(call SourceCallV2) {
	var state packedLocationState
	w.location(call.Location, &state)
	w.location(call.Callee, &state)
	w.locations(call.Arguments)
	w.text(call.Target)
}

// entityRun writes one path's entity rows: a count, then delta-coded starts,
// lengths, and flagged optional fields. The run's path is written by the
// caller, whose frame decides where it goes.
func (w *packedWriter) entityRun(entities []EntityFactV2) {
	w.u64(uint64(len(entities)))
	var previousStart uint64
	for _, entity := range entities {
		w.signed(int64(entity.Location.StartByte) - int64(previousStart))
		w.u64(entity.Location.EndByte - entity.Location.StartByte)
		w.text(entity.Symbol)
		flags := uint64(0)
		if entity.TypeDescriptor != nil {
			flags |= 1
		}
		if entity.ResolvedCall != nil {
			flags |= 2
		}
		if entity.Callability != "" {
			flags |= 4
		}
		if entity.ReferenceSpace != "" {
			flags |= 8
		}
		if entity.RuntimeIdentity != "" {
			flags |= 16
		}
		w.u64(flags)
		if entity.TypeDescriptor != nil {
			w.typeDescriptor(entity.TypeDescriptor)
		}
		if entity.ResolvedCall != nil {
			w.resolvedCall(entity.ResolvedCall)
		}
		if entity.Callability != "" {
			w.text(string(entity.Callability))
		}
		if entity.ReferenceSpace != "" {
			w.text(string(entity.ReferenceSpace))
		}
		if entity.RuntimeIdentity != "" {
			w.text(entity.RuntimeIdentity)
		}
		previousStart = entity.Location.StartByte
	}
}

// fileFactBody writes one file's calls, bindings, functions, and async
// functions — everything but the path, whose position differs by frame.
func (w *packedWriter) fileFactBody(file FileFactV2) {
	w.u64(uint64(len(file.Calls)))
	for _, call := range file.Calls {
		w.sourceCall(call)
	}
	w.u64(uint64(len(file.Bindings)))
	for _, binding := range file.Bindings {
		flags := uint64(0)
		if binding.Array {
			flags |= bindingFlagArray
		}
		w.u64(flags)
		w.locations(binding.Names)
		w.sourceCall(binding.Initializer)
	}
	w.u64(uint64(len(file.Functions)))
	for _, function := range file.Functions {
		var state packedLocationState
		w.location(function.Name, &state)
		w.location(function.Body, &state)
		w.locations(function.Parameters)
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
		w.u64(flags)
	}
	w.u64(uint64(len(file.AsyncFunctions)))
	for _, function := range file.AsyncFunctions {
		var state packedLocationState
		w.location(function.Expression, &state)
		w.text(function.Symbol)
		w.text(function.Target)
		flags := uint64(0)
		if function.CanReturnAsync {
			flags |= asyncFunctionFlagCanReturnAsync
		}
		w.u64(flags)
		w.locations(function.CallsAfterAwait)
	}
}

// packedSourceDigest converts a canonical "sha256:<hex>" digest to its raw
// 32-byte frame form.
func packedSourceDigest(sha256Text string) ([]byte, error) {
	digest := strings.TrimPrefix(sha256Text, "sha256:")
	if len(digest) != 64 {
		return nil, fmt.Errorf("packed source digest is not canonical: %q", sha256Text)
	}
	raw := make([]byte, 32)
	if _, err := hex.Decode(raw, []byte(digest)); err != nil {
		return nil, fmt.Errorf("decode packed source digest: %w", err)
	}
	return raw, nil
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
		raw, err := packedSourceDigest(source.SHA256)
		if err != nil {
			return nil, err
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
		rows.entityRun(group)
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
		body.fileFactBody(file)
		rows.text(file.Path)
		rows.raw(body.bytes)
	}

	return packedFrame(table.Schema, table.Generation, rows), nil
}

// packedFrame assembles the versioned frame around finished rows: the header,
// the prefix-coded string dictionary the rows index into, then the rows.
func packedFrame(schema, generation uint64, rows packedWriter) []byte {
	frame := packedWriter{bytes: make([]byte, 0, len(rows.bytes)+4096)}
	frame.u64(packedFactTableVersion)
	frame.u64(schema)
	frame.u64(generation)
	appendPackedDictionary(&frame, rows.dict)
	frame.raw(rows.bytes)
	return frame.bytes
}

// appendPackedDictionary emits the string dictionary: hashed symbol IDs as
// raw digest bytes, everything else prefix-coded against the previous entry.
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

// Packed v3 delta-frame encoding. Deltas used to ship as plain CBOR structs —
// every row a map repeating its field-name keys, every location repeating its
// absolute path — which is exactly the overhead the compact shapes exist to
// avoid. The delta frame reuses the table frame's row and dictionary
// machinery; only the header and section order differ. Its leading version
// varint is the compatibility gate, and both executables ship in build-ID
// lockstep. The Rust decoder is decode_packed_fact_table_delta in
// crates/typefacts/src/v3.rs.
const packedFactTableDeltaVersion = 2

// PackedFactTableDeltaV3From encodes a delta into the packed frame:
// version, generation, dictionary, then sources, removed source paths,
// entity files, removed entity paths, symbols, removed symbol IDs, symbol
// reference files, files, and removed file paths, in that fixed order.
func PackedFactTableDeltaV3From(delta FactTableDeltaV3) ([]byte, error) {
	rows := packedWriter{bytes: make([]byte, 0, 4096), dict: newStringTableV3()}
	texts := func(values []string) {
		rows.u64(uint64(len(values)))
		for _, value := range values {
			rows.text(value)
		}
	}

	rows.u64(uint64(len(delta.Sources)))
	for _, source := range delta.Sources {
		rows.text(source.Path)
		raw, err := packedSourceDigest(source.SHA256)
		if err != nil {
			return nil, err
		}
		rows.raw(raw)
	}
	texts(delta.RemovedSourcePaths)

	rows.u64(uint64(len(delta.EntityFiles)))
	for _, file := range delta.EntityFiles {
		rows.text(file.Path)
		rows.entityRun(file.Entities)
	}
	texts(delta.RemovedEntityPaths)

	rows.u64(uint64(len(delta.Symbols)))
	for _, symbol := range delta.Symbols {
		rows.text(symbol.ID)
		rows.text(symbol.AliasTarget)
		rows.declarations(symbol.Declarations)
		rows.locations(symbol.References)
	}
	texts(delta.RemovedSymbolIDs)

	rows.u64(uint64(len(delta.SymbolReferenceFiles)))
	for _, file := range delta.SymbolReferenceFiles {
		rows.text(file.ID)
		rows.text(file.Path)
		rows.locations(file.References)
	}

	rows.u64(uint64(len(delta.Files)))
	for _, file := range delta.Files {
		rows.text(file.Path)
		rows.fileFactBody(file)
	}
	texts(delta.RemovedFilePaths)

	frame := packedWriter{bytes: make([]byte, 0, len(rows.bytes)+1024)}
	frame.u64(packedFactTableDeltaVersion)
	frame.u64(delta.Generation)
	appendPackedDictionary(&frame, rows.dict)
	frame.raw(rows.bytes)
	return frame.bytes, nil
}

// The internal-row writers mirror their v2 counterparts above, applying the
// same scalar conversions the v2 constructors apply (locationV2 widening,
// wireSymbolName escaping) inline — so no intermediate row is materialized.

func (w *packedWriter) internalLocation(location Location, state *packedLocationState) {
	w.location(locationV2(location), state)
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

func (w *packedWriter) internalResolvedCall(call *Call) {
	w.text(string(call.Target))
	w.text(call.ReturnTypeText)
	w.text(string(call.Validity))
	w.text(string(call.Kind))
	w.u64(boolBit(call.Declaration != nil))
	if call.Declaration != nil {
		w.internalResolvedDeclaration(call.Declaration)
	}
	w.u64(uint64(len(call.Arguments)))
	for _, mapping := range call.Arguments {
		w.u64(uint64(mapping.ArgumentIndex))
		w.text(string(mapping.Status))
		w.text(string(mapping.Unresolved))
		w.u64(boolBit(mapping.Parameter != nil))
		if mapping.Parameter == nil {
			continue
		}
		parameter := mapping.Parameter
		w.u64(uint64(parameter.Index))
		w.text(string(parameter.Symbol))
		flags := uint64(0)
		if parameter.Declaration != nil {
			flags |= 1
		}
		if parameter.Rest {
			flags |= 2
		}
		if parameter.Optional {
			flags |= 4
		}
		if parameter.TypeDescriptor != nil {
			flags |= 8
		}
		w.u64(flags)
		if parameter.Declaration != nil {
			w.text(wireSymbolName(parameter.Declaration.Name))
			w.text(parameter.Declaration.Kind)
			var state packedLocationState
			w.internalLocation(parameter.Declaration.Location, &state)
		}
		w.text(string(parameter.Callability))
		if parameter.TypeDescriptor != nil {
			w.internalTypeDescriptor(parameter.TypeDescriptor)
		}
	}
}

func (w *packedWriter) internalSourceCall(call SourceCall) {
	var state packedLocationState
	w.internalLocation(call.Location, &state)
	w.internalLocation(call.Callee, &state)
	w.internalLocations(call.Arguments)
	w.text(string(call.Target))
}

// internalEntityGroups is packedEntityGroups over internal rows.
func internalEntityGroups(entities []EntityFact) []int {
	var groups []int
	for index, entity := range entities {
		if index == 0 || entities[index-1].Location.Path != entity.Location.Path {
			groups = append(groups, 0)
		}
		groups[len(groups)-1]++
	}
	return groups
}

// PackedFactTableV3FromInternal encodes the canonical internal table into the
// same frame PackedFactTableV3From produces from the v2 form, byte for byte —
// without materializing that form. The full-mode response path uses this;
// the v2 route stays for callers that already hold a wire table.
func PackedFactTableV3FromInternal(table FactTable, generation uint64) []byte {
	rows := packedWriter{bytes: make([]byte, 0, 1<<20), dict: newStringTableV3()}

	rows.u64(uint64(len(table.Sources)))
	for _, source := range table.Sources {
		rows.text(source.Path)
		digest := sha256.Sum256(source.Source)
		rows.raw(digest[:])
	}

	groups := internalEntityGroups(table.Entities)
	rows.u64(uint64(len(groups)))
	offset := 0
	for _, length := range groups {
		group := table.Entities[offset : offset+length]
		offset += length
		rows.text(group[0].Location.Path)
		rows.u64(uint64(length))
		var previousStart uint64
		for _, entity := range group {
			start := uint64(entity.Location.StartByte)
			rows.signed(int64(start) - int64(previousStart))
			rows.u64(uint64(entity.Location.EndByte) - start)
			rows.text(string(entity.Symbol))
			flags := uint64(0)
			if entity.TypeDescriptor != nil {
				flags |= 1
			}
			if entity.ResolvedCall != nil {
				flags |= 2
			}
			if entity.Callability != "" {
				flags |= 4
			}
			if entity.ReferenceSpace != "" {
				flags |= 8
			}
			if entity.RuntimeIdentity != "" {
				flags |= 16
			}
			rows.u64(flags)
			if entity.TypeDescriptor != nil {
				rows.text(entity.TypeDescriptor.Text)
				rows.text(entity.TypeDescriptor.OriginModule)
				rows.internalDeclarations(entity.TypeDescriptor.AliasDeclarations)
			}
			if entity.ResolvedCall != nil {
				rows.internalResolvedCall(entity.ResolvedCall)
			}
			if entity.Callability != "" {
				rows.text(string(entity.Callability))
			}
			if entity.ReferenceSpace != "" {
				rows.text(string(entity.ReferenceSpace))
			}
			if entity.RuntimeIdentity != "" {
				rows.text(string(entity.RuntimeIdentity))
			}
			previousStart = start
		}
	}

	rows.u64(uint64(table.symbolFactsCount()))
	table.rangeSymbolFacts(func(symbol SymbolFact) {
		rows.text(string(symbol.ID))
		rows.text(string(symbol.AliasTarget))
		rows.internalDeclarations(symbol.Declarations)
		// The v2 constructor withholds reference lists from alias symbols;
		// the frame must carry the same empty list here.
		if symbol.AliasTarget == "" {
			rows.internalLocations(symbol.References)
		} else {
			rows.u64(0)
		}
	})

	rows.u64(uint64(len(table.Files)))
	var scratch []byte
	for _, file := range table.Files {
		// The dictionary is ordered by first use and a file interns its own
		// path after the rows it contains, so buffer the rows and emit the
		// path index in front of them. Reordering this changes the frame.
		body := packedWriter{bytes: scratch[:0], dict: rows.dict}
		body.u64(uint64(len(file.Calls)))
		for _, call := range file.Calls {
			body.internalSourceCall(call)
		}
		body.u64(uint64(len(file.Bindings)))
		for _, binding := range file.Bindings {
			flags := uint64(0)
			if binding.Array {
				flags |= bindingFlagArray
			}
			body.u64(flags)
			body.internalLocations(binding.Names)
			body.internalSourceCall(binding.Initializer)
		}
		body.u64(uint64(len(file.Functions)))
		for _, function := range file.Functions {
			var state packedLocationState
			body.internalLocation(function.Name, &state)
			body.internalLocation(function.Body, &state)
			body.internalLocations(function.Parameters)
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
			body.internalLocation(function.Expression, &state)
			body.text(string(function.Symbol))
			body.text(string(function.Target))
			flags := uint64(0)
			if function.CanReturnAsync {
				flags |= asyncFunctionFlagCanReturnAsync
			}
			body.u64(flags)
			body.internalLocations(function.CallsAfterAwait)
		}
		rows.text(file.Path)
		rows.raw(body.bytes)
		scratch = body.bytes
	}

	return packedFrame(TypeFactsTableSchemaVersion, generation, rows)
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
