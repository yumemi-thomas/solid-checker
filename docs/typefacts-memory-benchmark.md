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

## 5,000-file reference result

The baseline is commit `f38e219`; values are from the same corpus and machine.
Allocation counts and live heap are post-analysis Go measurements. Physical
peak is the Producer's `vmmap` peak, not the complete checker process tree.

| Metric | Baseline | Optimized |
|---|---:|---:|
| Producer physical peak | 945.9 MiB | 521.7 MiB |
| Producer steady physical | 412 MiB | 320.6 MiB |
| Go live heap | ~345 MiB | 241.2 MiB |
| Allocated bytes, cold | 2,086 MiB | 597 MiB |
| Allocation count, cold | 3.55 M | 3.48 M |
| First analysis | 1.303 s | 1.204 s |
| Producer first analysis | 670.6 ms | 559.0 ms |
| Cached analysis median | 1.061 ms | 1.128 ms |
| Incremental analysis median | 79.9 ms | 77.8 ms |
| Producer incremental median | 12.5 ms | 10.7 ms |
| Semantic payload | 5,247,633 B | 5,247,633 B |
| Outer response | 5,310,478 B | 5,310,478 B |

Cold peak fell 44.8%, exact post-GC `HeapAlloc` fell 30.1%, and cold allocated
bytes fell about 71%. Payload sizes are byte-for-byte unchanged. The cached
median varied between 1.04 and 1.13 ms across repeated 30-sample runs; the
incremental end-to-end and Producer medians both improved in the final run.
