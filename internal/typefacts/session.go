package typefacts

import (
	"bufio"
	"cmp"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strconv"
	"time"
)

var ErrSessionClosed = errors.New("Type Facts session is closed")

// Session owns one retained Type Facts analysis lifetime. Its interface
// concentrates project identity, generation, retained demand state, wire
// table selection, and project closure behind the v2 and v3 request shapes.
//
// Calls are dispatched serially by the protocol adapter. Cancellation may
// arrive concurrently by cancelling the context of the active call.
type Session struct {
	closure             *ClosureProject
	projectID           string
	retained            retainedSessionState
	retainedDiagnostics SessionDiagnostics
	inlineSources       map[string]struct{}
	sourceArenaPath     string
	closed              bool
	closeErr            error
}

type retainedSessionState struct {
	token     uint64
	tokenText string
	demands   map[string][]EntityDemand
	table     *FactTable
}

// SessionDiagnostics carries adapter-facing observability without coupling
// the session to environment variables or stderr.
type SessionDiagnostics struct {
	RequestID         uint64
	Updated           bool
	Analyzed          bool
	OperationDuration time.Duration
	Closure           ClosureStats
}

// NewSession assumes ownership of backend, including when construction fails.
func NewSession(backend Project, projectID string, fallback bool) (*Session, error) {
	projectID = filepath.Clean(projectID)
	if projectID == "" || projectID == "." {
		_ = backend.Close()
		return nil, errors.New("Type Facts session requires a project identity")
	}
	closure, err := NewClosureProject(backend, fallback)
	if err != nil {
		_ = backend.Close()
		return nil, err
	}
	return &Session{
		closure:       closure,
		projectID:     projectID,
		inlineSources: make(map[string]struct{}),
	}, nil
}

func (s *Session) Closure(ctx context.Context, request ClosureRequest) (ClosureResponse, error) {
	if s.closed {
		return ClosureResponse{}, ErrSessionClosed
	}
	if filepath.Clean(request.ProjectID) != s.projectID {
		return ClosureResponse{}, ErrGenerationMismatch
	}
	return s.closure.ClosureResponseFor(ctx, request)
}

func (s *Session) Lifecycle(ctx context.Context, request LifecycleRequest) LifecycleResponse {
	return s.lifecycle(ctx, request)
}

func (s *Session) Diagnostics(requestID uint64) SessionDiagnostics {
	if s.retainedDiagnostics.RequestID != requestID {
		return SessionDiagnostics{}
	}
	return s.retainedDiagnostics
}

