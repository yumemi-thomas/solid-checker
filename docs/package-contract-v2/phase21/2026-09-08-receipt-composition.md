# Receipt composition: what it actually was, and what it was worth

The [dependency-composition lever](2026-09-08-dependency-composition-lever.md)
closed with 145 refused artifact cases across 44 rows and one open question:
composing corpus rows into a shared authenticated catalog. This is that work,
and the first thing it found is that the question was the wrong one.

## Composition already exists, and it is not cross-row

Two lanes inside `contract certify` already compose a dependency's receipt into
its dependent. The published-dependency-graph lane acquires the dependency's
exact installed artifact, generates its proposal dependency-first, certifies it
in the same native transaction as the root, and binds the root's re-export
against it. Entrypoint recovery does the same over the union of the proposal's
own cases and the refused frontier. The composition is *within* a probe, over
the exact installed identity, in dependency order — which is why the version
and condition-coverage problems a cross-row catalog would raise do not arise.

Reusing another *row's* receipt would not work anyway, and the corpus says so
plainly. Of 121 distinct dependency identities the plans name as
`authenticated-receipt-unavailable`, exactly **three** are certified corpus
rows at the exact installed version: `@corvu/utils@0.4.2`,
`@solidjs/signals@2.0.0-rc.3` and `@solid-devtools/shared@0.20.0`. Probes pin
their own versions; dependents install different ones — `@solidjs/signals`
appears installed at `2.0.0-rc.6`, `2.0.0-rc.3` and `2.0.0-rc.0` in three
different rows. An exact-version rule refuses all but the coincidences, and
relaxing it is not on the table.

So the work was not to build a cross-row catalog. It was to find why 51 rows
whose refusal census names a dependency frontier were not all reaching the
lanes that already compose.

## Three reasons, all found in the audit

**The lane was requested by a reviewed list of probe ids, not by the row.**
The graph lane ran only under `--dependency-graph-lane`; recovery only for ids
named on the command line. That list held 48 ids, and 27 of the 51 frontier
rows were not on it. Five never asked for either lane —
`solid-js@2.0.0-rc.3`, three `@tanstack/solid-start` probes and
`@kobalte/core@2.0.0-alpha.0`, 34 refused cases between them.
[ADR 0072](../../adr/0072-composition-lane-by-refusal-census.md) makes the
request a policy over the row's own refusal census.

**One broken graph node refused every artifact case in the transaction.**
Preparation was all-or-nothing, and four rows paid for it in four different
ways: `@solidjs/start@2.0.3` on a `crossws` install missing above `h3`'s
declarations, `solid-devtools@0.34.5` on `@babel/core`'s
`./babel-7-helpers.cjs`, `@solidjs/web@2.0.0-rc.3` and `solid-js@1.9.14` on
`node:async_hooks`, a Node builtin that can never carry a package receipt.
[ADR 0071](../../adr/0071-independent-graph-case-preparation.md) isolates a
refused node to the exact cases whose graph reaches it.

**A retained case must never be traded away.** The first measurement of ADR
0071 dropped `@solidjs/start@2.0.3` from ten certified entrypoints to one: the
graph now prepared, but without the retained cases it could not carry, so
recovery published less than the proposal would have. The retained cases are
the floor, and at preparation time — unlike at certification time, where
[ADR 0070](../../adr/0070-independent-prepared-set-selection.md) weighs one
wager against another — the comparison is certain. A dropped retained case now
abandons the graph.

## A resource budget the policy needed

Routing by census sent `@kobalte/core@2.0.0-alpha.0` to recovery, which
prepares its 59 generated cases alongside four frontier ones. The probe's
process tree exceeded the runner's 4096 MiB ceiling and was killed: the row
went from 59 certified entrypoints to `infrastructure-failure` and zero, for
the sake of four frontier cases. The corpus totals fell to 1,093 entrypoints,
which is how the run reported it.

Recovery therefore refuses a prepared set above 32 cases before doing any work,
and the row keeps its proposal. The bound is measured, not chosen: 24 cases
over 113 nodes prepares and certifies (`@kobalte/utils@0.9.2`); 59 did not
survive. It is a resource deadline and says so — not a semantic ceiling.

## Measured

`2026-09-08-receipt-composition-full-v2.json`, 418 probes, `--timeout 900`,
exit 0, 817.7 s against the baseline's 816.2 s.

| Metric | Before (ADR 0070) | After (ADRs 0071, 0072) |
| --- | ---: | ---: |
| Complete rows | 324 | 324 |
| Partial rows | 62 | 62 |
| Refused rows | 23 | 23 |
| Certified entrypoints | 1,151 | **1,152** |
| Rows with the root certified | 370 | **371** |

**One row changes and 417 are identical, with nothing lost:**

| Probe | Before | After |
| --- | --- | --- |
| `solid-js@2.0.0-rc.3`, Solid 2 | 1 entrypoint, root uncertified, lane `reused-proposal` | **2 entrypoints, root certified**, lane `published-graph` |

Counting the frontier directly rather than by row: dependency-composition
artifact cases that reach publication went from **61 of 173 to 65 of 173**.

