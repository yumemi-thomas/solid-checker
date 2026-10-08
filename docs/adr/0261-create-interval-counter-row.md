# ADR 0261: an authored row for createIntervalCounter

- Status: accepted and implemented (2026-10-09).
- Owners: the `@solid-primitives/timer@1.4.5-next.1` spec and its shared pairs.

`createIntervalCounter` had no export row, so the admitted object mapped it to
an open callable and every import was `package-contract-incomplete`. On every
browser normal completion it delegates to the package's own
`createPolled(prev => prev + 1, timeout, -1, options)` (dist `index.js:138-141`).
Its poll function is package-internal and numeric, so the async-iterable
result blocker that keeps `createPolled`'s callbacks open does not arise.

The new row closes `reads`, `creates` and `returns` by copying
`createPolled`'s audited behavior with that specialization: the tick and
polled signals, the dependent initializer computation, and the split effect
and timer owner registrations. It returns exactly the polled accessor. The
private constructor `depSignal()` read stays ambient and `0..1`, never
strict-read-cleared. Delay is argument 0 and options argument 1. Their
positive rows (delay tracking; options get, `equals`, `unobserved`) are stated,
but `callbacks` stays open because nested option behavior is not enumerated.
`writes`, `invalidates`, `throws`, `cleanups` and `disposals` stay open. There
is no host-free row: on the server the export returns a constant accessor
before delegating.

All seven probe pairs pass in Chrome on rc.13: owner, top-level-read, leaf,
delay-tracking, options-get, equals-owner and unobserved-owner. A drafted
`private-read` pair was dropped. Chrome warns only from inside the package
file, which ADR 0253 does not accept as caller attribution, so that read is
backed by its citation alone.

## Measured consequences

- Primitives ledger, browser: 95 of 111 report correctly (was 94):
  `createIntervalCounter` top-level read. No correct twin has a violation.
- rc.13 corpus: no violation or uncertifiable site added or removed.
