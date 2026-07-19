package typefacts

import (
	"context"
	"errors"
	"fmt"
	"path/filepath"
	"time"
)

// ReuseFloorReport prices the cross-generation reuse design before it is
// built (the durable-identity kill-test): after an edit, how much of the
// previous generation's closure could be carried over, and what would a
// reuse-based rebuild cost at minimum? Components are measured on a real
// fresh generation; the floor is their sum. This is experiment
// instrumentation, not a production path.
type ReuseFloorReport struct {
	AffectedFiles int `json:"affectedFiles"`
	TotalFiles    int `json:"totalFiles"`

	// Identity remap: previous-generation symbols re-resolved in the new
	// generation at their declaration sites (unaffected files only —
	// affected-file symbols are recomputed by seeding anyway).
	RemapCandidates  int   `json:"remapCandidates"`
	RemapResolved    int   `json:"remapResolved"`
	RemapNoDecl      int   `json:"remapNoDecl"`
	RemapAffected    int   `json:"remapAffectedDecl"`
	RemapFailed      int   `json:"remapFailed"`
	RemapNanos       int64 `json:"remapNs"`
	RefIndexNanos    int64 `json:"refIndexNs"`
	RefsAffected     int   `json:"refsAffectedSymbols"`
	RefsAffectedNano int64 `json:"refsAffectedNs"`
	SeedAffectedNano int64 `json:"seedAffectedNs"`

	FloorNanos int64 `json:"floorNs"`
	FullNanos  int64 `json:"fullRebuildNs"`
}

// MeasureReuseFloor runs one experiment step on a fresh generation that was
// produced by `affected`. previous is the last generation's table (wire
// slices intact).
func MeasureReuseFloor(ctx context.Context, backend Project, fused FileFactsDiscoverer, previous *FactTable, affected AffectedSet) (ReuseFloorReport, error) {
	full, ok := backend.(ClosureBackend)
	if !ok {
		return ReuseFloorReport{}, errors.New("reuse floor requires the closure backend capabilities")
	}
	report := ReuseFloorReport{AffectedFiles: len(affected.Files), TotalFiles: len(previous.Files)}
	affectedSet := make(map[string]struct{}, len(affected.Files))
	for _, path := range affected.Files {
		affectedSet[filepath.Clean(path)] = struct{}{}
	}

	// 1. Identity remap for symbols declared in unaffected files.
	symbols := previous.symbolFactsSlice()
	remap := make(map[SymbolID]SymbolID, len(symbols))
	started := time.Now()
	for _, fact := range symbols {
		report.RemapCandidates++
		if len(fact.Declarations) == 0 {
			report.RemapNoDecl++
			continue
		}
		location := fact.Declarations[0].Location
		if _, hit := affectedSet[filepath.Clean(location.Path)]; hit {
			report.RemapAffected++
			continue
		}
		symbol, err := full.SymbolAt(ctx, location)
		if err != nil {
			if ctx.Err() != nil {
				return report, err
			}
			// Any resolution failure is a remap miss — a data point for the
			// kill-test, not a fault.
			report.RemapFailed++
			continue
		}
		remap[fact.ID] = symbol
		report.RemapResolved++
	}
	report.RemapNanos = time.Since(started).Nanoseconds()

	// 2. Per-generation reference index cost (paid once by any design).
	started = time.Now()
	for _, fact := range symbols {
		if target, ok := remap[fact.ID]; ok {
			if _, err := full.References(ctx, target); err != nil && !errors.Is(err, ErrNotFound) {
				return report, err
			}
			break
		}
	}
	report.RefIndexNanos = time.Since(started).Nanoseconds()

	// 3. Reference recompute for carried-over symbols whose previous lists
	// intersect affected files (their lists may have changed).
	started = time.Now()
	for _, fact := range symbols {
		target, ok := remap[fact.ID]
		if !ok || len(fact.References) == 0 {
			continue
		}
		touched := false
		for _, reference := range fact.References {
			if _, hit := affectedSet[filepath.Clean(reference.Path)]; hit {
				touched = true
				break
			}
		}
		if !touched {
			continue
		}
		report.RefsAffected++
		if _, err := full.References(ctx, target); err != nil && !errors.Is(err, ErrNotFound) {
			return report, err
		}
	}
	report.RefsAffectedNano = time.Since(started).Nanoseconds()

	// 4. Seed and close only the affected files (fresh facts the reuse
	// design must always compute).
	started = time.Now()
	builder := &closureBuilder{
		backend:     full,
		fused:       fused,
		scanCache:   make(map[string][]scanSeed),
		entities:    make(map[Location]*EntityFact),
		symbolSeen:  make(map[SymbolID]struct{}),
		fullTier:    make(map[SymbolID]struct{}),
		descriptors: make(map[SymbolID]*TypeDescriptor),
		cleanPaths:  make(map[string]string),
	}
	sources, err := full.SourceFiles(ctx)
	if err != nil {
		return report, err
	}
	for _, file := range sources {
		if _, hit := affectedSet[filepath.Clean(file.Path)]; !hit {
			continue
		}
		if _, err := builder.seedFile(ctx, file); err != nil {
			return report, fmt.Errorf("seed affected %s: %w", file.Path, err)
		}
	}
	if _, err := builder.closeSymbols(ctx); err != nil {
		return report, fmt.Errorf("close affected symbols: %w", err)
	}
	report.SeedAffectedNano = time.Since(started).Nanoseconds()

	report.FloorNanos = report.RemapNanos + report.RefIndexNanos + report.RefsAffectedNano + report.SeedAffectedNano
	return report, nil
}