func (s *Session) lifecycle(ctx context.Context, request LifecycleRequest) LifecycleResponse {
	generation := s.closure.generation
	response := LifecycleResponse{
		Schema: TypeFactsSchemaVersionV3, RequestID: request.RequestID,
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
		started := time.Now()
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
		for _, change := range changes {
			path := filepath.Clean(change.Path)
			if change.Deleted {
				delete(s.inlineSources, path)
			} else {
				s.inlineSources[path] = struct{}{}
			}
		}
		s.retainedDiagnostics = SessionDiagnostics{
			RequestID:         request.RequestID,
			Updated:           true,
			OperationDuration: time.Since(started),
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
		stateful := request.ResetState || request.StateToken != "" || len(request.RemovedDemandPaths) != 0
		nextDemands := s.retained.demands
		if stateful {
			if !request.ResetState && request.StateToken != s.retained.tokenText {
				return fail("state-mismatch", ErrGenerationMismatch)
			}
			nextDemands = applySessionDemandChanges(s.retained.demands, request.Demands, request.RemovedDemandPaths, request.ResetState)
			if !request.ResetState &&
				len(request.Demands) == 0 &&
				len(request.RemovedDemandPaths) == 0 &&
				s.retained.table != nil &&
				s.retained.table.Generation == generation {
				response.TableMode = TableModeReuse
				response.StateToken = s.retained.tokenText
				response.Timings = &LifecycleTimings{}
				response.OK = true
				return response
			}
		}
		started := time.Now()
		buildSequence := s.closure.Stats().BuildSequence
		var analyzed ClosureResponse
		var analyzedTable *FactTable
		var err error
		if stateful {
			analyzedTable, err = s.closure.DemandTableForGroups(
				ctx,
				generation,
				sessionDemandGroups(nextDemands),
				sessionDemandChangedPaths(request.Demands, request.RemovedDemandPaths),
			)
		} else if len(request.Demands) != 0 {
			analyzed, err = s.closure.DemandResponseFor(ctx, s.projectID, generation, request.Demands)
		} else {
			spans := append(append([]LocationV2(nil), request.StructuralSpans...), request.CompilerSpans...)
			slices.SortFunc(spans, func(a, b LocationV2) int {
				return cmp.Or(cmp.Compare(a.Path, b.Path), cmp.Compare(a.StartByte, b.StartByte), cmp.Compare(a.EndByte, b.EndByte))
			})
			v2 := ClosureRequest{
				Schema: TypeFactsSchemaVersionV2, ProjectID: s.projectID, Generation: generation,
				RulesetVersion: 1, CompilerSpans: spans,
			}
			analyzed, err = s.closure.ClosureResponseFor(ctx, v2)
		}
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
		s.retainedDiagnostics = SessionDiagnostics{
			RequestID:         request.RequestID,
			Analyzed:          true,
			OperationDuration: elapsed,
			Closure:           stats,
		}
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
			response.Timings.PrepareNs = uint64(stats.PrepareDuration)
			response.Timings.RetainedFiles = uint64(stats.Retention.RetainedFiles)
			response.Timings.RecomputedFiles = uint64(stats.Retention.RecomputedFiles)
			response.Timings.NonDurableFiles = uint64(stats.Retention.NonDurableFiles)
		}
		if stateful {
			nextToken := s.retained.token + 1
			nextTokenText := strconv.FormatUint(nextToken, 10)
			response.StateToken = nextTokenText
			if request.ResetState || s.retained.table == nil || stats.Retention.NonDurableFiles != 0 {
				response.TableMode = TableModeFull
				packed, err := PackedFactTableV3From(FactTableV2From(*analyzedTable, s.projectID, generation))
				if err != nil {
					return fail("assembly-failed", err)
				}
				response.PackedTable = packed
			} else {
				delta := DiffFactTablesV3FromInternal(*s.retained.table, *analyzedTable, generation)
				if s.retained.table.Generation == analyzedTable.Generation && delta.Empty() {
					response.TableMode = TableModeReuse
				} else {
					response.TableMode = TableModeDelta
					response.TableDelta = &delta
				}
			}
			s.retained.token = nextToken
			s.retained.tokenText = nextTokenText
			s.retained.demands = nextDemands
			table := *analyzedTable
			s.retained.table = &table
		} else {
			response.Table = &analyzed.Table
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

func applySessionDemandChanges(previous map[string][]EntityDemand, changes []EntityDemand, removed []string, reset bool) map[string][]EntityDemand {
	next := make(map[string][]EntityDemand)
	if !reset {
		for path, demands := range previous {
			next[path] = demands
		}
	}
	changed := make(map[string][]EntityDemand)
	for _, demand := range changes {
		path := filepath.Clean(demand.Location.Path)
		changed[path] = append(changed[path], demand)
	}
	for path, demands := range changed {
		next[path] = demands
	}
	for _, path := range removed {
		delete(next, filepath.Clean(path))
	}
	return next
}

func sessionDemandGroups(grouped map[string][]EntityDemand) []DemandGroup {
	paths := make([]string, 0, len(grouped))
	for path := range grouped {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	result := make([]DemandGroup, 0, len(paths))
	for _, path := range paths {
		result = append(result, DemandGroup{Path: path, Demands: grouped[path]})
	}
	return result
}

func sessionDemandChangedPaths(changes []EntityDemand, removed []string) []string {
	paths := make(map[string]struct{}, len(changes)+len(removed))
	for _, demand := range changes {
		paths[filepath.Clean(demand.Location.Path)] = struct{}{}
	}
	for _, path := range removed {
		paths[filepath.Clean(path)] = struct{}{}
	}
	result := make([]string, 0, len(paths))
	for path := range paths {
		result = append(result, path)
	}
	sort.Strings(result)
	return result
}
