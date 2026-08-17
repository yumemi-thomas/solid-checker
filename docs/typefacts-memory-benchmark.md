# TypeFacts memory benchmark

This benchmark separates cold materialization, retained state, cached reuse,
and incremental updates. RSS is not used as a memory target on macOS: `vmmap`
physical footprint and peak account for reclaimable Go pages, while a forced-GC
heap profile or `runtime.MemStats.HeapAlloc` measures live Go objects.

The active protocol is lifecycle schema V1 with Wire table schema v9. V1 uses
the sparse ownership path: Go extracts per-file entity rows and compact
retention evidence, while the Rust client owns the expanded symbol/reference
closure. The historical V5/V6 figures below are attribution records, not
current architecture requirements.

## Reproduction

Build the same Producer revision and Solid Checker release binary for every
comparison. Generate the 5,000-file corpus at
`/tmp/bench-corpus-5k/tsconfig.json`, then run:

```sh
make benchmark-memory

make benchmark-memory-5k \
  SOLID_CHECKER_REPO=/path/to/solid-checker \
  MEMORY_PROJECT=/tmp/bench-corpus-5k/tsconfig.json \
  MEMORY_EDIT=/tmp/bench-corpus-5k/mod4383.tsx
```

`benchmark-memory-5k` enforces the retained process-tree physical budget and
runs 30 cached and 30 incremental samples. The repository-owned tests enforce
geometric cold symbol-set growth, canonical entity backing ownership, compact
demand capacity, reference-index release, and checker identity rehydration.

For attribution work, change one major variable, rebuild, and record:

1. `vmmap -summary` Producer physical footprint and peak after cold analysis.
2. Forced-GC `HeapAlloc`, `Mallocs`, and `TotalAlloc`, plus heap profiles by
   `inuse_space`, `alloc_space`, and `alloc_objects`.
3. First-analysis wall time and Producer `serverAnalyze`.
4. Cached median and incremental median/p95, including Producer time.
5. Semantic transition bytes and outer framed response bytes.

Do not compare RSS alone, and do not use `GOMEMLIMIT`, process restarts, or GC
frequency as an optimization. The post-flush `debug.FreeOSMemory` call is kept
only to return already-dead spans to macOS; live-heap reduction must come from
ownership, compaction, and lifecycle changes.

## Attribution

Major variables were applied and measured separately:

- Geometric growth of the interned demand-closure symbol set removed the
  1.44 GiB cumulative exact-resize allocation site. In isolation, Producer
  physical peak moved from 945.9 MiB to 508.2 MiB and Producer first analysis
  moved from 670.6 ms to 537.0 ms.
- The reference index then moved from per-file maps of full `Location` values
  to grouped `int32` spans, an interned path table, and `uint8` spaces. Per-file
  construction spans are released after merge and merged rows are pruned to
  symbols that escaped into retained facts.
- Retained contributions now borrow capped windows of the canonical entity
  table, and the canonical chunked symbol store is the symbol memo. The former
  duplicate entity and `SymbolID -> SymbolFact` tables no longer survive cold
  materialization.
- TypeScript-Go checker leases, pointer-keyed symbol/query caches, cold closure
  queues, and descriptor preparation maps are released after their immutable
  facts transfer. Durable declaration references and the compact reference
  index are the narrow lazy-rehydration boundary for updates.
- The large transition frame, dictionary, plan, decoded request, and worker
  copies transfer ownership instead of cloning. The outer lifecycle map writes
  deterministic CBOR fragments around the borrowed transition bytes, so the
  complete semantic payload and a complete second encoded payload do not
  coexist.
- Rust's packed retained table is the sole owner of client-visible retained
  source rows. Go now retains only SHA-256 rows and an AST-node digest memo;
  original source bytes are materialized only for an explicit `sources`
  lifecycle response. On the same 5,000-file corpus this isolated change moved
  the Producer peak from 521.7 to 483.9 MiB and steady physical memory from
  320.6 to 308.2 MiB. Exact post-GC live heap moved from 241.2 to 238.3 MiB.
