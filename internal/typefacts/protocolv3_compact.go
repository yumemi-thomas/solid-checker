package typefacts

import (
	"encoding/binary"
	"fmt"
	"strings"
	"unicode/utf8"
)

// Compact v3 full-frame encoding.
//
// Cold analyze exchanges dominate boundary bytes because the plain wire
// shapes repeat CBOR field-name keys on every record and the absolute source
// path on every location. The compact forms carry one string dictionary per
// frame and encode rows as fixed-arity arrays; they decode into exactly the
// plain shapes, so everything past the transport seam is unchanged. Both
// executables ship in build-ID lockstep (the handshake rejects a mismatch),
// so the compact forms need no runtime negotiation.
//
// Dictionary index 0 is reserved for the empty string, which is also how
// optional string fields encode their absence. Optional nested rows encode
// as zero-or-one-element arrays and collections always encode as arrays —
// never null, which the deterministic CBOR contract forbids.

// CompactLocationV3 is [path-index, startByte, endByte].
type CompactLocationV3 struct {
	_         struct{} `cbor:",toarray"`
	Path      uint64
	StartByte uint64
	EndByte   uint64
}

// CompactDemandV3 is [flags, startByte, endByte, query-location-or-empty].
type CompactDemandV3 struct {
	_         struct{} `cbor:",toarray"`
	Flags     uint64
	StartByte uint64
	EndByte   uint64
	Query     []CompactLocationV3
}

// CompactDemandGroupV3 is [path-index, packed demand rows for that path].
// Rows use unsigned LEB128 `(flags << 1 | hasQuery, startDelta, length)`,
// followed by `(queryPath, queryStart, queryLength)` when present.
type CompactDemandGroupV3 struct {
	_       struct{} `cbor:",toarray"`
	Path    uint64
	Demands []byte
}

// CompactDemandsV3 is the compact form of a full demand snapshot.
type CompactDemandsV3 struct {
	Groups  []CompactDemandGroupV3 `cbor:"groups" json:"groups"`
	Strings []string               `cbor:"strings" json:"strings"`
}

// Demand flag bits shared with the Rust encoder.
const (
	demandFlagSymbol             = 1 << 0
	demandFlagReferences         = 1 << 1
	demandFlagTypeDescriptor     = 1 << 2
	demandFlagResolvedCall       = 1 << 3
	demandFlagAsync              = 1 << 4
	demandFlagStructuralAccessor = 1 << 5
	demandFlagCallability        = 1 << 6
	demandFlagReferenceSpace     = 1 << 7
	demandFlagRuntimeIdentity    = 1 << 8
	demandFlagRuntimeValueDomain = 1 << 9
	demandFlagCallResultDomain   = 1 << 10
	demandFlagConstantValue      = 1 << 11
	demandFlagArrayShape         = 1 << 12
	demandFlagTupleShape         = 1 << 13
	demandFlagLibraryTypes       = 1 << 14
)

// stringTableV3 interns strings in first-occurrence order; index 0 is "".
type stringTableV3 struct {
	indexes map[string]uint64
	values  []string
}

func newStringTableV3() *stringTableV3 {
	return &stringTableV3{indexes: map[string]uint64{"": 0}, values: []string{""}}
}

func (t *stringTableV3) intern(value string) uint64 {
	if !utf8.ValidString(value) {
		value = strings.ToValidUTF8(value, "\uFFFD")
	}
	if index, ok := t.indexes[value]; ok {
		return index
	}
	index := uint64(len(t.values))
	t.indexes[value] = index
	t.values = append(t.values, value)
	return index
}

type stringUntableV3 []string

func (t stringUntableV3) lookup(index uint64) (string, error) {
	if index >= uint64(len(t)) {
		return "", fmt.Errorf("compact string index %d out of range (%d strings)", index, len(t))
	}
	return t[index], nil
}

