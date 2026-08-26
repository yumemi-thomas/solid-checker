# Go/Rust performance exploration

Date: 2026-07-28  
Machine: Apple M4 Pro, darwin/arm64, Go 1.26.5, Rust 1.97.1

This exploration measured three large-refactor candidates:

1. retaining TypeScript-Go checker initialization across Generations;
2. replacing repeated path normalization with a canonical-path seam and using
   a flat result arena;
3. moving one isolated path-ID kernel from Go to Rust.

End-to-end Go results are medians of ten alternating baseline/candidate pairs
with 50 update/analyze operations per sample. Alternation controls for machine
frequency and thermal drift. The Rust kernel results are medians of ten
one-second samples.

## Result

Keep TypeScript compiler-facing work in Go and retain the canonical-path and
flat-arena changes. Pursue a safe checker prelude upstream. Do not add a
Go/Rust FFI seam for small data-processing kernels.

## 1. Checker Generation retention

A temporary TypeScript-Go fork rebased the existing checker onto the new
Program while deliberately retaining its initialized global state and caches.
This is a ceiling experiment, not a safe implementation.

| warm leaf edit → analyze | median | allocation |
| --- | ---: | ---: |
| fresh checker | 0.892 ms | ~708 KB / 3,201 |
| prototype rebase | 0.691 ms | ~385 KB / 1,902 |

The prototype is **1.29x faster**, removes about 46% of allocated bytes, and
removes about 41% of allocations. Almost the entire saving is in update time;
analysis time is unchanged.

The transactional oracle rejects the implementation. With the rebase enabled,
`TestProjectFailedUpdateIsTransactional` deadlocks in declaration emit:
`EmitResolver.PrecalculateDeclarationEmitVisibility` attempts to acquire
checker state still owned by the preceding Generation. More generally, the
checker still mixes immutable global initialization with Program-specific
nodes, symbols, emit state, diagnostics, flow state, and relation caches.

The measured result supports the existing upstream design:

```text
BuildPrelude(program) -> immutable CheckerPrelude
NewGeneration(program, prelude) -> CheckerGeneration
```

The prelude may own compiler options, intrinsic types, standard-library/global
symbols, and other dependency-proven global state. The Generation must own
Program/AST links, augmentations, inferred exports, diagnostics, flow,
emit-resolver state, and relation/type/signature caches. Candidate state must
remain unpublished until analysis succeeds.

## 2. Canonical paths and result arena

Profiles attributed 7–10% of shape-change CPU samples to `filepath.Clean`.
Most calls normalized paths already made canonical by Session demand grouping
or by the TypeScript-Go adapter. The retained implementation makes that fact an
explicit module-interface invariant and uses the canonical string itself as
the path identity. It also gives `SemanticDemandRuns` one flat entity arena and
one flat structural-symbol arena rather than two allocations per file.

| end-to-end path | baseline | final | speedup | allocation change |
| --- | ---: | ---: | ---: | ---: |
| warm leaf edit → analyze | 0.889 ms | 0.819 ms | **1.09x** | 3,202 → 3,201 |
| root-changing edit → analyze | 0.904 ms | 0.846 ms | **1.07x** | 3,204 → 3,203 |
| shape-changing edit → analyze | 15.840 ms | 13.958 ms | **1.13x** | 60,609 → 60,510 |
| cold/reset analyze | 4.928 ms | 4.382 ms | **1.12x** | 3,558 → 3,459 |

The path change supplies nearly all of the latency win. The result arena saves
about 96 allocations on broad/cold materialization and roughly 8–11 KB, with a
small additional shape-path improvement. Integer path handles are not
justified: canonical strings already provide stable comparable identities
without a second mapping layer.

## 3. Isolated Rust kernel

The throwaway prototype implemented the hot location-filtering loop over
`{path_id, start, end}` rows in both Go and a Rust static library. Three costs
were separated:

- **Go**: direct loop over prepared integer rows;
- **Rust prepared**: the same rows passed through cgo, Rust's best case;
- **Rust with crossing**: convert current string paths to integer rows, call
  Rust, and return the result.

| rows | Go | Rust prepared | Rust with crossing |
| ---: | ---: | ---: | ---: |
| 128 | 68.8 ns | 73.9 ns | 846 ns |
| 1,024 | 618 ns | 513 ns | 7.21 µs |
| 8,192 | 5.30 µs | 4.47 µs | 55.4 µs |
| 65,536 | 42.2 µs | 36.6 µs | 448 µs |

Rust is 13–17% faster once the integer representation already exists and the
batch is large enough. At 128 rows the FFI call alone makes Rust slower.
Including the representation crossing makes the Rust path **10.5–12.3x
slower** at every measured size.

The Rust implementation would become attractive only if a much deeper module
moved behind the language seam so its state stayed native across many
operations. Moving individual closure, path, or transport loops is rejected.

## Verification

The temporary checker fork and Rust kernel were deleted after recording their
answers. The retained Go changes pass the complete Go and Rust suites, the Go
race detector, and the Rust process-session/golden transition tests.
