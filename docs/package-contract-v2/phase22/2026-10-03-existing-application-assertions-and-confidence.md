# Existing application assertions and confidence

## Outcome

We can be cautiously optimistic about useful feedback without certifying every
package. The strongest recent evidence is still the authored replay experiment:
ten failing value assertions pass after an exact source capture proposal, and
22 correct controls stay quiet in that conditional feedback channel. Its policy
was adapted to those examples; it is not a general warning precision estimate.

This follow-up runs existing tests from the retained `finds-team/frontend`
application. All 24 selected assertions pass without changing the test or
application source. No genuine failing package-use case is found in this group.
These runs establish usable application baselines, not additional detections,
quiet feedback controls or proof that the application is correct.

## Trial

`real-app-existing-tests-v1.mjs` copies 200 application inputs into a fresh
directory and uses the original Vite configuration and retained dependencies.
The application and package inputs are authenticated before and after each
run. There are no installs, seeded defects, feedback transforms or source fixes.
The existing request tests provide their own fetch mocks.

| Selection | Existing test files | Passed assertions | Failed | Skipped |
| --- | ---: | ---: | ---: | ---: |
| Dialog, select, virtual list and UI composites | 4 | 12 | 0 | 0 |
| Relay network, connections, SSR isolation and router document | 4 | 12 | 0 | 0 |
| Total | 8 | 24 | 0 | 0 |

The retained versions include Solid/signals/web `2.0.0-rc.9`, TanStack
Solid Router/Start `2.0.0-rc.8`, virtual-core `3.17.11`, Relay `20.1.1`,
Vitest `5.0.1` and Vite `8.3.0`. Original inputs remain byte-identical. The
original configuration regenerates `src/routeTree.gen.ts` only in each isolated
copy; every other copied input remains byte-identical.

The UI tests exercise server output. Dialog and select use the application's
native fallbacks; they do not execute Kobalte. Virtual-list server output does
not execute its browser-owned virtualizer setup. Passing these assertions
therefore does not establish those packages' browser lifecycle behavior.
Relay tests exercise the real runtime with mocked transport, including filter
membership, paging, overlapping request isolation, cache restoration and the
router's server document boundary.

The two test processes take approximately 1.79 and 1.67 seconds, excluding
input authentication and copying. These are single development observations,
not a performance benchmark or feedback latency measurement.

## What supports optimism

The replay experiment demonstrates a useful, bounded statement: a particular
source change made a particular failing assertion pass. Exact source bindings
can propose that experiment without a hand-authored package contract. Existing
application tests can supply expectations and checks beyond the primary value.
That gives a practical path to feedback for tested interactions.

Confidence is lower for automatic warnings across arbitrary applications.
The prior capture proposal changes task counts in 18 of 20 trials, and visible
counter controls demonstrate changed effects despite an unchanged primary
result. Missing observations, refused getter shapes, source discovery gaps,
execution coverage, startup cost and unbounded runtime buffers remain open.
There is still no demonstrated positive defect from an unchanged real
application. No claim covers all packages or all rules.

The next substantial confidence gain should come from genuine failing
application assertions, with the broader existing suite checked after each
proposal. A seeded failure must remain labeled as seeded. More successful
authored examples alone cannot establish real-world warning precision.

## Verification and artifacts

`real-app-existing-test-audit-v1.mjs` independently compares raw Vitest results,
the exact selected test files, assertion counts, runner bytes, original/copy
inputs and package closure digests. It rejects failed, skipped or empty trials.

Ignored outputs are `rust/target/real-app-existing-{ui,relay}-v1/`, containing
the isolated application copies, raw test logs, raw JSON and authenticated run
reports, plus `rust/target/real-app-existing-test-audit-v1.json`.

The prototype detector and its 734-test population are unchanged; those tests
are not rerun or counted as new application assertions. No new TypeScript rule
or typing check is added. Syntax and the independent baseline audit are the
focused checks. Fast handoff checks cover the producer stamp, Rust formatting,
pinned workspace Clippy, schema/manifest validation and whitespace. Full
production verification, coverage, ownership, contract and release gates remain
deferred for this research/documentation slice. No production code, snapshot,
contract, schema or manifest changes.

Earlier runtime and feedback profiles remain immutable. See
[capture replays and feedback from failing tests](2026-10-03-capture-replays-and-test-assisted-feedback.md)
for the conditional suggestions and their precision and repair limits.
