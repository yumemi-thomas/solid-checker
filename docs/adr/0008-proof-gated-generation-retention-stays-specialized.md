---
status: accepted
---

# Proof-gated Generation retention stays specialized

The warm edit path keeps two proof-gated specializations: Retained
contributions for unchanged Semantic demand runs and Stable symbol closure
patching for an unchanged alias universe. It does not add a general
incremental alias affected-cone or retain a checker across compiler Programs.
The current fresh paths remain the oracle and fallback whenever a proof is
missing.

This is a measured limit, not a claim that either deferred design is
impossible. On the 1,000-symbol generated corpus, the retained implementation
has these median end-to-end costs over ten 50-iteration samples:

| path | `HEAD` | retained implementation | change |
| --- | ---: | ---: | ---: |
| warm leaf edit → analyze | 1.885 ms | 0.938 ms | 2.01x faster |
| root-changing edit → analyze | 1.805 ms | 0.952 ms | 1.90x faster |
| shape-changing edit → analyze | 33.261 ms | 15.574 ms | 2.14x faster |
| cold/reset analyze | 6.210 ms | 4.855 ms | 1.28x faster |

The warm leaf path allocates about 708 KB in 3,201 allocations, down from
about 1.05 MB in 6,882 allocations. Its response is about 607 bytes rather
than 508 bytes because the v5 Wire table transition authenticates its base and
target identity inside the frame; that fixed proof cost is retained because
transport accounts for only about 12 microseconds of the 0.938 millisecond
path and the transition must remain self-identifying.

## Checker Generation retention is deferred

Trace measurements split the current warm leaf median approximately as
follows:

| stage | time | share | impossible-best ceiling |
| --- | ---: | ---: | ---: |
| Program/checker Generation | 0.29 ms | 34% | 1.52x |
| Semantic demand work | 0.165 ms | 19% | 1.24x |
| canonical root/entity assembly | 0.073 ms | 9% | 1.09x |
| Stable symbol closure patch | 0.077 ms | 9% | 1.10x |
| Wire table transition | 0.012 ms | 1% | 1.01x |

Allocation profiles attribute about 274 KB per warm edit to
`checker.NewChecker`, roughly 38% of retained-path allocation. That makes an
incremental checker Generation a meaningful upstream project, but even making
Program/checker construction free cannot double the current end-to-end path.

Instrumentation in the pinned TypeScript-Go fork refined that broad ceiling:
binding costs less than one microsecond, checker setup costs about 18–20
microseconds, and `initializeChecker` costs 190–200 microseconds, about 22% of
a paired 0.881 millisecond warm baseline. A proof-gated in-place rebase reduced
the paired median to 0.694 milliseconds (**1.27x**) and allocation from roughly
703–715 KB / 3,206 allocations to 377 KB / 1,902. Its results matched a cold
fresh-checker oracle across three repeated adversarial runs.

The fork is still rejected for production. TypeScript-Go's checker fuses the
Program pointer, global symbol merge, global and module augmentations,
built-in types, node/symbol links, inferred exports, diagnostics, flow state,
and type/signature/relation caches inside one mutable object.
`Program.UpdateProgram` creates a new checker pool; there is no transactional
fork operation. The measured prototype mutates the published checker before
candidate analysis, and an undo journal cannot reliably reverse writes made
through shared map-valued caches.

Two safe interfaces were designed before choosing one:

- Selected: build an immutable `CheckerPrelude` containing only compiler
  options, library/global symbols, intrinsic/global types, and other
  dependency-proven global state; create a separate `CheckerGeneration` for
  Program AST/symbol links, augmentations, inferred exports, diagnostics,
  flow, and relation caches. Candidate Generation state is published only
  after analysis succeeds.
- Alternative: `ForkGeneration(program, proof)` creates a checker whose
  Generation-local maps use copy-on-write epochs. This minimizes API surface
  but requires the same exhaustive cache ownership classification and makes
  accidental cross-Generation writes less visible.

The measured in-place `Rebase(program)` interface is rejected despite its
speed because it cannot satisfy cancellation rollback. Until the selected
upstream seam exists and passes the fresh oracle for globals, augmentations,
aliases, inferred exports, deletion, and config changes, ADR-0004 remains in
force.

## General incremental Symbol closure is deferred

An ablation of Stable symbol closure raises the warm leaf median from about
0.913 ms to 1.126 ms, so the retained specialization earns about 19%.
Root-changing analysis is only about 15 microseconds slower than the stable
leaf path after the existing memoized full closure. A retained root/Full-tier
multiplicity table, alias graph, reverse alias index, and cycle-safe old/new
affected cone therefore has at most a low-single-digit contribution to the
primary gate, while adding a new correctness proof for alias cycles and final
root removal.

The design remains valid if a future corpus shows a larger gap: clear the
union of the old and new alias cone, reseed it from surviving external roots
and incoming edges, recompute both reachable and Full-tier fixed points, and
patch the canonical symbol store transactionally. Counts alone are forbidden
because a rootless alias cycle can falsely support itself.

## Transaction and oracle

The chosen module interface is conceptually two operations matching observable
lifecycle semantics: advance a Generation, then materialize an Analysis
transaction. Proof lanes and profiling stay private implementation details.
An accepted source update remains accepted even when a later analysis fails;
the analysis publishes its Retained demand set, Demand closure, Fact table,
Transport manifest, Wire table transition, and successor State token together.

The retained-versus-fresh full-transition oracle covers stable edits and now
also covers module/global augmentation, inferred exports, alias retargeting,
alias cycles, final-root removal, Full-tier reference changes, deletion,
config changes, and repeated alternating Generations. Those tests exposed and
fixed one history-dependent identity defect: checker-equivalent exported
symbols were keyed only by pointer after a rebuild. Export identities now have
a declaration-derived secondary index, so analyzing an intermediate
Generation cannot change later deterministic symbol IDs.

## Consequences

- The 2x primary target is accepted only on median end-to-end warm
  edit → analyze latency, not on an isolated stage.
- Stable symbol patching, leaf declaration-proof elision, Retained
  contributions, and the unified Wire table transition stay because their
  ablations or paired benchmarks show repeatable wins.
- An immutable-prelude checker fork, general alias affected cone, and complete
  canonical-table patcher remain deferred behind quantified ceilings. The
  checker fork has a measured 1.27x end-to-end opportunity, but the unsafe
  in-place prototype is rejected.
- Any future retained path must use the fresh implementation as fallback and
  must pass exact retained-versus-fresh and transition-applied oracles before
  publication.
