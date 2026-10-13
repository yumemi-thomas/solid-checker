# ADR 0233: A closed return may not hide an opaque member

- Status: accepted and implemented (2026-10-08).
- Owners: `scripts/author-contracts.mjs` (`opaqueMember`, in
  `validateClosures`); the `createRAF` spec; `fixtures/primitives-misuse/cases.json`.
- Relation: corrects ADR 0227, which closed `createRAF`'s `returns`. Records
  the soundness boundary the returned-member format extension
  (`rust/target/research/format-extension/DESIGN.md`, E1) has to respect.

## Context

A contract may return a tuple or object with an opaque `callable` (or
`unknown`) member. The consumer projects only the reactive leaves of a
returned container and keeps no obligation for invoking an opaque member.
While `returns` is open, the import keeps a `package-contract-incomplete`
notice. Once `returns` is closed, nothing remains, and every call of that
member is certified clean.

Measured on rc.13:

- `createRAF`'s returned `start` reads its `running` signal before deciding to
  start. Called in a component body, Chrome raises `STRICT_READ_UNTRACKED`; the
  checker, with `returns` closed (ADR 0227), reported nothing.
- Closing `createDate`'s and `createDateNow`'s `returns` the same way, which is
  the cheap path the design research proposed, made `setDate(1)` and
  `update()` in a component body come out clean too. That closure was
  reverted before commit.

## Decision

1. `createRAF` and its `default` alias leave `returns` open again.
2. The authoring tool refuses a closed `returns` whose return output is a tuple
   or object with a `callable` or `unknown` member, until the consumer keeps an
   obligation for invoking such a member.
3. The ledger gains `raf-createRAF-start-top-level` (Chrome raises
   `STRICT_READ_UNTRACKED`). Its correct twin calls `start` from a click
   handler.
4. `event-bus-toEffect-module-scope`'s correct twin is fixed. Calling the
   returned emitter inside the `createRoot` body writes `toEffect`'s stack
   signal in an owned scope, which Chrome rejects. The twin now emits from a
   microtask, and Chrome reports nothing.

## Consequences

- Primitives ledger, browser: 16 report correctly (was 18).
  `raf-createRAF-top-level-read` and `raf-default-top-level-read` still prove
  the misuse; their correct twins keep the incomplete-contract notice. No
  correct twin on any host has a violation.
- The two date cases cannot become clean without the returned-member
  extension: a per-member invocation obligation in the consumer, then
  `effectful-callable` claims for the members a caller invokes.
