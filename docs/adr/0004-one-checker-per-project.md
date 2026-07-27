---
status: accepted
---

# One checker per project

The tsgo adapter installs a custom checker pool that holds exactly one
`checker.Checker` for the lifetime of a project, instead of the compiler's
default pool of four. The default builds its checkers afresh on **every** program
update, so on the editor path — where a generation is one keystroke-scale edit —
it charges four checker constructions per edit for concurrency the adapter cannot
use anyway: every entry point is already serialized behind the project mutex, and
requests are dispatched through a single ordered worker.

## Why a custom pool rather than `SingleThreaded`

The compiler already has a switch that yields one checker: `SingleThreaded` makes
`checkerCount` 1. It is the wrong switch, because it also serializes parsing and
binding, which are the phases that genuinely do scale. The custom pool is what
lets the adapter keep parallel parse and bind while holding a single checker —
the combination neither built-in setting offers.

## What this costs

Type checking is the serial phase. Everything checker-derived — scoped semantic
entities, async control flow, reference batches, declarations, alias resolution —
runs through the one checker under one lock. Measured on the 1,000-symbol
generated corpus, analysis scales about 1.4x from one core and is done improving
by two to four cores of fourteen:

| GOMAXPROCS | warm edit | cold full table |
| ---------- | --------- | --------------- |
| 1          | 2.34 ms   | 4.14 ms         |
| 2          | 1.84 ms   | 3.34 ms         |
| 4          | 1.77 ms   | 2.89 ms         |
| 14         | 1.72 ms   | 2.79 ms         |

The residual scaling comes from parallel parse and bind, from the closure's own
chunked symbol hydration, and from concurrent garbage collection over the
multiple megabytes an analysis allocates. Those three were not separated, so no
single one of them should be credited.

## Consequences

- **A profile showing one busy core during analysis is the design, not a bug.**
  Anyone removing the custom pool to "use the machine" will reintroduce four
  checker constructions per edit on the path that can least afford them.
- **The file-affine lease is deliberately non-exclusive.** `GetChecker` with a
  non-nil file returns the checker with a no-op release, because a targeted emit's
  declaration resolver takes the checker's own mutex and returning the lifetime
  lease would deadlock on that reentrant lock. This is only safe *because* the
  project mutex already serializes every adapter entry point — so the pool and the
  mutex are one decision, and neither can be relaxed alone.
- **Concurrency is not where the producer's remaining cost is.** The producer
  accounts for roughly 2.3 ms of a ~15 ms client-side edit, already near its
  parallel ceiling. The wins available on that path have been transport shape and
  allocation, not more cores.
- **Reversing this is a real project, not a flag flip.** It means restoring
  per-update checker construction, giving the adapter a concurrency story above
  the project mutex, and replacing the ordered worker with something that can run
  analyses in parallel without reordering generation-scoped requests. Measure the
  four-checker allocation cost on the editor path first; that is the number this
  decision was made on.