- Rust's compact demand runs now transfer directly into Go's retained query
  plan. Go expands only changed runs at the TypeScript-Go semantic-query seam
  and releases those rows immediately after extraction. The retained
  contribution module also derives descriptor seeds and closure roots from its
  canonical entities rather than cloning them, and stores reference/descriptor
  membership as entity indexes. The symbol invalidation index uses compact
  per-path ID slices instead of one hash map per path, and TypeScript-Go no
  longer retains the inverse durable-ID mint map.

The remaining Go floor is primarily the TypeScript-Go program/AST plus the
canonical entity rows required to construct V1 sparse transitions. Symbol and
reference closure rows are Rust-owned after transfer, so the producer does not
rebuild them or retain a duplicate expanded table.

## Shared transition arena and direct result ownership

The V1 process adapter now creates and owns a private transition arena. Go
streams packed rows into it in bounded 64 KiB chunks; Rust validates the
committed request identity and range before decoding. Standalone producer
responses remain inline and byte-compatible. Against the inline process
adapter, three alternating 5,000-file runs measured:

| Metric | Inline adapter | Shared arena | Change |
|---|---:|---:|---:|
| Producer peak physical memory | 464.6 MiB | 443.7 MiB | -4.5% |
| Go `HeapSys` | 438.6 MiB | 423.0 MiB | -3.6% |
| Go live heap | 249.2 MiB | 249.6 MiB | noise |
| First analysis | 1,217.9 ms | 1,214.1 ms | -0.3% |
| Incremental analysis | 84.12 ms | 84.19 ms | +0.1% |

The next direct-ingestion experiment removed two Go-side ownership stages:

- Retained-contribution preparation compacts `EntityFact` and structural-symbol
  results in place and takes ownership of the TypeScript-Go result arenas.
- Sparse V1 transport borrows those canonical per-file runs directly instead
  of copying them into a generation-wide `FactTable.Entities` slice.

The full-table allocation benchmark fell from about 7.97 MiB/op to 7.67
MiB/op (-3.8%) and removed roughly 100 allocations. Cold Go assembly fell from
about 1.24-1.29 ms to 15-22 us. The 5,000-file physical peak moved only from
443.7 MiB to 443.2 MiB and live heap was unchanged, establishing that the
remaining floor is the persistent TypeScript-Go program, AST, and binder state
rather than fact handoff. The ownership changes remain because they remove
copies, simplify the V1 lifetime, preserve exact transition bytes, and do not
regress cached or incremental latency.

## Historical V6 ownership measurements

The V6 experiment made Rust the semantic-closure owner. The active V1 path keeps
that ownership result. Go releases expanded source, entity,
file, symbol, and reference rows after transfer; it retains the TSGo program
and compact per-file extraction proof. Rust derives roots, runs alias closure,
owns reference-tier membership, and patches path-scoped reference evidence.
Symbol oracle responses reuse the packed transition dictionary codec rather
than materializing a second large CBOR object graph. See
[ADR 0009](adr/0009-v6-client-owns-expanded-path-rows.md). The measurements in
this section are historical comparisons and should not be read as a request to
restore a V5 producer path.

The comparison below uses the same 5,000-file corpus. Latency attribution uses
the same rebuilt Solid Checker checkout for both versions; physical v5 values
are the three-run medians recorded immediately before the v6 work.

| Metric | Optimized v5 | v6 | Change |
|---|---:|---:|---:|
| Producer physical peak | 493.3 MiB | 477.7 MiB | -3.2% |
| Producer steady physical | 294.4 MiB | 274.8 MiB | -6.7% |
| Go live heap | 220.1 MiB | 193.9 MiB | -11.9% |
| Allocated bytes, cold + one cached reuse | 590 MiB | 610.2 MiB | +3.4% |
| Allocation count, cold + one cached reuse | 3.482 M | 3.517 M | +1.0% |
| First analysis | 1.105 s | 1.189 s | +7.6% |
| Cached analysis median | 1.105 ms | 1.105 ms | unchanged |
| Incremental analysis median | 83.12 ms | 83.45 ms | +0.4% |
| Producer incremental median | 15.26 ms | 14.96 ms | -2.0% |
| Cold initial response | 5,310,478 B | 3,575,077 B | -32.7% |
| Cold responses, combined | 5,310,478 B | 6,434,977 B | +21.2% |
| Incremental response median | 417 B | 7,061 B | +6,644 B |

