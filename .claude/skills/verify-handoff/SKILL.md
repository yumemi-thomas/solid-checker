---
name: verify-handoff
description: Choose validation checks proportional to a solid-checker change and write the final handoff report. Use when a change slice is complete, before claiming a task done, when deciding which tests to run, or when a test run looks suspiciously green.
---

# Verification and handoff

Run each check once; do not repeat one while source, fixtures, binaries, and
environment are unchanged.

1. `make verify-delta` selects the checks for the paths changed since the last
   fully green commit (`BASE=<rev>` overrides). Add `make coverage` after touching any fixture
   `node_modules` (git cannot see it).
2. `make verify` once before claiming done. Confirm success from its summary:
   a `FAILED during step` line or a missing TOTAL row means it did not pass.
3. An engine change (`rust/`, `apps/solid-typefacts/`) also runs `make corpus-guard` once (ADR 1012: about 7 to 10 minutes, the frozen corpus and a
   baseline recorded for this machine): a lost violation, or a (project, file, rule) that was clean and holds an uncertifiable row, fails unless a new row of
   `scripts/guard/corpus-changes.json` names it. Exit 2 means it compared nothing (no corpus, or no baseline for this machine: `node scripts/guard/corpus.mjs
   record` from a main checkout); report that, never a pass. It is not in `make verify`: it needs the corpus and a per-machine baseline.
4. Every commit must be green on its own: a dialect-seam change lands as one
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
