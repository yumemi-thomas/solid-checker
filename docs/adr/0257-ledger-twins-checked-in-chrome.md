# ADR 0257: Ledger twins checked in Chrome

- Status: accepted and implemented (2026-10-09).
- Owner: `fixtures/primitives-misuse/cases.json`.
- Relation: completes ADR 0241's expectation review for the 20 browser
  cases still failing after ADR 0256. Both twins of each were run in Chrome
  on rc.13 (`misuse-runtime-ledger.mjs`).

## Context

A ledger case asserts that its misuse is a defect and its correct twin is
not. For 16 of the 20 remaining cases Chrome agrees: the misuse warns and the
correct twin is silent. Their failure is a checker gap. Four cases fail
Chrome itself.

## Decision

1. **`createIntervalCounter`, `createPolled` and `createPagination`
   (top-level read).**
   - Their "correct" twins warn STRICT_READ_UNTRACKED: each primitive reads
     its own state untracked while it is constructed, so constructing it
     inside a component warns whatever the caller does.
   - Both twins now build the primitive in a module-level `createRoot`, so
     only the read position differs.
   - Chrome warns for the misuse and is silent for the correct twin, under
     fresh case ids (`MISUSE_CASES_ROOT` set, so the edited twins actually
     ran).
2. **`combineProps` (top-level read) is removed.** Chrome is silent for its
   misuse, so it is not a misuse case.

## Consequences

- Primitives ledger, browser: 92 of 111 report correctly. The total drops by
  the removed case.
- The three rewritten cases now fail the checker for an honest reason. It
  does not carry the primitive's returned accessor through `createRoot`'s
  return of its callback's value, so the misuse read is not proven. That
  passthrough is a general consumer gap, recorded in the precision backlog.