The incremental byte increase includes sparse path candidates plus invalidated
symbol declarations and affected-path reference runs. It remains about 7 KiB
and changes end-to-end latency by less than 2%. A first implementation sent
complete corpus-wide reference lists (about 1.1 MiB per edit) and materially
regressed latency. It was rejected in favor of path-scoped reference patches.
Cold analysis transfers and closes 50,003 symbols and the 35,001-symbol
reference tier in Rust. Packed dictionary frames and a references-only second
batch keep the first-analysis delta at 7.6%; cached reuse is unchanged and
incremental latency remains within 1% of optimized v5.

## 5,000-file reference result

The baseline is commit `f38e219`; values are from the same corpus and machine.
Allocation counts and live heap are post-analysis Go measurements. Physical
peak is the Producer's `vmmap` peak, not the complete checker process tree.

| Metric | Baseline | Optimized |
|---|---:|---:|
| Producer physical peak | 945.9 MiB | 477.7 MiB |
| Producer steady physical | 412 MiB | 274.8 MiB |
| Go live heap | ~345 MiB | 193.9 MiB |
| Allocated bytes, cold + cached reuse | 2,086 MiB | 610.2 MiB |
| Allocation count, cold + cached reuse | 3.55 M | 3.517 M |
| First analysis | 1.303 s | 1.189 s |
| Producer first analysis | 670.6 ms | 488.9 ms |
| Cached analysis median | 1.061 ms | 1.105 ms |
| Incremental analysis median | 79.9 ms | 83.45 ms |
| Producer incremental median | 12.5 ms | 14.96 ms |
| Cold response bytes, combined | 5,310,478 B | 6,434,977 B |
| Incremental response | not recorded | 7,061 B |

The last directly measured v6 physical peak was 477.7 MiB (49.5% below the
original baseline); exact post-GC `HeapAlloc` fell 43.8%, and cold allocated
bytes fell about 71%. The cold result now spans one path transition and two
packed oracle frames, increasing combined response bytes by 21.2%. The cached
median varied between 1.04 and 1.13 ms across repeated 30-sample runs.
Incremental end-to-end remains within 5% of the original baseline and within
1% of optimized v5. Physical figures are the last pre-oracle v6 medians because
this sandbox denies `ps`/`vmmap` process inspection without an approval prompt.

## Cold-query performance follow-up

CPU profiling on the same corpus found that runtime-symbol identity repeatedly
called `filepath.EvalSymlinks` for declarations in the same file. The project
now resolves each declaration path once per checker generation and drops that
cache with the checker. Semantic extraction also resolves each result AST node
once, shares it between the structural and semantic phases, and reuses adjacent
query-position lookups. Checker calls remain in their original deterministic
order, preserving generation-scoped symbol-handle minting.

Three alternating cold runs against the same Rust checker measured:

| Metric | Before | After | Change |
|---|---:|---:|---:|
| First analysis, median | 1,162.0 ms | 1,111.6 ms | -4.3% |
| Producer demand stage, median | 376.0 ms | 314.3 ms | -16.4% |
| Cached analysis | 1.07-1.13 ms | 1.04-1.13 ms | noise |
| Incremental analysis, 30-sample median | 79.0-81.7 ms | 79.0-79.4 ms | no regression |
| Incremental Producer median | 14.0 ms | 14.1 ms | noise |
| Initial response | 3,575,177 B | 3,575,177 B | unchanged |
| Incremental response | 7,166 B | 7,166 B | unchanged |

The reference scanner now verifies source-order appends and sorts only a symbol
bucket that actually arrives out of order. This reduced the isolated demand
stage by about 3% in alternating samples, but did not move end-to-end time
outside run noise.

Two larger experiments were rejected:

- Parallel checker calls would require multiple TypeScript checkers for one
  Program. That conflicts with the project mutex and file-affine single-checker
  lease described in ADR 0004, duplicates checker caches, and makes
  generation-scoped symbol-handle order nondeterministic. Pure AST lookup was
  separated and fused instead.
- Loading references during structural symbol-closure rounds increased cold
  first analysis from a 1,089 ms median to 1,292 ms (+18.6%). The packed
  references-only frame is materially cheaper than embedding large reference
  vectors in 50,000 structural rows, so the dedicated final reference round
  remains.

