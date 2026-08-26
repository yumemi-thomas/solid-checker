---
status: accepted
---

# Durable symbol identity gates retained state

Retaining a demand closure across generations requires symbol identities that
stay meaningful after the generation that minted them, so a symbol identity is
**durable** only when it is derived from its declaration — the declaration's
path, name span, and symbol name — and every other identity is scoped to the
single generation that issued it. A retained contribution is stored only when
every identity it carries is durable; exported symbols additionally get a
byte-independent spelling, which is what lets retention survive an edit that
merely shifts a file's bytes.

> Recovered, not original. Three comments in `closure.go` and
> `semantic_retained.go` cited `ADR 0001` for this rule, but no ADR existed in
> this repository's history. This records the rule **as implemented** so those
> citations resolve. The reasoning behind the original choice is not recoverable
> and is not claimed here.

## Consequences

- A file holding any generation-scoped identity cannot be retained. Such files
  recompute every generation — all of them together and in canonical order, so
  the identities they mint match a fresh whole-batch run.
- A non-retainable file recomputes, but it does **not** force the whole table
  onto the wire. An earlier revision of this ADR claimed a delta could not
  describe rows whose identities were re-minted; that was wrong. A delta removes
  the old identities and adds the new ones like any other change, provided the
  recomputed paths are named in the transport manifest — which they now are. See
  [ADR-0005](0005-every-recomputed-path-is-named-in-the-transport-manifest.md).
- Declaration-less synthetic symbols are excluded from cross-generation reuse
  even when their identity spelling looks durable: an accepted update has no
  source path by which to evict them.
