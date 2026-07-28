---
status: accepted
---

# Stable symbol closure patches rows

The Demand closure retains three handle-indexed membership snapshots from the
preceding successful generation:

- raw reachable roots before alias expansion;
- raw Full-tier roots before alias expansion;
- the expanded Full tier after alias expansion.

An accepted update also records every previously reached durable symbol fact
evicted by its affected paths. On the next materialization, the Producer may
reuse the preceding Symbol closure only when all of the following are proven:

1. raw reachable-root membership is identical;
2. raw Full-tier-root membership is identical;
3. the retained symbol memo covered the complete preceding canonical universe;
4. every invalidated reached symbol resolves to the same alias target;
5. the compiler adapter reports an exact reference delta.

Equal roots and equal alias edges prove an equal reachable fixed point. Equal
Full-tier roots and those same edges prove an equal Full-tier fixed point.
Declarations and exact reference rows may still differ, so the Producer patches
those rows into copy-on-write canonical symbol chunks. Any failed predicate uses
the existing full closure implementation.

## Why not reference counts

Reachability counts are not sufficient in an alias cycle. After the final
external root of `A -> B -> A` disappears, each node can still appear supported
by the other. A general incremental closure would need a cycle-safe affected
region that clears and reseeds under the union of old and new edges. Profiles
after the stable-closure path leave less than the required headroom for that
additional state and complexity.

## Transaction boundary

Seed snapshots and the expanded Full tier publish only after semantic
resolution, symbol-row patching, Fact table assembly, and Transport manifest
construction all succeed. Cancellation or failure returns their candidate
storage to scratch and leaves the preceding proof state unchanged.

Interner replacement clears every snapshot because handles have meaning only
inside one interner lifetime. Non-durable or declaration-less reached symbols
make the memo incomplete and therefore force the full closure path.

## Measured consequence

On the generated editor corpus, an isolated ablation after the other retained
work raises median update-plus-analyze latency from roughly 0.913 ms to
1.126 ms. The specialization therefore earns about 19% end to end and reduces
steady-state allocation from roughly 745 KB / 3,235 allocations to
708 KB / 3,201. The retained-vs-fresh wire oracle remains the correctness gate
and explicitly requires that at least one generation exercise this path.

The general affected-cone design remains deferred by
[ADR-0008](0008-proof-gated-generation-retention-stays-specialized.md): the
root-changing end-to-end path is only about 15 microseconds slower than the
stable path on this corpus, which is insufficient leverage for a second alias
closure proof.