// CompactDemandsV3From converts a full demand snapshot into its compact
// form. Demands are grouped by location path in input order.
func CompactDemandsV3From(demands []EntityDemand) CompactDemandsV3 {
	strings := newStringTableV3()
	compact := CompactDemandsV3{Groups: make([]CompactDemandGroupV3, 0, 16)}
	previousStart := 0
	for _, demand := range demands {
		path := strings.intern(demand.Location.Path)
		if len(compact.Groups) == 0 ||
			compact.Groups[len(compact.Groups)-1].Path != path ||
			demand.Location.StartByte < previousStart {
			compact.Groups = append(compact.Groups, CompactDemandGroupV3{
				Path:    path,
				Demands: make([]byte, 0, 64),
			})
			previousStart = 0
		}
		flags := uint64(0)
		if demand.Symbol {
			flags |= demandFlagSymbol
		}
		if demand.References {
			flags |= demandFlagReferences
		}
		if demand.TypeDescriptor {
			flags |= demandFlagTypeDescriptor
		}
		if demand.ResolvedCall {
			flags |= demandFlagResolvedCall
		}
		if demand.Async {
			flags |= demandFlagAsync
		}
		if demand.StructuralAccessor {
			flags |= demandFlagStructuralAccessor
		}
		if demand.Callability {
			flags |= demandFlagCallability
		}
		if demand.ReferenceSpace {
			flags |= demandFlagReferenceSpace
		}
		if demand.RuntimeIdentity {
			flags |= demandFlagRuntimeIdentity
		}
		if demand.RuntimeValueDomain {
			flags |= demandFlagRuntimeValueDomain
		}
		if demand.CallResultDomain {
			flags |= demandFlagCallResultDomain
		}
		if demand.ConstantValue {
			flags |= demandFlagConstantValue
		}
		if demand.ArrayShape {
			flags |= demandFlagArrayShape
		}
		if demand.TupleShape {
			flags |= demandFlagTupleShape
		}
		if demand.LibraryTypes {
			flags |= demandFlagLibraryTypes
		}
		header := flags << 1
		if demand.QueryLocation != nil {
			header |= 1
		}
		group := &compact.Groups[len(compact.Groups)-1]
		group.Demands = binary.AppendUvarint(group.Demands, header)
		group.Demands = binary.AppendUvarint(group.Demands, uint64(demand.Location.StartByte-previousStart))
		group.Demands = binary.AppendUvarint(group.Demands, uint64(demand.Location.EndByte-demand.Location.StartByte))
		previousStart = demand.Location.StartByte
		if demand.QueryLocation != nil {
			group.Demands = binary.AppendUvarint(group.Demands, strings.intern(demand.QueryLocation.Path))
			group.Demands = binary.AppendUvarint(group.Demands, uint64(demand.QueryLocation.StartByte))
			group.Demands = binary.AppendUvarint(
				group.Demands,
				uint64(demand.QueryLocation.EndByte-demand.QueryLocation.StartByte),
			)
		}
	}
	compact.Strings = strings.values
	return compact
}

// Expand converts the compact demand snapshot back into plain demands.
func (compact CompactDemandsV3) Expand() ([]EntityDemand, error) {
	strings := stringUntableV3(compact.Strings)
	demandCount, queryCount, err := compact.demandShape()
	if err != nil {
		return nil, err
	}
	// The expanded slice becomes retained session state. Allocate it once at
	// its final size instead of geometrically growing a large array whose
	// unused tail would remain live for the session lifetime.
	demands := make([]EntityDemand, 0, demandCount)
	// Query locations used to escape one allocation at a time. They have the
	// same lifetime as the retained demand slice, so one exact arena removes
	// tens of thousands of tiny heap objects without changing pointer-based
	// model or wire semantics.
	queries := make([]Location, queryCount)
	queryIndex := 0
	for _, group := range compact.Groups {
		path, err := strings.lookup(group.Path)
		if err != nil {
			return nil, err
		}
		packed := group.Demands
		previousStart := uint64(0)
		for len(packed) != 0 {
			header, rest, err := takeCompactUvarint(packed)
			if err != nil {
				return nil, err
			}
			startDelta, rest, err := takeCompactUvarint(rest)
			if err != nil {
				return nil, err
			}
			length, rest, err := takeCompactUvarint(rest)
			if err != nil {
				return nil, err
			}
			start := previousStart + startDelta
			if start < previousStart || start+length < start {
				return nil, fmt.Errorf("compact demand location overflow")
			}
			previousStart = start
			flags := header >> 1
			demand := EntityDemand{
				Location:           Location{Path: path, StartByte: int(start), EndByte: int(start + length)},
				Symbol:             flags&demandFlagSymbol != 0,
				References:         flags&demandFlagReferences != 0,
				TypeDescriptor:     flags&demandFlagTypeDescriptor != 0,
				ResolvedCall:       flags&demandFlagResolvedCall != 0,
				Async:              flags&demandFlagAsync != 0,
				StructuralAccessor: flags&demandFlagStructuralAccessor != 0,
				Callability:        flags&demandFlagCallability != 0,
				ReferenceSpace:     flags&demandFlagReferenceSpace != 0,
				RuntimeIdentity:    flags&demandFlagRuntimeIdentity != 0,
				RuntimeValueDomain: flags&demandFlagRuntimeValueDomain != 0,
				CallResultDomain:   flags&demandFlagCallResultDomain != 0,
				ConstantValue:      flags&demandFlagConstantValue != 0,
				ArrayShape:         flags&demandFlagArrayShape != 0,
				TupleShape:         flags&demandFlagTupleShape != 0,
				LibraryTypes:       flags&demandFlagLibraryTypes != 0,
			}
			if header&1 != 0 {
				queryPathIndex, next, err := takeCompactUvarint(rest)
				if err != nil {
					return nil, err
				}
				queryStart, next, err := takeCompactUvarint(next)
				if err != nil {
					return nil, err
				}
				queryLength, next, err := takeCompactUvarint(next)
				if err != nil {
					return nil, err
				}
				if queryStart+queryLength < queryStart {
					return nil, fmt.Errorf("compact demand query location overflow")
				}
				queryPath, err := strings.lookup(queryPathIndex)
				if err != nil {
					return nil, err
				}
				queries[queryIndex] = Location{
					Path:      queryPath,
					StartByte: int(queryStart),
					EndByte:   int(queryStart + queryLength),
				}
				demand.QueryLocation = &queries[queryIndex]
				queryIndex++
				rest = next
			}
			packed = rest
			demands = append(demands, demand)
		}
	}
	return demands, nil
}

