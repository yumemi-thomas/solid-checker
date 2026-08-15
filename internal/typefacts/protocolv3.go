package typefacts

import "fmt"

const TypeFactsSchemaVersionV5 uint64 = 5
const TypeFactsSchemaVersionV6 uint64 = 6
const TypeFactsSchemaVersionV7 uint64 = 7
const TypeFactsSchemaVersionV8 uint64 = 8
const TypeFactsSchemaVersionV9 uint64 = 9

const (
	TypeFactsHandshakeProtocol uint64 = 1
	TypeFactsSchemaSHA256             = "sha256:a4dfff25783d9dd99cf0d44e315a7c01e6c7d132965431ab5624a0975fd549a8"
	TypeFactsSchemaV6SHA256           = "sha256:adffdee1486dd009bb2599593e09edd4c48804678b4f23002f72e5693ffc606d"
	TypeFactsSchemaV7SHA256           = "sha256:6939a166249694edf3cf4fe1f81bd687f9b572d331988f2faaa6f2277047d352"
	TypeFactsSchemaV8SHA256           = "sha256:edbb15dc48793a12230a70305d2586da503f27f26ae5644c43b271661a30b1e1"
	// TypeFactsSchemaV9SHA256 is the digest of schema/typefacts-v9.schema.json;
	// protocolv3_test.go verifies the pairing.
	TypeFactsSchemaV9SHA256 = "sha256:c6f4ffd342381a64ba6220785f7a71f688d15c9abee43bddd6d37c8d790c2e8d"
)

type ServiceHandshake struct {
	Protocol   uint64 `cbor:"protocol" json:"protocol"`
	SchemaHash string `cbor:"schemaHash" json:"schemaHash"`
	BuildID    string `cbor:"buildId" json:"buildId"`
}

type LifecycleOperation string

const (
	LifecycleOpen    LifecycleOperation = "open"
	LifecycleUpdate  LifecycleOperation = "update"
	LifecycleAnalyze LifecycleOperation = "analyze"
	LifecycleSymbols LifecycleOperation = "symbols"
	LifecycleSources LifecycleOperation = "sources"
	LifecycleCancel  LifecycleOperation = "cancel"
	LifecycleClose   LifecycleOperation = "close"
)

type FileChangeV3 struct {
	Path    string `cbor:"path" json:"path"`
	Version uint64 `cbor:"version" json:"version"`
	Source  []byte `cbor:"source,omitempty" json:"source,omitempty"`
	Deleted bool   `cbor:"deleted,omitempty" json:"deleted,omitempty"`
}

type LifecycleRequest struct {
	Schema             uint64             `cbor:"schema" json:"schema"`
	RequestID          uint64             `cbor:"requestId" json:"requestId"`
	Operation          LifecycleOperation `cbor:"operation" json:"operation"`
	ProjectID          string             `cbor:"projectId" json:"projectId"`
	Generation         uint64             `cbor:"generation" json:"generation"`
	Changes            []FileChangeV3     `cbor:"changes,omitempty" json:"changes,omitempty"`
	Demands            []EntityDemand     `cbor:"demands,omitempty" json:"demands,omitempty"`
	CompactDemands     *CompactDemandsV3  `cbor:"compactDemands,omitempty" json:"compactDemands,omitempty"`
	StateToken         string             `cbor:"stateToken,omitempty" json:"stateToken,omitempty"`
	ResetState         bool               `cbor:"resetState,omitempty" json:"resetState,omitempty"`
	RemovedDemandPaths []string           `cbor:"removedDemandPaths,omitempty" json:"removedDemandPaths,omitempty"`
	SymbolQueries      []SymbolQueryV6    `cbor:"symbolQueries,omitempty" json:"symbolQueries,omitempty"`
	ReleaseAnalysis    bool               `cbor:"releaseAnalysis,omitempty" json:"releaseAnalysis,omitempty"`
	ReferenceChanges   bool               `cbor:"referenceChanges,omitempty" json:"referenceChanges,omitempty"`
	ReferencePaths     []string           `cbor:"referencePaths,omitempty" json:"referencePaths,omitempty"`
	CancelRequestID    uint64             `cbor:"cancelRequestId,omitempty" json:"cancelRequestId,omitempty"`
}

// SymbolQueryV6 is one row in Rust's batched TSGo oracle request. Alias and
// declarations are returned by the closure pass. The canonical reference pass
// sets ReferencesOnly so those already-owned rows are not encoded twice.
type SymbolQueryV6 struct {
	ID             SymbolID `cbor:"id" json:"id"`
	References     bool     `cbor:"references,omitempty" json:"references,omitempty"`
	ReferencesOnly bool     `cbor:"referencesOnly,omitempty" json:"referencesOnly,omitempty"`
}

