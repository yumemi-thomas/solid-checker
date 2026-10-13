# ADR 0243: Retrying three withheld primitive claims

- Status: accepted and implemented (2026-10-08).
- Owners: the `pagination`, `tween` and `range` specs and their probe pairs.
- Relation: ADRs 0236 and 0238 withheld four claims because a probe pair
  failed. Diagnosed read-only in `rust/target/research/primitives-retry/`.

## Decision

- `createTween` and `mapRange`: the failing misuse passed a runtime-created
  accessor straight to the export. Chrome's warning was then raised with
  package code as the first non-Solid frame (`tween/dist/index.js:28`,
  `range/dist/mapRange.js:28`), so it was not attributed to the case. The
  misuse now passes `() => source()` (or `() => step()`), so the package still
  invokes the claimed callback and the read happens in case-file code.
- `createPagination`: its constructor reads its own options memo
  (`pagination/dist/index.js:71-72`), a package-internal read that no
  case-file pair can be attributed to. The read operation and its pair are
  withdrawn, `reads` stays open, and the returned props/page claim ships.
- `createDropzone` stays withheld. Its public import loads
  `drag-drop@0.1.0-next.0/dist/context.js:3`, which imports `solid-js/web`,
  and rc.13 exports no such subpath. The published package cannot load on
  rc.13, so no pair can run.

## Consequences

- Browser ledger: 68 of 114 report correctly (was 66). `createPagination`
  and `mapRange` now prove their misuse; their correct twins keep an open
  domain. No correct twin on any host has a violation.
