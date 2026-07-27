---
status: accepted
---

# A coordinated schema bump removes the request fields nothing honoured

The v3 request carried five fields no producer path ever read: the demand flags
`type`, `resolveAlias` and `declarations`, and the `structuralSpans` and
`compilerSpans` location lists. All five are now gone from the schema, from both
language bindings, and from the compact demand encoding, and the schema digest
pinned in the startup handshake moved with them.

Removing them is a breaking protocol change, but not an expensive one: the
handshake already rejects on protocol version, schema digest **and build id**, so
a producer and a client have always had to ship together. A mismatched pair
cannot limp along half-working — it refuses to talk at all, which is a far better
failure than the silent no-op these fields produced. On the Rust side the two
span fields were public on `AnalysisDemand`, so their removal is a compile error
at upgrade; the crate takes a major version bump to say so.

## Why they were unhonoured

- `resolveAlias` and `declarations` were never capabilities a caller could
  request. Alias targets and declarations arrive unconditionally through symbol
  closure, so the flags asked for something already guaranteed.
- `type` selected an opaque per-generation type identity that had no consumer,
  was absent from the wire fact table, and — because the identity embedded the
  generation number — made every entity row holding a resolved call compare as
  changed on every generation, inflating each delta.
- `structuralSpans` and `compilerSpans` fed the byte-scan seeding design that
  v3's explicit demand list replaced. The client sends precise locations now, so
  handing the producer spans to re-scan would undo that.

## Consequences

- The per-file demand hash digests six flag bytes instead of nine.
- `EntityDemand` in both languages now carries only flags the producer honours,
  so the request shape no longer advertises capabilities that do not exist.
- The compact demand encoding keeps bits 0–5 and drops 6–8. Bit positions of the
  surviving flags are unchanged.
