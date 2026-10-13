# Native store targets and package shortcuts

## Result

There is reason for measured optimism: observing the Solid runtime beneath
package wrappers keeps extending useful feedback without a behavior contract
for every getter. It still needs another mechanism for packages that bypass
the runtime when reading without an observer.

The V7 research profile adds native store target provenance and property
expression candidates. It catches the previously missed history getter in
the unchanged 31-case replay, improving target hints from **8/12 to 9/12**.
The same two intentional debugging wrappers remain noisy; **15/17** working
controls receive no new hint.

After sealing the detector, a fresh 38-case property/store challenge gives:

| Population | Targets with hints | Controls without new hints | Controls without any feedback | Typing exclusions |
| --- | --- | --- | --- | --- |
| Earlier 31-case replay | 9/12 | 15/17 | 15/17 | 2 |
| Fresh 38-case challenge | 8/15 | 20/21 | 19/21 | 2 |

All 38 working controls across the two populations behave as intended. All
27 targets retain their demonstrated stale result. Hints explain an observed
read; instrumentation does not repair the application. All 69 comparisons
against plain execution preserve displayed values, callback counts, source
and package pins, native diagnostic deliveries and exceptions. There are no
harness failures.

The fresh population deliberately includes several known boundaries. Its
score is a challenge result, not an estimate of typical application coverage.
The replay is adapted evidence; its consumers predate this detector.

## What changed

Untracked store property reads can serve backing values without entering the
native reactive cell reader. The earlier getter/node hook cannot observe
those reads. The new hook tags the actual target returned by `createTarget`
and records successful calls to its own-data serving function during an
admitted consumer expression.

`native-store-premise-v1.mjs` resolves the exact returned target binding, the
native `Proxy` argument, its handler declaration, the handler's final `get`
dispatch, and the serving function declaration. All are tied to the inspected
rc.9 artifact bytes. Names select the inspected code; exact bindings and
structure establish the relationships. The hook preserves the actual target,
Proxy, getters and returned values.

`native-read-sites-v2.mjs` adds declared property paths such as `state.n`,
`state.user.n` and `state.list.length`. Exact memo aliases, callback spans and
declarations still admit candidates. A declaration does not prove which
receiver implementation runs. Native identity, serving-entry and consumer
frames supply the observed execution witness; static dispatch remains open.

`native-read-runtime-v2.mjs` stores identities in WeakMaps and commits a ticket
only after the original read and enclosing expression succeed. Existing
native reader observation and synchronous `untrack` intent remain. Multiple
store keys in one expression retain separate raw events but produce one hint.
Runtime ownership and observation are sampled from that native instance; both
must be absent. Scopes end synchronously and do not propagate after `await`.

The V7 transform and browser profile use the new hooks, selector and feedback
projector. Existing executed versions remain unchanged. Feedback is `info`,
`intent-open`, with `authority:false`, `certification:false` and open static
dispatch. Callback timing, result flow and developer intent remain open.

## Fresh detections and misses

The eight detected targets are own properties, nested properties, array
length, a previously materialized property, an imported store, a store
accessor property, a wrapper around history's `canUndo`, and a consumer-created
callback passed to an external helper.

The last case is useful: the callback expression remains in the consumer's
source, so its actual later store read can be scoped without asserting the
helper's callback schedule. A callback body declared only in an external
module remains outside this selection path.

Seven fresh stale results receive no new hint:

| Case | Missing evidence or unsupported path |
| --- | --- |
| `ReactiveMap.size` | Its trigger cache returns before native reading when no observer exists. |
| `createBreakpoints(...).wide` | Its static store returns a copied value before materializing a signal when no observer exists. |
| An absent optional store property | The native `get` trap's absent-key branch bypasses the own-data serving function. |
| Computed property access | The consumer selector does not admit computed keys. |
| Optional property access | The consumer selector does not admit optional chains. |
| Explicit `state.then` | The read observer excludes `then` to avoid implicit Promise probes. |
| A store read after an imported async helper's own `await` | The synchronous consumer call scope has already ended. |

