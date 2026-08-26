---
status: accepted
---

# One packed transition carries Wire table changes

Lifecycle schema v5 replaces the separate `tableMode`, `packedTable`, and
`packedDelta` response fields with one optional **Wire table transition**. Its
frame identifies full versus delta mode, the project and table schema, the base
and target generations, and the base State token; its canonical path and symbol
operations encode directly from Retained contributions. An absent transition
means reuse, and the successor State token remains on the response so that
reuse stays allocation- and frame-free.

The previous full-v4 and delta-v2 frames duplicated row order, flags,
dictionary rules, writers, readers, and temporary wire models, yet a delta
still carried no proof of which retained table it transformed. Reusing their
scratch would reduce allocation but preserve those two shallow implementations.
A generated section-directory format would make future layouts more mechanical,
but adds generator and compatibility machinery before a second format exists.
The unified transition keeps one concrete Go encoder and one strict Rust
decoder, uses compact numeric tags for closed wire enums, and treats
cross-language goldens and malformed-frame fixtures as the executable contract.

No compatibility reader is retained. The startup handshake already pins the
protocol version, schema digest, and build ID, so mismatched Producer and
consumer builds refuse to communicate and have always shipped in lockstep
(ADR-0003). Rust validates the transition's base identity and the complete
frame before preparing a candidate table, then publishes that table and the
successor State token together.

## Consequences

- Full and delta packing share canonical chunk traversal, dictionary storage,
  row writers, and session-owned scratch; reuse performs no codec work.
- An exact Transport manifest limits delta work to named paths and symbols.
  It is authenticated by a private retained-table state ID as well as source
  generation; a mismatch falls back to a full canonical diff, preserving
  ADR-0005.
- A materialized analysis that fails or is cancelled before Session publication
  discards every demand-derived Closure cache. The rare retry rebuilds from the
  last accepted demand snapshot instead of exposing rejected contributions.
- Rust decodes and preflights every fallible table-relative operation before
  taking the retained table. Unique sections then mutate in place; snapshots
  still held by callers retain copy-on-write isolation.
- The Go and Rust bindings remove the legacy response fields in one coordinated
  release, and the Rust crate takes the corresponding breaking-version bump.
- Applying delta rows directly while decoding remains a separate measured
  opportunity; the v5 decoder first preserves transactional validation.
