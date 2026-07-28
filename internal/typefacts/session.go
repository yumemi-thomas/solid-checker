package typefacts

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
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
	projectID = filepath.Clean(projectID)
	if projectID == "" || projectID == "." {
		_ = backend.Close()
		return nil, errors.New("Type Facts session requires a project identity")
	}
	closure, err := NewDemandClosure(backend, trace)
	if err != nil {
		_ = backend.Close()
		return nil, err
	}
	return &Session{
		closure:   closure,
		trace:     trace,
		projectID: projectID,
	}, nil
}

func (s *Session) Lifecycle(ctx context.Context, request LifecycleRequest) LifecycleResponse {
	return s.lifecycle(ctx, request)
}

func (s *Session) lifecycle(ctx context.Context, request LifecycleRequest) LifecycleResponse {
	generation := s.closure.generation
	response := LifecycleResponse{
		Schema: TypeFactsSchemaVersionV5, RequestID: request.RequestID,
		ProjectID: s.projectID, Generation: generation,
	}
	fail := func(code string, err error) LifecycleResponse {
		response.Error = &LifecycleError{Code: code, Message: err.Error()}
		return response
	}
	if err := ValidateLifecycleRequest(request); err != nil {
		return fail("invalid-request", err)
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
		if request.CompactDemands != nil {
			if len(request.Demands) != 0 {
				return fail("invalid-demands", fmt.Errorf("analyze carries both demands and compactDemands"))
			}
			expanded, err := request.CompactDemands.Expand()
			if err != nil {
				return fail("invalid-demands", err)
			}
			request.Demands = expanded
		}
		// Analyze is always retained-state scoped: a caller either resets the
		// state or presents the token the previous analyze handed back.
		if !request.ResetState && request.StateToken != s.retained.tokenText {
			return fail("state-mismatch", ErrGenerationMismatch)
		}
		if !request.ResetState &&
			len(request.Demands) == 0 &&
			len(request.RemovedDemandPaths) == 0 &&
			s.retained.table != nil &&
			s.retained.table.Generation == generation {
			response.StateToken = s.retained.tokenText
			response.Timings = &LifecycleTimings{}
			response.OK = true
			return response
		}
		demandTransaction := s.retained.demands.begin(
			request.Demands,
			request.RemovedDemandPaths,
			request.ResetState,
		)
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
		}
		if !request.ResetState && s.retained.table != nil {
			transitionInput.Base = s.retained.table
			transitionInput.BaseStateToken = s.retained.tokenText
		}
		transition, err := s.transition.Encode(transitionInput)
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
		analysisPublished = true
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