The replay still misses an imported async wrapper, a computed member call and
a callback body declared elsewhere. Unknown argument flow, symbol reads,
inherited store methods, ownerful untracked reads and several native accessor
branches remain outside the supported profile. No unresolved path is promoted
to a violation.

These misses identify two next mechanisms: broader native store branch
witnesses, and source-derived evidence for package observer guards that
intentionally avoid native reads. Guard evidence must establish the exact
imported observer, branch, fallback and tracked path. Observing a call to
`getObserver` alone would not establish a reactive defect.

## Precision and typings

Explicit and wrapped native `untrack`, ordinary objects, an ordinary Proxy and
a discarded direct store read remain quiet. A constant-returning wrapper that
reads a store for debugging receives an informational hint despite its result
being intentionally fixed. Together with the two earlier debugging wrappers,
this is three noisy hints across the two populations. Successful native reads
do not establish that those values drive the returned result.

The materialized-property control has an existing `STRICT_READ_UNTRACKED`
warning from reading its auxiliary memo in the component body. It occurs in
plain and instrumented execution, and explains the difference between 20/21
quiet new-hint controls and 19/21 quiet combined controls. Its result updates
correctly. The native warning is retained rather than attributed to the new
consumer read hint.

Preflight and browser enrollment use real published declarations. Two fresh
type-invalid examples are excluded before execution and receive no feedback:
missing store property (`TS2339`) and assigning a string to a numeric draft
property (`TS2322`). Eight explicit `tsc --noEmit` runs confirm six clean
examples, including map/media target-control pairs, and those two diagnostics.
The two replay typing exclusions also remain silent. No fixture stub is used.

## Evidence and verification

Two seals authenticate 309 and 310 input pins. The second adds the first
fresh case module while leaving the detector unchanged. The initial frozen
consumer flow assumed the wrong initial media viewport; `native-store-cases-v2`
starts at the harness's actual 1280px viewport and then crosses the breakpoint.
The original case module and its unexecuted preflight remain available. Both
completed studies authenticate that detector modules were sealed before their
population and remain unchanged across execution.

`native-read-audit-v2.mjs` imports neither selector, transform nor projector.
It independently checks property/call declarations, memo identity, actual
constructor/Proxy/handler bindings, native serving entry and mapped consumer
locations. It audits all 20 hints, including the three noisy hints. Runtime
traces remain research evidence, not an adversarial root of trust.

Primary retained artifacts:

- `rust/target/native-store-detector-freeze-v1.json` and `-v2.json`
- `rust/target/native-store-regression-preflight-v1/population.json`
- `rust/target/native-store-regression-browser-v1/browser/results.json`
- `rust/target/native-store-regression-study-v1.json`
- `rust/target/native-store-regression-audit-v1.json`
- `rust/target/native-store-regression-parity-v1.json`
- `rust/target/native-store-fresh-preflight-v2/population.json`
- `rust/target/native-store-fresh-browser-v1/browser/results.json`
- `rust/target/native-store-fresh-browser-plain-v1/browser/results.json`
- `rust/target/native-store-fresh-study-v1.json`
- `rust/target/native-store-fresh-audit-v1.json`
- `rust/target/native-store-fresh-parity-v1.json`
- `rust/target/native-store-additional-source-parity-v1.json`
- `rust/target/native-store-tsc-v1/results.json`

The prototype suite passes **262/262**, including 18 new focused store tests.
All 310 prototype modules pass syntax checks. Nine seals authenticate 310
distinct historical/current pins without changes. Additional-file comparisons
preserve 13 file instances. `make verify-fast`, schema parsing, dialect manifest
validation and whitespace checks pass; the matching producer is reused and
pinned workspace Clippy passes. Full `make verify`, production coverage and
ownership, contract corpus and certification gates are deferred for this
research-only slice.

No production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated observations stay under `rust/target/`.
Real-application noise, browser cost, stack retention, other Solid artifacts,
HMR, server execution and project references remain unverified here.
