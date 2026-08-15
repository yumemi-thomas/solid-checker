package typefacts

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime/debug"
	"sort"
	"strconv"
	"time"
)

var ErrSessionClosed = errors.New("Type Facts session is closed")

// Session owns one retained Type Facts analysis lifetime. Its interface
// concentrates project identity, generation, retained demand state, wire
// table selection, and project closure behind the v3 lifecycle request shape.
//
// Calls are dispatched serially by the protocol adapter. Cancellation may
// arrive concurrently by cancelling the context of the active call.
type Session struct {
	closure         *DemandClosure
	trace           Trace
	projectID       string
	retained        retainedSessionState
	transition      wireTransitionEncoder
	sourceArenaPath string
	closed          bool
	closeErr        error
	schema          uint64
}

type retainedSessionState struct {
	token     uint64
	tokenText string
	demands   retainedDemandStore
	table     *FactTable
}

// NewSession assumes ownership of backend, including when construction fails.
// trace may be nil, which disables producer-side tracing.
func NewSession(backend Project, projectID string, trace Trace) (*Session, error) {
	return newSession(backend, projectID, trace, TypeFactsSchemaVersionV5)
}

// NewSessionV6 transfers expanded path-row ownership to the Rust client after
// every successful transition. V5 remains available as the compatibility
// adapter for existing consumers.
func NewSessionV6(backend Project, projectID string, trace Trace) (*Session, error) {
	return newSession(backend, projectID, trace, TypeFactsSchemaVersionV6)
}

// NewSessionV7 enables RuntimeValueDomain and emits Wire table schema v4.
// Frozen v5 and v6 sessions remain available for compatibility.
func NewSessionV7(backend Project, projectID string, trace Trace) (*Session, error) {
	return newSession(backend, projectID, trace, TypeFactsSchemaVersionV7)
}

// NewSessionV8 carries explicit negative symbol-resolution facts and emits
// Wire table schema v5. Frozen v5-v7 sessions remain available for compatibility.
func NewSessionV8(backend Project, projectID string, trace Trace) (*Session, error) {
	return newSession(backend, projectID, trace, TypeFactsSchemaVersionV8)
}

// NewSessionV9 carries exhaustive resolved-call target candidate sets and
// emits Wire table schema v6. Frozen v5-v8 sessions remain available for
// compatibility.
func NewSessionV9(backend Project, projectID string, trace Trace) (*Session, error) {
	return newSession(backend, projectID, trace, TypeFactsSchemaVersionV9)
}

func newSession(backend Project, projectID string, trace Trace, schema uint64) (*Session, error) {
	projectID = filepath.Clean(projectID)
	if projectID == "" || projectID == "." {
		_ = backend.Close()
		return nil, errors.New("Type Facts session requires a project identity")
	}
	closure, err := newDemandClosure(backend, trace, schema >= TypeFactsSchemaVersionV6)
	if err != nil {
		_ = backend.Close()
		return nil, err
	}
	tableSchema := TypeFactsTableSchemaVersion
	if schema < TypeFactsSchemaVersionV7 {
		tableSchema = TypeFactsTableSchemaVersionV3
	} else if schema < TypeFactsSchemaVersionV8 {
		tableSchema = TypeFactsTableSchemaVersionV4
	} else if schema < TypeFactsSchemaVersionV9 {
		tableSchema = TypeFactsTableSchemaVersionV5
	}
	return &Session{
		closure:    closure,
		trace:      trace,
		projectID:  projectID,
		schema:     schema,
		transition: wireTransitionEncoder{tableSchema: tableSchema},
	}, nil
}

func (s *Session) Lifecycle(ctx context.Context, request LifecycleRequest) LifecycleResponse {
	return s.lifecycle(ctx, request, nil)
}

// LifecycleInto writes packed table transitions into arena while retaining
// lifecycle metadata in the ordinary response.
func (s *Session) LifecycleInto(
	ctx context.Context,
	request LifecycleRequest,
	arena TransitionArena,
) LifecycleResponse {
	return s.lifecycle(ctx, request, arena)
}

