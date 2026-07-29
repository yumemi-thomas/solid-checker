# Reactive IR performance certification — 2026-07-29

Machine: Apple Silicon macOS host, 14 logical processors. Corpus:
deterministic 5,000-file corpus at `/tmp/bench-corpus-5k`. Every cold
comparison used alternating control/candidate process launches with the same
Type Facts binary and corpus.

## Accepted changes

### Validated Type Facts-side patch

The supplied patch parallelizes immutable summary-node, graph-contribution,
result-read, and contract-export discovery. Cache mutation and ordered merging
remain sequential.

Median of three alternating cold pairs:

| Metric | Control | Patched | Change |
|---|---:|---:|---:|
| First analysis | 1,106.4 ms | 1,019.5 ms | -7.9% |
| Reactive IR | 409.7 ms | 338.5 ms | -17.4% |
| Interprocedural graph | 55.3 ms | 22.1 ms | -60.0% |
| Contract exports | 45.7 ms | 25.9 ms | -43.3% |
| First response | 5,247,633 B | 5,247,633 B | unchanged |

Thirty pooled incremental samples from three alternating sessions measured
79.99 ms median / 88.26 ms p95 for the control and 78.72 ms median / 82.97 ms
p95 for the patch. Both produced 414-byte same-span edit responses.

### Ordered parallel source discovery

Dirty files are selected sequentially. Immutable per-file contributions are
computed with an ordered parallel map only when at least 256 files are dirty.
Cache replacement, changed-symbol accumulation, and aggregate merging remain
in source order. One-file edits therefore take the sequential path.

Five alternating pairs before the worker-budget integration measured:

| Metric | Patched baseline | Source-parallel | Change |
|---|---:|---:|---:|
| First analysis | 1,012.7 ms | 950.3 ms | -6.2% |
| Reactive IR | 339.1 ms | 274.6 ms | -19.0% |
| Source discovery | 86.2 ms | 23.2 ms | -73.1% |

### Reactive IR worker budget

A small Reactive IR-owned worker budget caps ordered maps inside the two
concurrent source/reachability and local/interprocedural lanes. It does not
create a permanent pool or retain worker scratch state. Maps preserve input
order, workloads below 256 items remain sequential, and a focused test checks
ordered sequential/parallel equivalence plus the worker cap.

Five alternating pairs against the uncapped source-parallel build measured
959.06 ms versus 958.85 ms first analysis and 278.69 ms versus 280.36 ms
Reactive IR. The end-to-end result was unchanged and the 0.6% Reactive IR
difference was within observed noise; the budget was retained to prevent
machine-sized nested worker sets.

## Rejected experiments

| Experiment | Isolated result | Complete-pipeline result | Decision |
|---|---|---|---|
| Static half-machine cap for every ordered map | Graph/export stages slowed | Reactive IR +3.4%; first analysis +1.0% | Rejected |
| Drop the cached project-wide local-access aggregate and rebuild it from per-file contributions | Removes one retained clone of local-access findings | One-file edit median 80.5 ms → 100.6 ms (+25.0%); local-access stage 1.0 ms → 20.8 ms | Rejected; the aggregate is an intentional memory-for-latency tradeoff |
| Cache cleanup-return results per file when late-stage inputs are reusable | Cleanup stage -6.8%; Reactive IR -2.6% | Three alternating 20-sample sessions: end-to-end median only -0.2%, with one worse p95 | Rejected; owned diagnostic rematerialization dominates and the complete-pipeline win is within noise |
| Dense CSR-style reverse adjacency | Removed per-node adjacency allocations; propagation flat | Reactive IR +1.4%; first analysis +0.6% | Rejected and removed |
| Shared `Arc<ContractExport>` fragments | Contract export -20.7%; interprocedural -4.8% | Reactive IR +0.3%; first analysis +0.3% over seven pairs | Rejected and removed |
| Asymmetric 1:2 source/reachability budget | Reachability improved, source slowed | Reactive IR +2.6% | Rejected and removed |

The dense graph experiment covered sorted forward/reverse ordering, duplicate
edges, and empty ranges, but its representation was removed with the rejected
implementation. Existing fixed-point tests continue to cover first-writer and
returned-edge ordering; workspace session tests cover fresh/incremental
equivalence and cross-file contract invalidation.

