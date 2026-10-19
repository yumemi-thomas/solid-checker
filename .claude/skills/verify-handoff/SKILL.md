---
name: verify-handoff
description: Choose validation checks proportional to a solid-checker change and write the final handoff report. Use when a change slice is complete, before claiming a task done, when deciding which tests to run, or when a test run looks suspiciously green.
---

# Verification and handoff

Run each check once; do not repeat one while source, fixtures, binaries, and
environment are unchanged.

1. `make verify-delta BASE=<commit the slice started from>` selects the checks
   for the changed paths. Add `make coverage` after touching any fixture
   `node_modules` (git cannot see it).
2. `make verify` once before claiming done. Confirm success from its summary:
   a `FAILED during step` line or a missing TOTAL row means it did not pass.
3. Every commit must be green on its own: a dialect-seam change lands as one
   piece, and a snapshot update lands with the code that moved it.

## Suspiciously green?

- A process test passed with `SOLID_TYPEFACTS_BIN` overridden to empty, or
  against a stale `bin/solid-typefacts` (run `make build-typefacts`).
- Coverage ran against `bin/solid-checker-rust` instead of the fresh debug
  build (`make coverage` uses the fresh one).
- A negative fixture is vacuous: it would stay clean even if the rule were
  deleted.
- A fixture has no `node_modules/solid-js` stub, or the stub is not tracked:
  it ran under the default dialect.
- A gate result looks impossible: rerun with `SOLID_CHECKER_GATE_CACHE=0`.

## Report

- what changed and where;
- which fixtures and tests ran, with their actual results;
- which expensive checks ran or were deferred;
- generated artifacts changed (snapshots, manifests);
- the remaining fail-closed or uncertifiable cases.
