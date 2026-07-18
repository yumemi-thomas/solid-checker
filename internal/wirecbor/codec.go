// Package wirecbor owns the deterministic CBOR mode used by migration seam
// measurements and, later, the TypeFacts protocol.
package wirecbor

import (
	"fmt"

	"github.com/fxamacker/cbor/v2"
)

const (
	MaxMessageBytes     = 64 << 20
	MaxGenerationBytes  = 64 << 20
	MaxNestedLevels     = 32
	MaxCollectionLength = 1_000_000
)

var encodingMode = mustEncodingMode()
var decodingMode = mustDecodingMode()

func mustEncodingMode() cbor.EncMode {
	mode, err := (cbor.CoreDetEncOptions()).EncMode()
	if err != nil {
		panic(err)
	}
	return mode
}

func mustDecodingMode() cbor.DecMode {
	mode, err := (cbor.DecOptions{DupMapKey: cbor.DupMapKeyEnforcedAPF, MaxNestedLevels: MaxNestedLevels, MaxArrayElements: MaxCollectionLength, MaxMapPairs: MaxCollectionLength, IndefLength: cbor.IndefLengthForbidden, TagsMd: cbor.TagsForbidden, ExtraReturnErrors: cbor.ExtraDecErrorUnknownField}).DecMode()
	if err != nil {
		panic(err)
	}
	return mode
}

func Marshal(value any) ([]byte, error) {
	encoded, err := encodingMode.Marshal(value)
	if err != nil {
		return nil, fmt.Errorf("encode deterministic CBOR: %w", err)
	}
	if len(encoded) > MaxMessageBytes {
		return nil, fmt.Errorf("encoded message is %d bytes, limit is %d", len(encoded), MaxMessageBytes)
	}
	return encoded, nil
}

func Unmarshal(encoded []byte, value any) error {
	if len(encoded) > MaxMessageBytes {
		return fmt.Errorf("encoded message is %d bytes, limit is %d", len(encoded), MaxMessageBytes)
	}
	if err := decodingMode.Unmarshal(encoded, value); err != nil {
		return fmt.Errorf("decode deterministic CBOR: %w", err)
	}
	return nil
}
