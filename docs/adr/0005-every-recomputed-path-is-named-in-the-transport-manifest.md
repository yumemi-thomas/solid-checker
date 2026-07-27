---
status: accepted
---

# Every recomputed path is named in the transport manifest

The producer's delta is built from a manifest of changed paths, so any file whose
rows were recomputed must appear in that manifest or the delta silently omits its
changes and the client's retained table diverges. There are more ways to be
recomputed than being edited, and each one was found the hard way, so the rule is
now stated once and applied to all of them: **every recomputed group contributes
its path to the manifest, whatever caused the recompute.**

The causes known today are an edited or affected file, a changed demand run, a
descriptor refresh under a shifted structural-accessor union, and a file holding
non-durable identities — which are re-minted every generation and therefore
recompute forever.

## Why this replaced a cost cliff

Non-durable files used to be handled by giving up: a single one set
`NonDurableFiles != 0` and the producer packed the entire table instead of
emitting a delta. That was a correctness guard for the manifest gap, not a
statement about deltas, and it was expensive out of all proportion to its cause.
A property access on a mapped type is enough to make a file non-durable, and
mapped types are ordinary TypeScript — so an ordinary project paid a whole-table
pack on every keystroke.

Measured on a 1,001-module project containing exactly one mapped-type file:

| per edit           | forcing a full table | emitting a delta |
| ------------------ | -------------------- | ---------------- |
| analyze round trip | 9.20 ms              | 4.65 ms          |
| response bytes     | 423,317              | 1,238            |

## Consequences

- **A genuine cold analysis still packs the whole table**, and still costs about
  5 ms on a project this size. That is inherent: the client has nothing to apply
  a delta to. Only the per-edit repetition of that cost is gone.
- **The manifest has a path limit.** Past it, the producer falls back to diffing
  the tables in full — which still yields a delta, just computed without the
  manifest's help. A project with very many non-durable files therefore degrades
  to a slower diff rather than back to a whole-table pack.
- **Naming a path is now the obligation of whatever decided to recompute it.**
  Any future reason to recompute a group must contribute its path, and the
  delta-applied-versus-fresh oracles are what catch a new omission: they compare
  what a client would hold against a fresh materialization, which a
  producer-side-only comparison cannot do.