A fully demand-driven reference index was also rejected for this workload:
the cold reference tier contains roughly 35,000 of 50,000 closed symbols,
reference-space classification is project-wide, and exact incremental
invalidation depends on retained per-file symbol contributions. It would still
scan nearly the whole AST while either adding a second scan or weakening exact
changed-reference evidence.

## Whole-pipeline performance follow-up

The final TypeFacts pass removed two more avoidable ownership/query costs:

- Structural and semantic demands now share the already-selected AST node and,
  when JSX normalization leaves it unchanged, the checker symbol. Type
  descriptor and callability demands also share one `GetTypeAtLocation` result.
  Against a fresh 5,000-file control, the Producer demand stage improved about
  3.6% (311.4 ms to 300.1 ms median). End-to-end cold analysis improved about
  0.5%; cached analysis was unchanged and the 30-sample incremental median
  improved from 78.59 ms to 76.66 ms.
- Rust source-arena decoding now reads directly into each final `SourceFile`
  buffer. The former corpus-sized `std::fs::read` allocation and per-file
  `to_vec` copies no longer coexist. Stable source-setup samples improved about
  3.1% (117.49 ms to 113.81 ms) while preserving the exact payload.

Profiling the consuming Solid Checker exposed independent per-file work inside
Reactive IR. The repository includes an apply-ready patch at
[`solid-checker-reactive-ir-performance.patch`](solid-checker-reactive-ir-performance.patch).
It keeps cache mutation and merging sequential and ordered, while parallelizing
summary-node discovery, graph-contribution discovery, cold result reads,
contract-node construction, and contract-fragment construction. On the same
corpus:

| Metric | Before patch | With patch | Change |
|---|---:|---:|---:|
| First analysis, median | 1,066.1 ms | 1,014.4 ms | -4.8% |
| Reactive IR, median | 408.8 ms | 337.9 ms | -17.3% |
| Interprocedural graph | ~54 ms | ~21 ms | -61% |
| Interprocedural export summaries | ~47 ms | ~26 ms | -46% |
| Incremental analysis, median | 79.5-80.2 ms | 78.7-79.1 ms | no regression |
| Incremental response | 7,167 B | 7,167 B | unchanged |

The checker patch was validated in a writable mirror of the current dirty
Solid Checker checkout because that checkout was mounted read-only for this
task. Apply it from the Solid Checker root with
`git apply /path/to/solid-ts-facts/docs/solid-checker-reactive-ir-performance.patch`.

Rejected experiments were retained in the attribution record, not in
production:

- Combining async-flow and semantic-demand extraction behind one shared
  cross-phase hash-map module increased Producer analysis about 3.8%.
- Server-side transitive structural closure increased first analysis about
  2.3%; compact request/response rounds remained faster.
- Parallel local-access discovery halved that isolated stage but contended with
  the concurrently running interprocedural worker, so total Reactive IR did not
  improve.
- Generation-scoped canonical-symbol memoization added hash lookups and
  regressed the demand-stage median about 1.5%.

## Packed retained client rows

Whole-process profiling after the v6 ownership work found that the remaining
large Rust allocation was not the packed transition itself. It was the
materialized `EntityFact` row: inline optional `TypeDescriptor` and
`ResolvedCall` values made every retained entity pay for the largest nested
evidence shape, including rows where both values were absent.

The retained client now keeps those two uncommon, large values behind `Arc`.
This preserves the serialized shape and the opaque `FactTable` interface while
making the common row at most 96 bytes. `DemandGroup::shared` also lets a
grouped caller transfer its immutable `Arc<[EntityDemand]>` allocation into the
session by reference count instead of retaining a second owned run.

Measured through Solid Checker's 5,000-file retained actor with balanced IR
retention:

| Metric | Before | Packed/shared | Change |
|---|---:|---:|---:|
| Checker physical footprint | 328.8 MiB | 294.0 MiB | -34.8 MiB (-10.6%) |
| Process-tree physical footprint | 584.2 MiB | 554.9 MiB | -29.3 MiB (-5.0%) |
| Incremental certification median | 23.1 ms | 21.8 ms | no regression |
| Type Facts bytes per source | 695 B | 695 B | unchanged |

The producer varies by several MiB between runs; the checker-local reduction is
the attributable result. The wire schema and response bytes are unchanged.