## Final certification

Median of three fresh final runs:

| Metric | Final |
|---|---:|
| First analysis | 960.7 ms |
| Reactive IR | 281.5 ms |
| Source discovery | 25.6 ms |
| Interprocedural graph | 23.9 ms |
| Contract exports | 25.5 ms |
| Cached analysis | 1.09 ms |
| Cached Reactive IR | 0.0006 ms |
| First response | 5,247,633 B |

Relative to the original pre-patch control, final first analysis improved
13.2% and Reactive IR improved 31.3%. Relative to the supplied-patch baseline,
the final improvement was 5.8% first analysis and 16.8% Reactive IR.

The final 30-sample one-file same-span run measured 79.92 ms median and
81.24 ms p95. Its median is 1.5% above the best supplied-patch sample and below
the 3% regression ceiling. Paired control/candidate runs produced identical
payload sizes.

The repository performance certification passed with 52,279 ns/source first
Reactive IR, 3,125 ns cached Reactive IR, 19.34 ms incremental analysis,
917 Type Facts bytes/source, and 2.00x contract-export scaling from 500 to
1,000 files.

The final retained 5,000-file process measured:

- checker live heap: 2,939,035 allocations / 441,630,544 bytes;
- checker physical footprint: 529.0 MiB, 530.9 MiB peak;
- checker plus Type Facts physical footprint: 846.2 MiB;
- checker plus Type Facts RSS: 1,104.9 MiB.

No accepted structural representation change reduced retained allocations, so
the requested 20% allocation-reduction target is not claimed. The absolute
retained measurements above are the certification baseline for future dense
identity/index work.

## Verification