func (compact CompactDemandsV3) demandShape() (demandCount, queryCount int, err error) {
	for _, group := range compact.Groups {
		packed := group.Demands
		for len(packed) != 0 {
			header, rest, err := takeCompactUvarint(packed)
			if err != nil {
				return 0, 0, err
			}
			for range 2 {
				_, rest, err = takeCompactUvarint(rest)
				if err != nil {
					return 0, 0, err
				}
			}
			if header&1 != 0 {
				queryCount++
				for range 3 {
					_, rest, err = takeCompactUvarint(rest)
					if err != nil {
						return 0, 0, err
					}
				}
			}
			packed = rest
			demandCount++
		}
	}
	return demandCount, queryCount, nil
}

func takeCompactUvarint(input []byte) (uint64, []byte, error) {
	value, count := binary.Uvarint(input)
	switch {
	case count == 0:
		return 0, nil, fmt.Errorf("truncated compact demand varint")
	case count < 0:
		return 0, nil, fmt.Errorf("overflowing compact demand varint")
	default:
		return value, input[count:], nil
	}
}

// appendCompactDemandsWithFlag visits a retained packed run without expanding
// unrelated rows. It is the semantic-oracle seam used for batch-wide flags
// such as Async; full EntityDemand rows are materialized only when selected.
func appendCompactDemandsWithFlag(
	target []EntityDemand,
	group CompactDemandGroupV3,
	stringTable []string,
	requiredFlag uint64,
) ([]EntityDemand, error) {
	strings := stringUntableV3(stringTable)
	path, err := strings.lookup(group.Path)
	if err != nil {
		return nil, err
	}
	packed := group.Demands
	previousStart := uint64(0)
	for len(packed) != 0 {
		header, rest, err := takeCompactUvarint(packed)
		if err != nil {
			return nil, err
		}
		startDelta, rest, err := takeCompactUvarint(rest)
		if err != nil {
			return nil, err
		}
		length, rest, err := takeCompactUvarint(rest)
		if err != nil {
			return nil, err
		}
		start := previousStart + startDelta
		if start < previousStart || start+length < start {
			return nil, fmt.Errorf("compact demand location overflow")
		}
		previousStart = start
		flags := header >> 1
		selected := flags&requiredFlag != 0
		demand := EntityDemand{}
		if selected {
			demand = EntityDemand{
				Location:           Location{Path: path, StartByte: int(start), EndByte: int(start + length)},
				Symbol:             flags&demandFlagSymbol != 0,
				References:         flags&demandFlagReferences != 0,
				TypeDescriptor:     flags&demandFlagTypeDescriptor != 0,
				ResolvedCall:       flags&demandFlagResolvedCall != 0,
				Async:              flags&demandFlagAsync != 0,
				StructuralAccessor: flags&demandFlagStructuralAccessor != 0,
				Callability:        flags&demandFlagCallability != 0,
				ReferenceSpace:     flags&demandFlagReferenceSpace != 0,
				RuntimeIdentity:    flags&demandFlagRuntimeIdentity != 0,
				RuntimeValueDomain: flags&demandFlagRuntimeValueDomain != 0,
				CallResultDomain:   flags&demandFlagCallResultDomain != 0,
				ConstantValue:      flags&demandFlagConstantValue != 0,
				ArrayShape:         flags&demandFlagArrayShape != 0,
				TupleShape:         flags&demandFlagTupleShape != 0,
				LibraryTypes:       flags&demandFlagLibraryTypes != 0,
			}
		}
		if header&1 != 0 {
			queryPathIndex, next, err := takeCompactUvarint(rest)
			if err != nil {
				return nil, err
			}
			queryStart, next, err := takeCompactUvarint(next)
			if err != nil {
				return nil, err
			}
			queryLength, next, err := takeCompactUvarint(next)
			if err != nil {
				return nil, err
			}
			if queryStart+queryLength < queryStart {
				return nil, fmt.Errorf("compact demand query location overflow")
			}
			if selected {
				queryPath, err := strings.lookup(queryPathIndex)
				if err != nil {
					return nil, err
				}
				demand.QueryLocation = &Location{
					Path:      queryPath,
					StartByte: int(queryStart),
					EndByte:   int(queryStart + queryLength),
				}
			}
			rest = next
		}
		if selected {
			target = append(target, demand)
		}
		packed = rest
	}
	return target, nil
}
