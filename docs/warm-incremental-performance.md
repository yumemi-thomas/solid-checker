# Warm incremental performance report

Date: 2026-07-28
Machine: Apple M4 Pro, darwin/arm64, `GOMAXPROCS=14`

## Acceptance baseline

The corpus contains 49 source modules, 1,000+ demanded symbols, retained
references, resolved calls, async facts, and structural-accessor suppression.
Each sample executes 50 alternating updates and analyses; medians below use ten
independent samples. Response encoding is outside the timed loop but reported
as bytes per operation.

| end-to-end path | `HEAD` median | final median | speedup | final allocation | final response |
| --- | ---: | ---: | ---: | ---: | ---: |
| warm leaf edit → analyze | 1.885 ms | 0.938 ms | **2.01x** | 708 KB / 3,201 | 607 B |
| root-changing edit → analyze | 1.805 ms | 0.952 ms | **1.90x** | 717 KB / 3,203 | 3,345 B |
| shape-changing edit → analyze | 33.261 ms | 15.574 ms | **2.14x** | 8.88 MB / 60,606 | 123,695 B |
| cold/reset analyze | 6.210 ms | 4.855 ms | **1.28x** | 2.54 MB / 3,556 | 177,051 B |

The `HEAD` warm path allocated about 1.05 MB in 6,882 allocations. The final
path reduces bytes by roughly 33% and allocation count by roughly 53%.

## Retained candidates

### Retained Semantic demand runs and contributions

One immutable contribution owns each file's entities, descriptor evidence,
reachable roots, Full-tier roots, structural accessors, dependency paths, and
durability proof. Reverse dependency and descriptor-user indexes invalidate
only exact users. A Session-owned Retained demand set applies sparse run edits
with an undo log and commits only with its Fact table.

Evidence: all retained tables byte-match a fresh full Wire table transition.
The combined end-to-end result is included in the 2.01x primary result.

### Leaf declaration-proof elision

When the retained reverse import graph proves a changed external module has no
importers and no global/module augmentation, the tsgo adapter refreshes its
export identities without emitting a declaration proof.

| leaf path | median | allocation |
| --- | ---: | ---: |
| cutoff disabled | 1.10 ms | 913 KB / 5,743 |
| cutoff enabled | 0.913 ms | 708 KB / 3,201 |

Retained: about 17% end-to-end, with explicit tests for gaining an importer and
for augmentations.

### Stable symbol closure and direct canonical-store patching

Equal raw roots, equal raw Full-tier roots, complete prior symbol memo,
unchanged invalidated alias edges, and an exact reference delta prove an equal
alias universe. The implementation patches changed rows into copy-on-write
canonical chunks.

| warm leaf path | median | allocation |
| --- | ---: | ---: |
| Stable symbol closure disabled | 1.126 ms | 745 KB / 3,235 |
| Stable symbol closure enabled | 0.913 ms | 708 KB / 3,201 |

Retained: about 19% end-to-end.

### Unified Wire table transition

| codec path | previous | unified v5 | change |
| --- | ---: | ---: | ---: |
| exact-manifest delta | 13.8 µs / 1,648 B / 5 allocs | 9.8 µs / 305 B / 1 alloc | 29% faster |
| cold full frame | 1.19 ms / 2.13 MB / 76 allocs | 0.95 ms / 189 KB / 1 alloc | 20% faster |

Retained: one encoder and one Rust retained-table decoder replace duplicate
full/delta implementations while authenticating the transition base.

### Declaration-derived export identity index

Adversarial deletion/restoration found that pointer-only export lookup made
symbol IDs depend on whether an intermediate Generation had been analyzed.
The retained fix indexes export identity by durable declaration reference as
well as current checker symbol pointer.

Retained for correctness. The full warm benchmark shows no measurable
regression after the fix.

## Rejected or deferred candidates

### Cross-Program checker reuse — deferred upstream

