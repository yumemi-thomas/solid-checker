package typefacts

import (
	"errors"
	"fmt"
	"path/filepath"
	"sort"
)

const (
	TypeFactsSchemaVersion uint64 = 1
	MaxDemandRounds               = 2
)

var (
	ErrGenerationMismatch = errors.New("type facts generation mismatch")
	ErrRoundLimit         = errors.New("type facts demand round limit exceeded")
	ErrRepeatedRequestKey = errors.New("type facts request key repeated in generation")
	ErrOutsideUniverse    = errors.New("type facts request key is outside the finite universe")
)

// GenerationDescriptor is the identity handshake for one immutable analysis
// generation. Universe metrics are measured before serving demand subsets.
type GenerationDescriptor struct {
	Schema             uint64 `cbor:"schema" json:"schema"`
	ProjectID          string `cbor:"projectId" json:"projectId"`
	Generation         uint64 `cbor:"generation" json:"generation"`
	UniverseEntities   uint64 `cbor:"universeEntities" json:"universeEntities"`
	UniverseSymbols    uint64 `cbor:"universeSymbols" json:"universeSymbols"`
	UniverseBytes      uint64 `cbor:"universeBytes" json:"universeBytes"`
	UniverseBuildNanos uint64 `cbor:"universeBuildNanos" json:"universeBuildNanos"`
}

// BatchRequest is one deterministic demand round. FullUniverse is reserved for
// generation enumeration and G1 measurement; live analysis sends Demand.
type BatchRequest struct {
	Schema       uint64        `cbor:"schema" json:"schema"`
	ProjectID    string        `cbor:"projectId" json:"projectId"`
	Generation   uint64        `cbor:"generation" json:"generation"`
	Round        uint64        `cbor:"round" json:"round"`
	FullUniverse bool          `cbor:"fullUniverse,omitempty" json:"fullUniverse,omitempty"`
	Demand       DemandProfile `cbor:"demand" json:"demand"`
}

type BatchResponse struct {
	Schema     uint64    `cbor:"schema" json:"schema"`
	ProjectID  string    `cbor:"projectId" json:"projectId"`
	Generation uint64    `cbor:"generation" json:"generation"`
	Round      uint64    `cbor:"round" json:"round"`
	Table      FactTable `cbor:"table" json:"table"`
}

// ValidateBatchRequest enforces identity, bounded rounds, finite keys, and
// within-generation monotonicity. universe contains canonical keys returned by
// DemandKeys for the generation's complete catalog.
func ValidateBatchRequest(descriptor GenerationDescriptor, request BatchRequest, universe, previouslyRequested map[string]struct{}) error {
	if request.Schema != TypeFactsSchemaVersion || descriptor.Schema != TypeFactsSchemaVersion {
		return fmt.Errorf("unsupported TypeFacts schema")
	}
	if request.ProjectID != descriptor.ProjectID || request.Generation != descriptor.Generation {
		return ErrGenerationMismatch
	}
	if request.Round == 0 || request.Round > MaxDemandRounds {
		return ErrRoundLimit
	}
	keys := DemandKeys(request.Demand)
	for _, key := range keys {
		if _, ok := universe[key]; !ok {
			return fmt.Errorf("%w: %s", ErrOutsideUniverse, key)
		}
		if _, duplicate := previouslyRequested[key]; duplicate {
			return fmt.Errorf("%w: %s", ErrRepeatedRequestKey, key)
		}
	}
	return nil
}

// DemandKeys returns the language-neutral canonical key ordering.
func DemandKeys(demand DemandProfile) []string {
	keys := make([]string, 0, len(demand.Entities)+len(demand.Files)+len(demand.RefreshPaths)+len(demand.RefreshRanges))
	for _, entity := range demand.Entities {
		location := entity.Location
		prefix := fmt.Sprintf("entity:%s:%d:%d:", filepath.Clean(location.Path), location.StartByte, location.EndByte)
		for _, field := range []struct {
			requested bool
			key       string
		}{
			{entity.Symbol, "symbol"},
			{entity.Type, "type"},
			{entity.TypeDescriptor, "descriptor"},
			{entity.ResolvedCall, "call"},
			{entity.ResolveAlias, "alias"},
			{entity.Declarations, "declarations"},
			{entity.References, "references"},
		} {
			if field.requested {
				keys = append(keys, prefix+field.key)
			}
		}
	}
	for _, path := range demand.Files {
		keys = append(keys, "file:"+filepath.Clean(path))
	}
	for _, path := range demand.RefreshPaths {
		keys = append(keys, "refresh-file:"+filepath.Clean(path))
	}
	for _, location := range demand.RefreshRanges {
		keys = append(keys, fmt.Sprintf("refresh-range:%s:%d:%d", filepath.Clean(location.Path), location.StartByte, location.EndByte))
	}
	sort.Strings(keys)
	return keys
}
