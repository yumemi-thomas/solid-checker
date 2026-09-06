package typefacts

import "fmt"

const TypeFactsSchemaVersionV1 uint64 = 1

// TypeFactsHandshakeProtocol is 26: an iteration form may state the parameter
// its iterated value is rooted at, and a call may state
// CalleeIteratedParameter — the parameter-rooted iterable whose iteration
// produced the callee (ADR 0042). A rest parameter's array is also no longer
// recorded as an iteration form at all: the engine builds it, so iterating or
// spreading it reaches Array.prototype and nothing else.
// Protocol 25: an object or JSX prop spread's operand and
// an object binding pattern's source may carry SubjectParameter too, so the
// forms that read properties of a caller-supplied value all state where that
// value came from (ADR 0041). A protocol-24 producer roots only a property or
// element access.
// Protocol 24: an uncensused accessor form states its
// subject parameter in **write** position too, with SubjectWrite saying which
// position it is (ADR 0040). A protocol-23 consumer received a stated subject
// only for a read, so reading a protocol-24 transcript's subject without the
// position would treat a write into the caller's object as a read of it.
// Protocol 23: a premise reaches a local helper. A
// premised census records the argument types at each call to a runtime-source
// declaration (CallArgumentPremises), a local-declaration demand may carry
// them back as ParameterPremises, and the helper's transcript echoes the ones
// its own twin bound (ADR 0038, helper premises). A protocol-22 consumer
// refused any premise on a local declaration and never asked for one, so the
// number moves although every field is additive.
// Protocol 22: an export's root implementation transcript
// may state ParameterPremises — that its uncensused-form census was classified
// with each parameter bound to the type the export's declared call signature
// gives that position, instead of an unannotated JavaScript parameter's `any`
// (ADR 0038). An empty form list on such a transcript means "no form under the
// declared signature", which a protocol-21 consumer would read as "no form at
// all", so the number moves although the field is additive.
// Protocol 21 changes what overloadOrdinal and overloadCount range over (see
// below). Protocol 20: an implementation transcript may state
// ImplementationOf — the declaration whose body its census walked when its own
// declaration is a binding that aliases that function by identity
// (`const defaultScheduler = systemSetTimeoutZero`, followed through a sibling
// `.d.ts` to the runtime module's export of the imported name). A protocol-19
// producer refused such a binding as implementationUnavailable; a transcript
// without the field is a body of its own declaration on either protocol, so the
// field is additive, and the number moves because a consumer that never read
// the field must know that a body may now come from a named other declaration.
// Protocol 19: an implementation transcript states its
// completion form — plain, async, generator, async generator — so a `returns`
// census can refuse a callable that hands its caller a promise or an iterator
// whatever its body does (ADR 0035); an absent form on an older producer must
// not be read as plain. Protocol 18: an uncensused form and a `.call`/`.apply`
// row can state the parameter their subject is rooted at (ADR 0034), and an
// absent fact on an older producer must not be read as "not rooted".
// Protocol 17: the uncensused-form census states the
// ECMAScript guarantee that loose equality against an exact null literal does
// not invoke a coercion hook. An older producer's empty list does not carry
// that reviewed classifier meaning, so the handshake moves even though the
// wire shape does not. Protocol 16 added unchanged whole-parameter identity on
// return sites; protocol 15 stopped answering a jump-region question with
// silence.
//
// Two coordinated changes, and the field is only half of it. The
// implementation call census used to **drop** every row that lies in a region a
// `break` or `continue` makes non-universal, so as to keep an over-optimistic
// `reachable` off the wire. It now emits the row with Reach `unknown` instead —
// the weakest non-negative value, strictly weaker than the withheld row, and
// therefore unable to make any positive claim it could not already make. What
// dropping cost was a claim in the other direction: a dropped call is a
// `CallExpression`, so it leaves no uncensused-form row either, and
// `switch (kind) { case "mount": render(App, el); break; }` published nothing
// whatever about `render`.
//
// ControlFlowCensus.Incompleteness then says which of two different things each
// `unsupported` marker means. ControlFlowReachabilityLowerBound is a construct
// walked in full — a loop, a `switch`, a `try` — whose enclosed sites are all
// recorded and none of which is called `unreachable` on its account, so only
// the *guarantee* is unmodelled and a may-execute enumeration of the callables
// inside it stands. ControlFlowUnaccounted is a construct whose flow this
// census cannot account for in either direction, and it is also the
// classifier's default, so a marker nobody classified refuses on arrival rather
// than passing as the admissible arm. Unsupported keeps its exact meaning and
// its consumers.
//
// The number moves rather than the field being additive for the same reason as
// 14: an absent Incompleteness beside a nonempty Unsupported is a producer with
// no classification at all, a present empty one is the claim that nothing is
// unmodelled, and no decoder can separate them. A protocol-14 producer's
// silence would read to a protocol-15 consumer as the admissible arm, which is
// the unsound direction, so the handshake is the discriminator. In the other
// direction a protocol-14 consumer rejects a protocol-15 census outright:
// ControlFlowCensus denies unknown fields.
//
// Protocol 14 carried the two facts an implementation census needs and neither
// the call census nor Complete could carry.
//
// UncensusedInvokingForms on ExportImplementationTranscript names, per form,
// every syntactic position that can invoke user code and that Calls does not
// record — Calls holds CallExpression and NewExpression only. Its classifier's
// default is refusal: a node kind that is neither classified nor on the
// reviewed list of provably non-invoking kinds arrives as
// unclassified-invoking-form, so a form nobody has thought of refuses rather
// than passing in silence.
//
// That is the whole reason the number moves rather than the field being merely
// additive. An absent list and a present empty one are different facts — no
// opinion versus "every form I walked was a call, a construction, or provably
// non-invoking" — and no decoder can tell them apart, because the field's
// absence decodes as empty. A protocol-13 producer's silence would therefore
// read to a protocol-14 consumer as the positive claim, which is the unsound
// direction. The handshake is the discriminator, so the handshake has to move.
// In the other direction a protocol-13 consumer rejects a protocol-14
// transcript outright: its ImplementationCall and transcript types deny
// unknown fields.
//
// LocalDeclarationLocation on ExportValueDemand, answered by LocalDeclaration
// on ExportValueTranscript, asks for an implementation transcript of the
// function-like declaration at an exact source range. A census recurses into
// module-local helpers, and ImplementationLocation cannot reach one: it starts
// from an identifier and resolves through the export's runtime binding. The
// field is hashed into the export-value demand digest, so a protocol-13
// producer would answer a protocol-14 demand under a digest it computes
// differently.
//
// Protocol 13 added calleeSources to the implementation call census: the
// traced value provenance of the *callee expression*, from the same
// returnValueSourcesLocked walk and under the same
// gates that answer argumentSources for an argument. It answers "what created
// the value being called", which no other field on ImplementationCall answers
// — Target, TargetName, TargetModule, Declaration and CalleeParameter state the
// callee's *resolution* and are unchanged. An empty list is the producer's
// silence, never a negative claim about the callee.
//
// It is a break in both directions even though it is only additive:
// ImplementationCall denies unknown fields, so a protocol-12 consumer rejects a
// protocol-13 census outright, and a protocol-12 producer's silence on the field
// is indistinguishable from "traced nothing" for every call, which a
// protocol-13 consumer would read as "no callee anywhere has provenance". The
// digest and build id move with the number, and the handshake refuses on any
// mismatch.
//
// Protocol 12 added argumentSources: per written argument slot, the traced value
// provenance of the expression written there, and narrowed that tracer's
// identifier arm, which followed a symbol's first declaration and therefore
// traced a reassignable or redeclared binding to an initializer that need not
// be the value.
//
// Protocol 11 separated the members a value declares from the members it
// carries only through the compiler's apparent-type augmentation.
//
// Protocol 21 changes what overloadOrdinal and overloadCount range over: the
// declarations that state a call signature — TypeScript's overload set, the
// bodiless declarations — rather than every declaration of the signature's
// kind. A protocol-20 producer counted the implementation body of an
// overloaded function analyzed from source, so its set reported
// overloadCount == len + 1 and a consumer comparing the count with the type's
// signatures refused it as incomplete. The selected-signature identity digest
// includes the count, so the numbers move together.
const (
	TypeFactsHandshakeProtocol uint64 = 26
	TypeFactsSchemaSHA256             = "sha256:cc416d14b5f222ad4495cc526f69172bacce2ccf8cd796d6338da076fa9da490"
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