type LifecycleError struct {
	Code    string `cbor:"code" json:"code"`
	Message string `cbor:"message" json:"message"`
}

type SourceFileV3 struct {
	Path   string `cbor:"path" json:"path"`
	Source []byte `cbor:"source,omitempty" json:"source,omitempty"`
	Local  bool   `cbor:"local,omitempty" json:"local,omitempty"`
}

type LifecycleTimings struct {
	RequestDecodeNs uint64 `cbor:"requestDecodeNs,omitempty" json:"requestDecodeNs,omitempty"`
	AnalyzeNs       uint64 `cbor:"analyzeNs" json:"analyzeNs"`
	AsyncNs         uint64 `cbor:"asyncNs,omitempty" json:"asyncNs,omitempty"`
	DemandNs        uint64 `cbor:"demandNs,omitempty" json:"demandNs,omitempty"`
	AssemblyNs      uint64 `cbor:"assemblyNs,omitempty" json:"assemblyNs,omitempty"`
	SortNs          uint64 `cbor:"sortNs,omitempty" json:"sortNs,omitempty"`
	CloseSymbolsNs  uint64 `cbor:"closeSymbolsNs,omitempty" json:"closeSymbolsNs,omitempty"`
	Materialized    bool   `cbor:"materialized,omitempty" json:"materialized,omitempty"`
	RetainedFiles   uint64 `cbor:"retainedFiles,omitempty" json:"retainedFiles,omitempty"`
	RecomputedFiles uint64 `cbor:"recomputedFiles,omitempty" json:"recomputedFiles,omitempty"`
	NonDurableFiles uint64 `cbor:"nonDurableFiles,omitempty" json:"nonDurableFiles,omitempty"`
}

type LifecycleResponse struct {
	Schema                  uint64            `cbor:"schema" json:"schema"`
	RequestID               uint64            `cbor:"requestId" json:"requestId"`
	ProjectID               string            `cbor:"projectId" json:"projectId"`
	Generation              uint64            `cbor:"generation" json:"generation"`
	OK                      bool              `cbor:"ok" json:"ok"`
	TableTransition         []byte            `cbor:"tableTransition,omitempty" json:"tableTransition,omitempty"`
	SymbolEvidence          []SymbolFact      `cbor:"symbolEvidence,omitempty" json:"symbolEvidence,omitempty"`
	ReferenceEvidence       []SymbolFact      `cbor:"referenceEvidence,omitempty" json:"referenceEvidence,omitempty"`
	ChangedReferenceSymbols []SymbolID        `cbor:"changedReferenceSymbols,omitempty" json:"changedReferenceSymbols,omitempty"`
	ReferenceChangesExact   bool              `cbor:"referenceChangesExact,omitempty" json:"referenceChangesExact,omitempty"`
	StateToken              string            `cbor:"stateToken,omitempty" json:"stateToken,omitempty"`
	Affected                []string          `cbor:"affected,omitempty" json:"affected,omitempty"`
	Sources                 []SourceFileV3    `cbor:"sources,omitempty" json:"sources,omitempty"`
	SourceArena             string            `cbor:"sourceArena,omitempty" json:"sourceArena,omitempty"`
	SourceLengths           []uint64          `cbor:"sourceLengths,omitempty" json:"sourceLengths,omitempty"`
	Timings                 *LifecycleTimings `cbor:"timings,omitempty" json:"timings,omitempty"`
	Error                   *LifecycleError   `cbor:"error,omitempty" json:"error,omitempty"`
}

func ValidateLifecycleRequest(request LifecycleRequest) error {
	if request.Schema != TypeFactsSchemaVersionV5 && request.Schema != TypeFactsSchemaVersionV6 && request.Schema != TypeFactsSchemaVersionV7 && request.Schema != TypeFactsSchemaVersionV8 && request.Schema != TypeFactsSchemaVersionV9 {
		return fmt.Errorf("unsupported TypeFacts schema %d", request.Schema)
	}
	if request.RequestID == 0 || request.ProjectID == "" || request.Generation == 0 {
		return ErrGenerationMismatch
	}
	switch request.Operation {
	case LifecycleOpen, LifecycleUpdate, LifecycleAnalyze, LifecycleSymbols, LifecycleSources, LifecycleCancel, LifecycleClose:
	default:
		return fmt.Errorf("unsupported lifecycle operation %q", request.Operation)
	}
	return nil
}
