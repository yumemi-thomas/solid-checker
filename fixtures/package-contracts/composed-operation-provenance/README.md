# A read performed through a call names the export it was composed from

Four exports, one accessor read each in the emitted contract, and four
different answers to "where does this read happen".

- `readsItsOwnSignal` reads `count()` in its own body. Its `read-0` carries
  **no** `composedFrom`: there is nothing to compose, and the export's own
  implementation census is the only evidence a consumer may demand for it.
- `readsThroughADirectCall` performs the same read only by calling
  `readsItsOwnSignal`. Its `read-0` carries
  `composedFrom: {"export": "readsItsOwnSignal", "operation": "read-0"}` — the
  exact `(export, operation)` the row was composed from, inside this artifact
  case. That is the positive case: the call is direct, reachable and
  uncaptured, so the row's `at: call / schedule: same-stack` stamp survives
  the hop.
- `readsThroughAReturnedClosure` calls `readsItsOwnSignal` only from inside the
  closure it returns. It has **no read operation at all** — the read never
  reaches the export's summary — so there is no row to carry provenance and
  nothing to compose. A regression that carried the read here would publish
  "this export reads an accessor when you call it" about an export that reads
  nothing until the caller invokes the closure it was handed.
- `readsThroughAPrivateHelper` calls the unexported `privateReader`. The read
  reaches its summary, but the node it was discovered in is not an export of
  this package, so the aggregation can name no target and publishes **no**
  provenance. The row therefore stays exactly as unprovable as it was before
  composition existed — which is why this export shares
  `readsItsOwnSignal`'s summary object byte for byte. "Some node in this
  package performs the read" is not a claim this model can make.

Two more exports pin the narrowings the aggregation applies before it
publishes anything:

- `readsUnderTwoNames` is exported *twice*, as itself and as
  `alsoReadsUnderTwoNames`. `composesAnAmbiguouslyNamedTarget` calls it
  directly, and its `read-0` carries **no** provenance: two names are two
  claims, and picking one would name a target a consumer's call site may not
  resolve to.
- `readsItsOwnSignalAndComposesTheSameShape` reads its own accessor *and*
  calls `readsItsOwnSignal`. The two reads have the same `(kind, label)`, so
  they would have collapsed onto one row before the dedup key carried the
  provenance; instead the export publishes **two** rows — `read-0` with no
  provenance, `read-1` composed from `readsItsOwnSignal:read-0`. That is the
  point of widening the key: collapsing them would publish a single claim that
  this export's own census has to witness *and* a composed claim it cannot,
  and the stronger of the two demands would silently disappear. The cost is
  row inflation, recorded in `docs/precision-backlog.md`.

Provenance is a *nomination*, never authority. A consumer discharging
`readsThroughADirectCall:read-0` must additionally prove that this export
really calls `readsItsOwnSignal` — by the callee's resolved declaration
identity, never by its name — and that `readsItsOwnSignal:read-0` is itself
discharged from its own census. Both premises live in the certifier and are
pinned by unit tests beside `require_composed_operation`; this fixture pins
only what the generator publishes.

The `node_modules/solid-js` stub is 1.x, so `createSignal` resolves through the
v1 catalog.