That is a small gain for a correct rule, and the honest reading is that the
scheduling and the all-or-nothing preparation were never what most of the
corpus was blocked on. The value of these two ADRs is that the composing lanes
now reach every row whose census asks for them, and that the audit says
precisely what stopped each one -- which is what the census below is made of,
and it could not have been written before.


## What remains, exactly

The 108 unrecovered frontier cases, by what actually blocks them. None is a
composition gap.

| Cases | Blocker | Rows |
| ---: | --- | --- |
| 39 | **Type Facts proof refusal inside a composed graph.** The dependency composes; the proof does not close. | `corvu@0.7.2` (18), three `@tanstack/solid-router` (14), `@corvu/popover`, `@corvu-next/popover`, `@solid-primitives/intersection-observer` (×2), `@tanstack/solid-db` |
| 28 | **The 32-case recovery budget.** A resource deadline, and the one item here that is a deliberate trade rather than a missing fact. | `solid-js@1.9.14` (11), `@solidjs/web@2.0.0-rc.3` (11), `@kobalte/core@2.0.0-alpha.0` (4), `@kobalte/solidbase` (2) |
| 24 | **A Node builtin in the graph.** `node:async_hooks`, `node:stream`: not a package, so never a receipt. Needs the runtime-library behaviour model, not composition. | three `@tanstack/solid-start` (21), three `@tanstack/solid-start-server` (3) |
| 8 | Cases inside a working graph lane that the transaction did not publish. | `solid-js@2.0.0-rc.3` (5), `@tanstack/solid-table` (2), `@tanstack/solid-pacer` (1) |
| 7 | **A broken transitive install.** `crossws` missing above `h3`'s declarations, `@babel/core`'s `./babel-7-helpers.cjs`, `aria-query`, `partial-json`. Publisher and install facts. | `solid-devtools` (3), `@solidjs/start` (2), `@solidjs/testing-library`, `@tanstack/ai-solid` |
| 2 | Generation produced nothing to recover. | `@tanstack/ai-solid-ui@0.7.20` |

Two blockers deserve their exact wording, because both were guessed at before
and are now measured.

**Cross-row receipt reuse is not the answer, and the corpus says so.** Of 121
distinct dependency identities the plans record as
`authenticated-receipt-unavailable`, three are certified corpus rows at the
exact installed version. Probes pin their own versions and dependents install
others; `@solidjs/signals` alone appears as `2.0.0-rc.6`, `2.0.0-rc.3` and
`2.0.0-rc.0` in three different rows.

**`solid-js`'s eleven cases have a decided answer already, and it is not a
resolver change.** `solid-js/web/dist/web.js` re-exports `ErrorBoundary` from
`"solid-js"`, and the obvious reading -- that the bytes are in the same archive,
so the edge should resolve locally -- is exactly the reading
[ADR 0012](../../adr/0012-self-package-export-target-rebinding.md) considered
and rejected, on this same package and this same export: *"Both paths are in
one archive, which does not make the bare package import a relative file
edge."* A self-package edge is a **semantic dependency on another artifact case
of the same package**, planned and bound like any other, and the mechanism is
live -- `fixtures/package-contracts/self-package-rebinding` and
`self_package_rebinding_keeps_native_dependency_and_target_verification` pin
it, and ADR 0012 was written *for* the Kobalte 0.9.2 graph that certifies 22
entrypoints today.

Both the exports-map self-reference and the located-root variant were
implemented in the JS resolver, and the located-root variant in the Rust
certifier as well, before that was found. Both are reverted. Three things they
established, worth not rediscovering:

- The two resolvers must change together or not at all. The JS-only attempt
  cost `solid-js@2.0.0-rc.3` two graph cases to
  `artifact module closure mismatch`, because the Rust certifier rebuilds each
  artifact's closure from snapshot bytes and compares.
- Neither variant recovers the eleven cases, and the exports-map one cannot:
  `solid-js/web/package.json` names itself `"solid-js/web"`, so Node's
  `PACKAGE_SELF_RESOLVE` does not match there either.
- The located-root variant *does* work, and deletes ADR 0012's mechanism by
  making the edge local before anything can plan it. `self_package_rebinding`
  fails outright under it.

So the eleven cases need the composing lane to reach that row, which is what
the two blockers above already say: 42 prepared cases against a 32-case budget,
and `./web/storage` reaching `node:async_hooks` behind the retained floor.
Reopening the ADR 0012 decision is a separate question, and it is not one a
coverage measurement should answer on its own.

## Validation

`bun test packages/cli/test/contract-workflow.test.mjs`: 93 pass, 0 fail,
including the graph-node refusal attribution (a refused node refuses exactly
the cases whose graph reaches it, and no others), the cascade to every node
generated against its contract, the retained-case floor, and the recovery case
budget. `scripts/ecosystem-benchmark/run.test.mjs`: 65 pass, with the routing
rule's three arms pinned -- a frontier row asking for recovery, an explicit
`--dependency-graph-lane` still selecting the frontier-only lane, and a row
with nothing generated to retain keeping its reuse.

Full `make verify` passes with actual exit 0 — `TOTAL 102.22s` for the
measured tree, and `TOTAL 149.39s` again after the two resolver variants were
reverted — with no `FAILED during step` marker in either run.

No protocol, receipt, snapshot or bundled contract changed. No commit or push
was made.