func (s *Session) lifecycle(
	ctx context.Context,
	request LifecycleRequest,
	arena TransitionArena,
) LifecycleResponse {
	generation := s.closure.generation
	response := LifecycleResponse{
		Schema: s.schema, RequestID: request.RequestID,
		ProjectID: s.projectID, Generation: generation,
	}
	fail := func(code string, err error) LifecycleResponse {
		response.Error = &LifecycleError{Code: code, Message: err.Error()}
		return response
	}
	if err := ValidateLifecycleRequest(request); err != nil {
		return fail("invalid-request", err)
	}
	if request.Schema != s.schema {
		return fail("invalid-request", fmt.Errorf("unsupported TypeFacts schema %d", request.Schema))
	}
	if s.schema < TypeFactsSchemaVersionV7 {
		for _, demand := range request.Demands {
			if demand.RuntimeValueDomain {
				return fail("invalid-demands", errors.New("runtime value domain requires TypeFacts schema v7"))
			}
		}
		if request.CompactDemands != nil {
			for _, group := range request.CompactDemands.Groups {
				selected, err := appendCompactDemandsWithFlag(nil, group, request.CompactDemands.Strings, demandFlagRuntimeValueDomain)
				if err != nil {
					return fail("invalid-demands", err)
				}
				if len(selected) != 0 {
					return fail("invalid-demands", errors.New("runtime value domain requires TypeFacts schema v7"))
				}
			}
		}
	}
	if filepath.Clean(request.ProjectID) != s.projectID {
		return fail("project-mismatch", ErrGenerationMismatch)
	}
	if s.closed {
		if request.Operation == LifecycleClose && s.closeErr == nil {
			response.OK = true
			return response
		}
		return fail("session-closed", ErrSessionClosed)
	}

	switch request.Operation {
	case LifecycleOpen:
		if request.Generation != generation {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
	case LifecycleUpdate:
		if request.Generation != generation+1 {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
		changes := make([]FileChange, 0, len(request.Changes))
		for _, change := range request.Changes {
			changes = append(changes, FileChange{
				Path: change.Path, Version: change.Version, Source: change.Source, Deleted: change.Deleted,
			})
		}
		affected, err := s.closure.Update(ctx, changes)
		if err != nil {
			return fail("update-failed", err)
		}
		response.Generation = s.closure.generation
		response.Affected = affected.Files
	case LifecycleAnalyze:
		if request.Generation != generation {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
		if request.CompactDemands != nil && len(request.Demands) != 0 {
			return fail("invalid-demands", fmt.Errorf("analyze carries both demands and compactDemands"))
		}
		// Analyze is always retained-state scoped: a caller either resets the
		// state or presents the token the previous analyze handed back.
		if !request.ResetState && request.StateToken != s.retained.tokenText {
			return fail("state-mismatch", ErrGenerationMismatch)
		}
		if !request.ResetState &&
			len(request.Demands) == 0 &&
			(request.CompactDemands == nil || len(request.CompactDemands.Groups) == 0) &&
			len(request.RemovedDemandPaths) == 0 &&
			s.retained.table != nil &&
			s.retained.table.Generation == generation {
			response.StateToken = s.retained.tokenText
			response.Timings = &LifecycleTimings{}
			response.OK = true
			return response
		}
		var demandTransaction retainedDemandTransaction
		if request.CompactDemands != nil {
			var err error
			demandTransaction, err = s.retained.demands.beginCompact(
				*request.CompactDemands,
				request.RemovedDemandPaths,
				request.ResetState,
			)
			if err != nil {
				return fail("invalid-demands", err)
			}
		} else {
			demandTransaction = s.retained.demands.begin(
				request.Demands,
				request.RemovedDemandPaths,
				request.ResetState,
			)
		}
		demandsPublished := false
		defer func() {
			if !demandsPublished {
				demandTransaction.rollback()
			}
		}()
		started := time.Now()
		buildSequence := s.closure.Stats().BuildSequence
		analysisPublished := false
		defer func() {
			if !analysisPublished {
				s.closure.abandonAnalysis()
			}
		}()
		analyzedTable, err := s.closure.demandTableForCanonicalGroups(
			ctx,
			generation,
			demandTransaction.groups(),
			demandTransaction.paths(),
		)
		if err != nil {
			if ctx.Err() != nil {
				return fail("analysis-cancelled", ctx.Err())
			}
			return fail("analysis-failed", err)
		}
		if err := ctx.Err(); err != nil {
			return fail("analysis-cancelled", err)
		}
		stats := s.closure.Stats()
		elapsed := time.Since(started)
		materialized := stats.BuildSequence != buildSequence
		response.Timings = &LifecycleTimings{
			AnalyzeNs:    uint64(elapsed),
			Materialized: materialized,
		}
		if materialized {
			response.Timings.AsyncNs = uint64(stats.AsyncDuration)
			response.Timings.DemandNs = uint64(stats.DemandDuration)
			response.Timings.AssemblyNs = uint64(stats.AssemblyDuration)
			response.Timings.SortNs = uint64(stats.SortDuration)
			response.Timings.CloseSymbolsNs = uint64(stats.CloseDuration)
			response.Timings.RetainedFiles = uint64(stats.Retention.RetainedFiles)
			response.Timings.RecomputedFiles = uint64(stats.Retention.RecomputedFiles)
			response.Timings.NonDurableFiles = uint64(stats.Retention.NonDurableFiles)
		}
		nextToken := s.retained.token + 1
		nextTokenText := strconv.FormatUint(nextToken, 10)
		response.StateToken = nextTokenText
		// Building the wire form is not part of the analysis the response
		// reports, so it is traced separately. Without this the cost shows up
		// nowhere and reads as client or transport overhead.
		transportStarted := time.Now()
		transitionInput := wireTransitionInput{
			ProjectID: s.projectID,
			Target:    analyzedTable,
			Sparse:    s.schema >= TypeFactsSchemaVersionV6,
		}
		if !request.ResetState && s.retained.table != nil {
			transitionInput.Base = s.retained.table
			transitionInput.BaseStateToken = s.retained.tokenText
		}
		var transition encodedWireTransition
		if arena == nil {
			transition, err = s.transition.Encode(transitionInput)
		} else {
			transition, err = s.transition.EncodeInto(transitionInput, arena)
		}
		if errors.Is(err, errSparseTransitionUnavailable) {
			// Exact reference evidence can be unavailable after broad compiler
			// invalidation. Rebuild once and send a full replacement; the normal
			// sparse edit path remains proportional to changed files.
			s.closure.forceFullMaterialization()
			analyzedTable, err = s.closure.demandTableForCanonicalGroups(
				ctx,
				generation,
				demandTransaction.groups(),
				demandTransaction.paths(),
			)
			if err == nil {
				transitionInput.Base = nil
				transitionInput.BaseStateToken = ""
				transitionInput.Target = analyzedTable
				transitionInput.Sparse = false
				if arena == nil {
					transition, err = s.transition.Encode(transitionInput)
				} else {
					transition, err = s.transition.EncodeInto(transitionInput, arena)
				}
			}
		}
		if err != nil {
			return fail("assembly-failed", err)
		}
		transitionMode := transition.Mode.String()
		// Within one source generation an empty delta is genuine reuse and
		// needs no frame. Across generations the empty delta header still
		// advances the retained table identity.
		if transition.Mode != wireTransitionDelta ||
			analyzedTable.Generation != s.retained.table.Generation ||
			transition.PathOperations != 0 ||
			transition.SymbolOperations != 0 {
			response.TableTransition = transition.Bytes
		} else {
			transitionMode = "reuse"
		}
		if s.trace != nil {
			s.trace.Stage("analyze-transport-"+transitionMode, time.Since(transportStarted))
		}
		if err := ctx.Err(); err != nil {
			return fail("analysis-cancelled", err)
		}
		s.retained.token = nextToken
		s.retained.tokenText = nextTokenText
		demandTransaction.commit()
		demandsPublished = true
		table := *analyzedTable
		s.retained.table = &table
		if s.schema >= TypeFactsSchemaVersionV6 {
			if err := s.populateSymbolEvidence(ctx, request, &response, nil); err != nil {
				return fail("analysis-failed", err)
			}
			s.closure.releaseTransportRows()
			// retained.table is an intentionally detached header used to
			// authenticate the next transition. Clear its borrowed expanded
			// slices too; otherwise the header alone pins the transferred
			// backing arrays even though the closure released its copy.
			s.retained.table.Entities = nil
			clear(s.retained.table.entityRuns)
			s.retained.table.entityRuns = nil
			s.retained.table.Files = nil
			s.retained.table.sourceDigests = nil
		}
		analysisPublished = true
	case LifecycleSymbols:
		if s.schema < TypeFactsSchemaVersionV6 {
			return fail("invalid-request", errors.New("symbol evidence requires TypeFacts schema v6 or newer"))
		}
		if request.Generation != generation {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
		if request.StateToken == "" || request.StateToken != s.retained.tokenText {
			return fail("state-mismatch", fmt.Errorf(
				"%w: symbol evidence token %q, retained token %q",
				ErrGenerationMismatch,
				request.StateToken,
				s.retained.tokenText,
			))
		}
		if err := s.populateSymbolEvidence(ctx, request, &response, arena); err != nil {
			if ctx.Err() != nil {
				return fail("analysis-cancelled", ctx.Err())
			}
			return fail("analysis-failed", err)
		}
	case LifecycleSources:
		if request.Generation != generation {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
		sources, err := s.closure.SourceFiles(ctx)
		if err != nil {
			return fail("sources-failed", err)
		}
		arena, descriptors, lengths, err := s.writeSourceArena(sources)
		if err != nil {
			return fail("sources-failed", err)
		}
		response.SourceArena = arena
		response.Sources = descriptors
		response.SourceLengths = lengths
	case LifecycleCancel:
		// Cancellation is delivered through the active request's context by
		// the transport adapter. This operation acknowledges that delivery.
	case LifecycleClose:
		if request.Generation != generation {
			return fail("generation-mismatch", ErrGenerationMismatch)
		}
		if err := s.Close(); err != nil {
			return fail("close-failed", err)
		}
		response.OK = true
		return response
	}
	response.OK = true
	return response
}

func (s *Session) populateSymbolEvidence(
	ctx context.Context,
	request LifecycleRequest,
	response *LifecycleResponse,
	arena TransitionArena,
) error {
	evidence, err := s.closure.resolveSymbolEvidence(ctx, request.SymbolQueries)
	if err != nil {
		return err
	}
	if request.Operation == LifecycleSymbols && len(evidence) != 0 {
		sort.Slice(evidence, func(i, j int) bool { return evidence[i].ID < evidence[j].ID })
		input := wireTransitionInput{
			ProjectID: s.projectID,
			Target: &FactTable{
				Schema:     TypeFactsSchemaVersion,
				Generation: request.Generation,
				ProjectID:  s.projectID,
				Symbols:    evidence,
			},
		}
		var packed encodedWireTransition
		if arena == nil {
			packed, err = s.transition.Encode(input)
		} else {
			packed, err = s.transition.EncodeInto(input, arena)
		}
		if err != nil {
			return fmt.Errorf("pack symbol evidence: %w", err)
		}
		response.TableTransition = packed.Bytes
	} else {
		response.SymbolEvidence = evidence
	}
	if request.ReferenceChanges {
		ids, exact, err := s.closure.changedReferences(ctx)
		if err != nil {
			return err
		}
		response.ChangedReferenceSymbols = ids
		response.ReferenceChangesExact = exact
		if exact && len(ids) != 0 {
			queries := make([]SymbolQueryV6, len(ids))
			for index, id := range ids {
				queries[index] = SymbolQueryV6{ID: id, References: true}
			}
			response.ReferenceEvidence, err = s.closure.resolveSymbolEvidence(ctx, queries)
			if err != nil {
				return err
			}
			if len(request.ReferencePaths) != 0 {
				paths := make(map[string]struct{}, len(request.ReferencePaths))
				for _, path := range request.ReferencePaths {
					paths[filepath.Clean(path)] = struct{}{}
				}
				for index := range response.ReferenceEvidence {
					fact := &response.ReferenceEvidence[index]
					kept := fact.References[:0]
					for _, location := range fact.References {
						if _, ok := paths[location.Path]; ok {
							kept = append(kept, location)
						}
					}
					fact.References = kept
				}
			}
		}
	}
	if request.ReleaseAnalysis {
		s.closure.releaseBackendAnalysisState()
		// ReleaseAnalysis marks a cold materialization boundary: the immutable
		// transport table now belongs to the client and checker-expanded query
		// state has just been discarded. Force a collection here so the Go
		// runtime returns those now-unreachable pages instead of retaining a
		// cold-sized heap for the rest of the editor session.
		debug.FreeOSMemory()
	}
	return nil
}

func (s *Session) Close() error {
	if s.closed {
		return s.closeErr
	}
	s.closed = true
	s.closeErr = errors.Join(s.closure.Close(), removeSourceArena(s.sourceArenaPath))
	return s.closeErr
}

func (s *Session) writeSourceArena(sources []SourceFile) (string, []SourceFileV3, []uint64, error) {
	if err := removeSourceArena(s.sourceArenaPath); err != nil {
		return "", nil, nil, err
	}
	s.sourceArenaPath = ""
	file, err := os.CreateTemp("", "solid-typefacts-sources-*")
	if err != nil {
		return "", nil, nil, err
	}
	path := file.Name()
	keep := false
	defer func() {
		_ = file.Close()
		if !keep {
			_ = os.Remove(path)
		}
	}()
	writer := bufio.NewWriterSize(file, 1<<20)
	descriptors := make([]SourceFileV3, 0, len(sources))
	lengths := make([]uint64, 0, len(sources))
	for _, source := range sources {
		length := uint64(len(source.Source))
		if _, err := writer.Write(source.Source); err != nil {
			return "", nil, nil, err
		}
		descriptors = append(descriptors, SourceFileV3{Path: source.Path})
		lengths = append(lengths, length)
	}
	if err := writer.Flush(); err != nil {
		return "", nil, nil, err
	}
	if err := file.Close(); err != nil {
		return "", nil, nil, err
	}
	keep = true
	s.sourceArenaPath = path
	return path, descriptors, lengths, nil
}

func removeSourceArena(path string) error {
	if path == "" {
		return nil
	}
	err := os.Remove(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	return err
}
