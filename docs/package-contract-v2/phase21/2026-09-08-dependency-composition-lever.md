# The dependency-composition lever: what it is, and what it was worth

`dependency-composition` was the largest single refusal class on the partial
rows — 173 refused artifact cases across 51 rows, 67 distinct
`(dependency, export)` pairs, dominated by a handful: `solid-js::ErrorBoundary`
(11), `solid-js::Errored` (9), `@solidjs/signals::$PROXY` (9),
`@tanstack/*` unresolved dependency modules (~30). This is the investigation and
the measured result.

## What the refusal actually is

It is a **generation** refusal, not a proof one. The runner's `generateContract`
hook passes no dependency catalog, so a root proposal is generated
dependency-blind. `bindExport` then reaches an external re-export —
`@kobalte/utils`'s `./src/external.ts` is `export { Key } from
"@solid-primitives/keyed"` — finds no accepted binding, and refuses the case
before it ever becomes a certification candidate. Certification is then handed
that same proposal with `--proposal`, so the case never gets a second chance.

The refused export is not missing. Resolving `@solid-primitives/keyed@1.5.3`
directly produces six exports including `Key` (`export function Key(props)` at
`dist/index.js:115`). Nothing about the dependency is unprovable; it was simply
never offered.

**All 173 cases sit in the 51 rows whose dependency plan is not complete**
(49 `exact-leaf-refusal`, 2 `resource-refusal`); the other 367 probes have no
dependency plan at all. The dominant leaf kind across every plan is
`authenticated-receipt-unavailable` (740 of 2,125).

## Two distinct sub-causes, only one of which is a defect here

**Self-reference (11 cases, 1 row).** `solid-js/web/dist/web.js` line 2 is
`export { ErrorBoundary, For, Index, … } from 'solid-js'` — the package naming
*itself*. Node resolves that through the package's own `exports` map; the
checker treats it as an external dependency it can never hold a contract for.
That is a resolver gap with no trust surface at all — same package root, same
archive, same digests — and it is worth closing, but it is one row.

**No authenticated dependency receipt (162 cases, 50 rows).** For
`solid-js@2.0.0-rc.3` the plan's leaves name `@solidjs/signals@2.0.0-rc.3` as
`authenticated-receipt-unavailable` under `browser`, `deno`, `worker` and more —
even though that package is itself a corpus row that certifies. Each probe runs
in its own isolated output directory and no row's receipt is ever fed to
another. Closing this means composing corpus rows into a shared authenticated
catalog in dependency order: a scheduler change, a trust-domain decision, and a
condition-coverage problem (a dependency certified under one condition set does
not authenticate the others). **That is a policy call, not a bug fix, and it was
not taken here.**

## What was taken: ADR 0070

Entrypoint recovery already prepares the retained proposal cases *plus* exactly
the refused dependency-composition cases, and certifies them against an
authenticated dependency catalog — 24 prepared cases where `@kobalte/utils`'s
proposal had 20. That set was being thrown away. The retained cases are the
baseline every incremental trial extends, and a baseline failure ended the
strategy *and* abandoned the graph, falling back to the dependency-blind
proposal. One honest [ADR 0069](../../adr/0069-original-input-by-position.md)
refusal on `./src/scroll-into-view.ts` therefore took all four frontier cases
with it.

[ADR 0070](../../adr/0070-independent-prepared-set-selection.md) separates
ending the strategy from abandoning the lane: on a baseline failure the same
bounded selection runs across the whole prepared set, gated on nothing having
been published before.

The first implementation grew a prefix, one transaction per case. Every trial
re-certifies the whole prepared graph — 113 nodes for `@kobalte/utils` — so that
took the row from 134.6 s to a 600 s timeout, and took `@kobalte/core` down with
it through host contention. Binary subdivision instead: **14 native
transactions, 307 s scoped**, same selection.

## Measured

`2026-09-08-prepared-selection-full-v2.json`, 418 probes, `--timeout 900` (raised
so no row times out; the base run had no timeouts at 600 s, so the semantic
comparison is unaffected), exit 0.

| Metric | Before (ADR 0069) | After (ADR 0070) |
| --- | ---: | ---: |
| Complete rows | 324 | 324 |
| Partial rows | 62 | 62 |
| Refused rows | 23 | 23 |
| Certified entrypoints | 1,148 | **1,151** |
| Rows with the root certified | 369 | **370** |

**One row changes and 417 are identical, with nothing lost:**

| Probe | Before | After |
| --- | --- | --- |
| `@kobalte/utils@0.9.2`, Solid 1 | 19 entrypoints, root uncertified | **22 entrypoints, root certified** |

Its recovery published 23 of 24 prepared cases; the one refusal is
`./src/scroll-into-view.ts`, the ADR 0069 blocker, recorded exactly. Four rows
took the new strategy — `@kobalte/utils` and three `@tanstack/solid-router`
probes; the router rows published the same set as before and gained nothing.

## Honest accounting of the lever

Across the corpus, dependency-composition cases actually recovered and published
went from **24 of 173 to 28 of 173**. **145 remain unrecovered across 44 rows** —
`corvu@0.7.2` (18), `@solidjs/web` (11), `solid-js@1.9.14` (11),
`solid-js@2.0.0-rc.3` (9), the `@tanstack/solid-start` and `solid-router`
families.

So the class is *not* closed, and the earlier estimate that it gated up to 39
partial rows was a first-blocker ceiling, not a forecast — exactly the caveat
this branch has measured before (ADR 0042 cleared the largest form class and
closed six candidates; ADRs 0049 and 0069 measured zero). What ADR 0070 closed
is the subset where a prepared graph existed and one bad retained case was
discarding it. The remaining 145 need the receipt-composition decision above,
which is the user's call, not a defect to fix.

## Validation

`bun test packages/cli/test/contract-workflow.test.mjs`: 88 pass, 0 fail,
including three new ADR 0070 routes — a refused retained case selecting
independently and publishing the frontier, an existing (and an unstated)
publication never reduced, and a prepared set proving nothing rethrowing so the
proposal fallback still runs — plus a transaction-count bound pinning
subdivision. Full `make verify` passes with actual exit 0, `TOTAL 101.17s`, and
no `FAILED during step` marker. No protocol, receipt, snapshot or bundled
contract changed. No commit or push was made.
