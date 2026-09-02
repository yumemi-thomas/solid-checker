package typefacts

import "fmt"

const TypeFactsSchemaVersionV1 uint64 = 1

// TypeFactsHandshakeProtocol is 10 because an exported implementation now
// carries exact per-callable return-carry edges. They let a consumer compose a
// callable returned by a callable the export returns, while refusing lexical
// nesting, stored closures, and return sites that are not reachable.
//
// A protocol-9 client rejects unknown transcript fields outright, and a
// protocol-9 producer omits a field this protocol requires, so the addition
// is a break rather than a compatible extension and the number is what says
// so. The digest and build id still move with it, and the handshake refuses on
// any mismatch.
const (
	TypeFactsHandshakeProtocol uint64 = 10
	TypeFactsSchemaSHA256             = "sha256:aeb7900e0c359221ef14f0bd705358d516249d50a67db5063a33c00dcbac3c84"
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
	// LifecycleModules answers for the resolved module graph of the open
	// generation. Like sources it is a read of the retained program: it holds
	// no state token, edits no retained demand set, and advances no
	// generation.
	LifecycleModules LifecycleOperation = "modules"
	// LifecycleInvocations returns demand-shaped selected-signature, binding,
	// callable-path and census proof facts without retaining them in the editor
	// analysis table.
	LifecycleInvocations  LifecycleOperation = "invocations"
	LifecycleExportValues LifecycleOperation = "export-values"
	LifecycleCancel       LifecycleOperation = "cancel"
	LifecycleClose        LifecycleOperation = "close"
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
	// ModuleGraph selects how much of the resolved module graph a modules
	// operation answers. It is read only by that operation; an absent demand
	// there answers the module inventory alone.
	ModuleGraph        *ModuleInventoryDemand `cbor:"moduleGraph,omitempty" json:"moduleGraph,omitempty"`
	InvocationDemands  []InvocationDemand     `cbor:"invocationDemands,omitempty" json:"invocationDemands,omitempty"`
	ExportValueDemands []ExportValueDemand    `cbor:"exportValueDemands,omitempty" json:"exportValueDemands,omitempty"`
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
	// Modules, ModuleImports, and UnknownImportPaths carry a modules
	// operation's answer. They are the flattened ModuleInventory: the protocol
	// keeps response payloads flat, as sources and symbolEvidence already are.
	Modules                []ModuleFact            `cbor:"modules,omitempty" json:"modules,omitempty"`
	ModuleImports          []ModuleImportFact      `cbor:"moduleImports,omitempty" json:"moduleImports,omitempty"`
	UnknownImportPaths     []string                `cbor:"unknownImportPaths,omitempty" json:"unknownImportPaths,omitempty"`
	InvocationTranscripts  []InvocationTranscript  `cbor:"invocationTranscripts,omitempty" json:"invocationTranscripts,omitempty"`
	InvocationEnvelope     *InvocationEnvelope     `cbor:"invocationEnvelope,omitempty" json:"invocationEnvelope,omitempty"`
	ExportValueTranscripts []ExportValueTranscript `cbor:"exportValueTranscripts,omitempty" json:"exportValueTranscripts,omitempty"`
	ExportValueEnvelope    *InvocationEnvelope     `cbor:"exportValueEnvelope,omitempty" json:"exportValueEnvelope,omitempty"`
}

func ValidateLifecycleRequest(request LifecycleRequest) error {
	if request.Schema != TypeFactsSchemaVersionV1 {
		return fmt.Errorf("unsupported TypeFacts schema %d", request.Schema)
	}
	if request.RequestID == 0 || request.ProjectID == "" || request.Generation == 0 {
		return ErrGenerationMismatch
	}
	switch request.Operation {
	case LifecycleOpen, LifecycleUpdate, LifecycleAnalyze, LifecycleSymbols,
		LifecycleSources, LifecycleModules, LifecycleInvocations, LifecycleExportValues, LifecycleCancel, LifecycleClose:
	default:
		return fmt.Errorf("unsupported lifecycle operation %q", request.Operation)
	}
	return nil
}
