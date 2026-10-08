# ADR 0260: createPagination reads and owners; destructuring through createRoot

- Status: accepted and implemented (2026-10-09).
- Owners: the `@solid-primitives/pagination@1.0.0-next.8` spec;
  `opaque_member_obligations` in `local_access.rs`.

## Spec

`createPagination` now closes `reads` and `creates`, so its owner requirements
are known. The claims cite the audited dist bytes:

- ten memo registrations on normal completion, tolerating no owner and
  requiring a children-capable owner when one is present;
- the private `opts()` read at construction, stated as ambient and `0..1`
  (an error can stop construction first), never as strict-read-cleared;
- a partial callback row for the options accessor, tracked under the first
  memo with minimum zero.

`callbacks`, `writes`, `invalidates`, `throws`, `cleanups` and `disposals` stay
open. Returned setter, handler and getter bodies stay opaque. Two new pairs,
`options-tracking` and `leaf`, pass in Chrome on rc.13 with every existing
pair.

## Destructuring through a resolved root

`const [, page] = createRoot(() => createPagination(...))` raised the
"returned value holds a function its contract does not describe" obligation.
The tuple's opaque setter was never selected, but the check looked only at the
inner call. When ADR 0259 resolves the root and its callback returns the
factory call directly, the outer destructuring now counts as that call's
destructuring. Resolution already refuses any selected slot that is not an
accessor, setter or plain value, so no opaque member is selected. Default,
rest and whole-value bindings stay unresolved and keep the obligation.

## Measured consequences

- Fixture `root-callback-return`: the `[, page]` obligation is gone. The
  `defaulted`, `restItems` and `rest` obligations remain.
- Primitives ledger, browser: 94 of 111 report correctly (was 93):
  `createPagination` top-level read. No correct twin has a violation.
- rc.13 corpus: no violation moved. One SC9005 import site (readingroom) is
  gone. In its place there are four precise uncertifiables at the two
  `createPagination` calls in component bodies: the package's own
  construction read, and the options callback whose invocation stays partial.

`createPolled`'s twin stays open: generic poll results may be async iterables,
and the vocabulary cannot state that result-of-result consumption.