Instrumentation inside the pinned TypeScript-Go fork separates checker
construction from Program update:

| checker work on warm leaf edit | median | share of paired 0.881 ms baseline |
| --- | ---: | ---: |
| bind | < 1 µs | < 1% |
| checker setup | 18–20 µs | ~2% |
| global merge and `initializeChecker` | 190–200 µs | ~22% |

The useful target is therefore global initialization, not binding or ordinary
checker allocation. A temporary fork rebased the exclusively owned checker
only when the adapter proved one changed external-module leaf, no importers,
and no global/module augmentation.

| paired 500-edit benchmark, ten samples | median | allocation |
| --- | ---: | ---: |
| fresh checker each Generation | 0.881 ms | 703–715 KB / 3,206 |
| experimental in-place rebase | 0.694 ms | 377 KB / 1,902 |

The prototype demonstrates a repeatable **1.27x** end-to-end opportunity and
removes about 47% of bytes and 41% of allocations. Three repeated cold
fresh-checker oracle runs passed the augmentation, inferred-export,
deletion/restoration, config, and alternating-Generation matrix.

It is not retained. TypeScript-Go currently fuses the Program pointer, global
symbol merge, built-in types, node/symbol links, diagnostics, flow state, and
type/signature/relation caches inside one mutable `Checker`. The experimental
rebase mutates that published object before candidate analysis, so
cancellation or failure cannot restore maps mutated through shared caches.
Passing output tests does not prove transactional rollback.

Three interfaces were evaluated:

1. `BuildPrelude(program) -> immutable CheckerPrelude`, followed by
   `NewGeneration(program, prelude) -> CheckerGeneration`. This gives the
   strongest ownership proof and is the selected upstream design.
2. `checker.ForkGeneration(program, proof) -> candidate Checker`, backed by
   copy-on-write cache epochs. This is a smaller public seam but still requires
   classifying every checker cache as prelude-safe or Generation-local.
3. `checker.Rebase(program)` with an undo journal. This produced the measured
   result but is rejected: arbitrary mutations through map-valued caches make
   complete rollback unverifiable.

Until TypeScript-Go exposes the first or equivalently safe second seam, the
fresh checker remains the oracle and production fallback.

### General cycle-safe Symbol affected cone — deferred

Root-changing median latency is 0.952 ms, only about 15 µs above the 0.938 ms
stable leaf path. Even erasing that difference contributes under 2% to the
primary gate. The required retained multiplicities, alias reverse index, and
old/new cycle clearing remain a valid design, but the measured leverage does
not justify its proof surface now.

### Complete canonical Fact table patching — deferred

Canonical root/entity assembly costs about 73 µs, under 9% of the warm path;
making it free has a ceiling near 1.09x. The current immutable symbol chunks
already patch the expensive stable symbol universe. A full Sources/Entities/
Files patcher is deferred.

### Further transport work — rejected for this target

The retained transition costs about 12 µs, around 1% of the warm path. Even a
free transport encoder would not materially move the end-to-end gate.

### Public candidate-lane registry — rejected

Three interface designs were compared. The chosen design keeps `Advance` and
`Materialize` lifecycle operations and hides proof lanes, oracle fallback, and
profiling inside one deep implementation. A public tagged transaction and lane
registry adds caller knowledge without performance leverage.

## Correctness evidence

The complete Go suite and race detector pass, as do all Rust retained-table,
golden-codec, process-session, crash-replay, cancellation, and public-interface
tests. New fresh-oracle scenarios cover:

- module and global augmentation;
- inferred export alternation;
- alias retargeting and alias cycles;
- final external-root removal;
- Full-tier reference addition/removal;
- file deletion and restoration;
- config alternation;
- six to eight repeated alternating Generations.

Cancellation and injected analysis failures retain their existing rollback
tests. No candidate state publishes before semantic materialization, canonical
assembly, transition encoding, and the final cancellation check succeed.