The final checkout passed:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets`
- `cargo test --workspace`
- protocol, transport, session, contract, diagnostic, and rule-quality tests
- CLI tests
- bundled-contract conformance
- performance-workflow tests
- shell and JSON syntax checks
- repository performance certification

## Latest Type Facts v6 follow-up

After the initial certification, the following Type Facts v6 measurements were
taken at `fa66528b9d211cc50ca47b156db8adec8ab4c21d` (`typefacts` v0.6.0).
Solid Checker now pins its Rust client, Cargo lockfile, checked-out Go Producer,
and rebuilt `bin/solid-typefacts` to the published follow-up
`4a1800a44ecefb805ba0798f8318b306387ba85b`.

Median of three fresh 5,000-file runs with v6:

| Metric | Type Facts v6 |
|---|---:|
| First analysis | 932.1 ms |
| Reactive IR | 270.4 ms |
| Type Facts roundtrip | 421.8 ms |
| Type Facts server analysis | 393.2 ms |
| First response | 3,575,177 B |
| Cached analysis | 1.14 ms |
| Cached Reactive IR | 0.0008 ms |

The 10-sample one-file same-span run measured 80.07 ms median and 81.72 ms
p95, with a 7,163-byte median incremental response.

The repository performance certification passed again with 52,386 ns/source
first Reactive IR, 375 ns cached Reactive IR, 20.33 ms incremental analysis,
695 Type Facts bytes/source, and 1.95x contract-export scaling.

The v6 retained 5,000-file actor measured 1,033.1 MiB RSS and 835.1 MiB
physical footprint. Rust accounted for 613.4 MiB RSS / 482.4 MiB physical;
Type Facts accounted for 419.7 MiB RSS / 352.7 MiB physical. The Rust malloc
zone retained 3,054,054 allocations with 430.7 MiB live. The Type Facts heap
profile retained 273.2 MiB; its largest attributable subsystem was retained
semantic-demand materialization (89.6 MiB cumulative), followed by the
reference index (27.1 MiB cumulative).

Fresh-process startup is acceptable for a command but too visible for an
interactive editor fallback. Across five process-cold samples with warm OS
caches, the 5,000-file median was 1,218.4 ms wall time: 625.4 ms Type Facts,
283.9 ms Reactive IR, 72.5 ms solve/snapshot, and 105.9 ms sidecar spawn. The
first observed sample was 1,853.5 ms.

### Retained IR ownership follow-up

Cache-by-cache heap attribution showed that the large retained indexes are not
dead weight. Clearing source discovery, TypeScript indexes, reachability, the
local-access aggregate, or owner fragments saved memory but added roughly
21 ms, 34 ms, 20 ms, 116 ms, and 46 ms respectively to the edit shapes that
need them. Clearing interprocedural graph/result caches was free for a
same-span edit only because the late aggregate remained valid; it added about
40 ms to a span-shifting edit.

The accepted refactor therefore keeps the caches and changes their ownership.
Repeated text in read/write/action/async records now uses shared immutable
storage. Per-file and project-wide local-access caches share whole records,
and the retained interprocedural aggregate shares its immutable result slices.
The JSON representation and ordering are unchanged.

On the same 5,000-file corpus, exact checker live heap fell from 451,606,160
bytes to 437,662,304 bytes (-3.1%). A 12-sample same-span edit run improved
from 80.90 ms to 77.67 ms median; an eight-sample span-shifting run improved
from 383.37 ms to 380.10 ms median. Serialized payloads remain covered by the
fresh/incremental equivalence tests.

### Canonical path ownership follow-up

The next pass confirmed that Type Facts v6 already interns paths while decoding
its packed string table. Solid Checker's own `SourcePath`, retained cache keys,
and newly constructed `Location` values still allocated independent copies,
however. `SourcePath` now owns `Arc<str>` text, nine per-file Reactive IR maps
reuse that allocation as their key, and file-derived locations clone the same
canonical path.

Exact 5,000-file checker heap fell again from 437,662,320 bytes / 2,959,065
allocations to 426,928,048 bytes / 2,734,072 allocations. Relative to the
pre-ownership v6 baseline, the combined reduction is 24,678,112 bytes (-5.5%)
and 319,982 allocations (-10.5%). Same-span and span-shifting medians changed
by +0.9% and +0.3% respectively in direct A/B runs, within observed noise;
first analysis improved by roughly 2–3%.

Type Facts revision `4a1800a44ecefb805ba0798f8318b306387ba85b`
removes the v6-only retained path-to-symbol-roots table, which has no reader
after Rust takes symbol ownership. The change passed the complete Go suite and
reduced the Type Facts process by roughly 5–6 MiB in alternating measurements
with edit latency unchanged. Solid Checker now pins that published revision for
both its Rust client and bundled Producer.

### Retained-memory profile: large opportunities only

Allocation-stack profiling on the 5,000-file corpus measured a stable checker
live heap of 423,651,248 bytes in 2,734,072 allocations. Two page-reclamation
experiments found large physical-memory wins without changing retained
semantics:

- Calling `debug.FreeOSMemory` after the v6 `ReleaseAnalysis` boundary reduced
  the Type Facts Producer from 354.5–354.6 MiB to 259.1–259.9 MiB physical.
- Calling macOS `malloc_zone_pressure_relief` after a completed diagnostic
  reduced the checker from 511–513 MiB to 468–470 MiB physical.
- Together they measured 725.3 MiB physical for the process tree, down from
  roughly 859 MiB (-133 MiB, -15.5%). Median cold analysis increased from
  1,328.8 ms to 1,356.5 ms (+2.1%). Producer reclamation increased the
  incremental certificate from roughly 19–20 ms to 23.3 ms.

The Reactive IR incremental caches are the other large ownership boundary.
Dropping every cache while retaining the current Program and diagnostic
snapshot reduced checker live heap by 199,356,464 bytes (190.1 MiB). Independent
cache-family measurements found only four large groups:

| Cache family | Unique retained heap |
|---|---:|
| Late stages (local access and owner state) | 56.2 MiB |
| Interprocedural graph and results | 55.4 MiB |
| Reachability | 33.8 MiB |
| TypeScript indexes | 30.2 MiB |

AST indexes, source discovery, and typed-accessor caches were individually
below the large-opportunity threshold. Previous recomputation experiments put
the corresponding edit costs at about +116 ms for local aggregation, +46 ms
for owner state, +40 ms for interprocedural state, +34 ms for TypeScript
indexes, and +20 ms for reachability. A memory-tier policy should therefore
keep late-stage state first and evict the lower-cost index/reachability groups
under memory pressure, rather than discarding all incremental state.

The next structural refactor worth considering is a packed, path-local Type
Facts representation. Rust allocation stacks attribute 48.8 MiB to retained
per-path fact rows and 31.7 MiB to demand planning plus the Type Facts
session's duplicate owned demand runs. Sharing or packing the latter and
keeping entity rows columnar could plausibly remove tens of MiB. In contrast,
the final post-release Go profile leaves the reference index at about 12.9 MiB;
it is no longer a large target.

### Implemented retained-memory policy

The production daemon now applies the two directly validated lifecycle wins:

- after a newly materialized response is flushed, it asks the platform
  allocator to return free pages (`malloc_zone_pressure_relief` on macOS and
  `malloc_trim` on glibc Linux); cache hits stay off this path;
- projects with at least 1,000 source files default to balanced Reactive IR
  retention, releasing the interprocedural, TypeScript-index, and reachability
  cache families while preserving the current coherent program and diagnostic;
- `SOLID_CHECKER_CACHE_RETENTION=performance|balanced|compact` makes the
  latency/memory choice explicit. Compact retention releases every derived IR
  index while keeping exact same-generation reuse.

The pinned Type Facts producer already reclaims Go pages after writing a large
materialized lifecycle response. Profiling showed that transport-side placement
did not make the released semantic graph unreachable soon enough, so the
checked-out producer also performs the collection directly after
`ReleaseAnalysisState`. On the 5,000-file corpus that placement reduced the
producer from 354.1 MiB to 255.4 MiB physical.

With balanced Reactive IR retention, allocator lifecycle hooks, packed retained
entity rows, and shared Rust demand runs, the published and pinned build
measured 547.7 MiB physical in total: 292.2 MiB for the checker and 255.5 MiB
for Type Facts. That is about 311 MiB (36%) below the roughly 859 MiB baseline.
The performance certification passes at 22.0 ms median one-file analysis, with
695 Type Facts response bytes per source and unchanged wire shape. The balanced
tier deliberately recomputes its released cache families after an edit.

## CLI and ESLint-plugin readiness

For the intended product scope—CLI checks plus the ESLint/Oxlint plugin—the
engine is ready for large-project use. It provides deterministic project-wide
diagnostics, daemon reuse across lint invocations, about 78 ms median
same-span one-file updates, about 1.1 ms unchanged requests, bounded idle/RSS
process policy, restart/replay coverage, and small incremental Type Facts
responses. An LSP is not required for this scope.

The remaining caveat is density, not correctness: the optimized synthetic
5,000-file actor retains about 584 MiB physical across checker and Type Facts.
The watchdog is per project actor, so several large tsconfigs can still
multiply that footprint. Fresh one-shot CLI latency is also around 1.2 seconds
on this corpus; the daemon remains important for repeated ESLint runs.

The stage timer boundaries were corrected during this audit: `staticApi`
previously measured cleanup analysis, while `directives` combined static API
and directive work. This was telemetry-only and did not change analysis.

## Recommended order of work

1. Add a parent process budget across project actors, with LRU/idle eviction,
   so monorepo linting has one enforceable memory ceiling rather than one
   ceiling per tsconfig.
2. Intern project paths at hydration and use session-local numeric IDs in
   retained indexes. `Location` already shares its path representation, but
   independently decoded/constructed locations do not yet share one canonical
   allocation.
3. Reduce Type Facts retained semantic-demand materialization and its reference
   index; together they are the largest attributed producer-side opportunity.
4. Split typed accessor/prop-root discovery into replaceable per-file
   fragments with explicit global dependency fingerprints. It is the largest
   remaining single edit-path IR stage.
5. Cache cleanup-return output only after making its results shareable; the
   rejected per-file cache showed that scan avoidance without shared
   rematerialization does not improve the complete pipeline.
6. Replace repeated string-keyed retained maps with compact session IDs, guided
   by allocation profiles. Do not revive the rejected dense graph
   representation unchanged; it improved shape but regressed the complete
   pipeline.
