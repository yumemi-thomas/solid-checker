package typefacts

import "fmt"

const TypeFactsSchemaVersionV3 uint64 = 3

const (
	TypeFactsHandshakeProtocol uint64 = 1
	TypeFactsSchemaSHA256             = "sha256:6a35b7da27fa097f43cde6ea474e2d64b80823a468593e7044ffb25bb33f8e44"
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
	StructuralSpans    []LocationV2       `cbor:"structuralSpans,omitempty" json:"structuralSpans,omitempty"`
	CompilerSpans      []LocationV2       `cbor:"compilerSpans,omitempty" json:"compilerSpans,omitempty"`
	Demands            []EntityDemand     `cbor:"demands,omitempty" json:"demands,omitempty"`
	StateToken         string             `cbor:"stateToken,omitempty" json:"stateToken,omitempty"`
	ResetState         bool               `cbor:"resetState,omitempty" json:"resetState,omitempty"`
	RemovedDemandPaths []string           `cbor:"removedDemandPaths,omitempty" json:"removedDemandPaths,omitempty"`
	CancelRequestID    uint64             `cbor:"cancelRequestId,omitempty" json:"cancelRequestId,omitempty"`
}

type LifecycleError struct {
	Code    string `cbor:"code" json:"code"`
	Message string `cbor:"message" json:"message"`
}

type SourceFileV3 struct {
	Path   string `cbor:"path" json:"path"`
	Source []byte `cbor:"source" json:"source"`
}

type LifecycleTimings struct {
	RequestDecodeNs uint64 `cbor:"requestDecodeNs,omitempty" json:"requestDecodeNs,omitempty"`
	AnalyzeNs       uint64 `cbor:"analyzeNs" json:"analyzeNs"`
	AsyncNs         uint64 `cbor:"asyncNs,omitempty" json:"asyncNs,omitempty"`
	DemandNs        uint64 `cbor:"demandNs,omitempty" json:"demandNs,omitempty"`
	AssemblyNs      uint64 `cbor:"assemblyNs,omitempty" json:"assemblyNs,omitempty"`
	SortNs          uint64 `cbor:"sortNs,omitempty" json:"sortNs,omitempty"`
	CloseSymbolsNs  uint64 `cbor:"closeSymbolsNs,omitempty" json:"closeSymbolsNs,omitempty"`
	PrepareNs       uint64 `cbor:"prepareNs,omitempty" json:"prepareNs,omitempty"`
	Materialized    bool   `cbor:"materialized,omitempty" json:"materialized,omitempty"`
	RetainedFiles   uint64 `cbor:"retainedFiles,omitempty" json:"retainedFiles,omitempty"`
	RecomputedFiles uint64 `cbor:"recomputedFiles,omitempty" json:"recomputedFiles,omitempty"`
	NonDurableFiles uint64 `cbor:"nonDurableFiles,omitempty" json:"nonDurableFiles,omitempty"`
}

type LifecycleResponse struct {
	Schema     uint64            `cbor:"schema" json:"schema"`
	RequestID  uint64            `cbor:"requestId" json:"requestId"`
	ProjectID  string            `cbor:"projectId" json:"projectId"`
	Generation uint64            `cbor:"generation" json:"generation"`
	OK         bool              `cbor:"ok" json:"ok"`
	Table      *FactTableV2      `cbor:"table,omitempty" json:"table,omitempty"`
	TableDelta *FactTableDeltaV3 `cbor:"tableDelta,omitempty" json:"tableDelta,omitempty"`
	TableMode  string            `cbor:"tableMode,omitempty" json:"tableMode,omitempty"`
	StateToken string            `cbor:"stateToken,omitempty" json:"stateToken,omitempty"`
	Affected   []string          `cbor:"affected,omitempty" json:"affected,omitempty"`
	Sources    []SourceFileV3    `cbor:"sources,omitempty" json:"sources,omitempty"`
	Timings    *LifecycleTimings `cbor:"timings,omitempty" json:"timings,omitempty"`
	Error      *LifecycleError   `cbor:"error,omitempty" json:"error,omitempty"`
}

const (
	TableModeFull  = "full"
	TableModeDelta = "delta"
	TableModeReuse = "reuse"
)

// EntityFileV3 replaces all demanded entities for one source path.
type EntityFileV3 struct {
	Path     string         `cbor:"path" json:"path"`
	Entities []EntityFactV2 `cbor:"entities" json:"entities"`
}

// FactTableDeltaV3 transforms the table identified by the request's state
// token into the response generation. Collections remain canonically ordered
// after application.
type FactTableDeltaV3 struct {
	Generation         uint64           `cbor:"generation" json:"generation"`
	Sources            []SourceDigestV2 `cbor:"sources,omitempty" json:"sources,omitempty"`
	RemovedSourcePaths []string         `cbor:"removedSourcePaths,omitempty" json:"removedSourcePaths,omitempty"`
	EntityFiles        []EntityFileV3   `cbor:"entityFiles,omitempty" json:"entityFiles,omitempty"`
	RemovedEntityPaths []string         `cbor:"removedEntityPaths,omitempty" json:"removedEntityPaths,omitempty"`
	Symbols            []SymbolFactV2   `cbor:"symbols,omitempty" json:"symbols,omitempty"`
	RemovedSymbolIDs   []string         `cbor:"removedSymbolIds,omitempty" json:"removedSymbolIds,omitempty"`
	Files              []FileFactV2     `cbor:"files,omitempty" json:"files,omitempty"`
	RemovedFilePaths   []string         `cbor:"removedFilePaths,omitempty" json:"removedFilePaths,omitempty"`
}

func ValidateLifecycleRequest(request LifecycleRequest) error {
	if request.Schema != TypeFactsSchemaVersionV3 {
		return fmt.Errorf("unsupported TypeFacts schema %d", request.Schema)
	}
	if request.RequestID == 0 || request.ProjectID == "" || request.Generation == 0 {
		return ErrGenerationMismatch
	}
	switch request.Operation {
	case LifecycleOpen, LifecycleUpdate, LifecycleAnalyze, LifecycleSources, LifecycleCancel, LifecycleClose:
	default:
		return fmt.Errorf("unsupported lifecycle operation %q", request.Operation)
	}
	return nil
}
