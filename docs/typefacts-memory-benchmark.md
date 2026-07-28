# TypeFacts memory benchmark

This benchmark separates cold materialization, retained state, cached reuse,
and incremental updates. RSS is not used as a memory target on macOS: `vmmap`
physical footprint and peak account for reclaimable Go pages, while a forced-GC
heap profile or `runtime.MemStats.HeapAlloc` measures live Go objects.

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
canonical entity and symbol rows required to construct v5 deltas. Making Go a
fully stateless semantic oracle requires a new multi-round protocol: v5 can ask
questions only by source location and cannot request alias/declaration/reference
facts by symbol ID. Such a protocol should preserve v5 as the compatibility
adapter rather than forcing full recomputation or sending Rust's retained table
back to Go.

## Schema v6 ownership transfer

V6 makes Rust the semantic-closure owner. Go releases expanded source, entity,
file, symbol, and reference rows after transfer; it retains the TSGo program
and compact per-file extraction proof. Rust derives roots, runs alias closure,
owns reference-tier membership, and patches path-scoped reference evidence.
Symbol oracle responses reuse the packed transition dictionary codec rather
than materializing a second large CBOR object graph. See
[ADR 0009](adr/0009-v6-client-owns-expanded-path-rows.md).

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
