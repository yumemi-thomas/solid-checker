# Native read scopes and hidden wrapper reads

## Result

Observing the native reader closes three earlier gaps without a contract for
each wrapper. Replaying the earlier population improves target hints from
11/15 to 14/15. The remaining target passes a read through unknown argument
flow. The new mechanism preserves the earlier 14/15 quiet controls in its own
hint channel and the existing pagination initialization warning.

After freezing the detector, 31 fresh cases give 8/12 target hints and 15/17
quiet controls. All 17 controls work. The four missed targets still show stale
results; two deliberate debugging controls still receive informational hints.
No universal package or rule coverage is established. These are authored
regressions and fresh challenges against retained real packages, not a new
real-app defect census.

## What changed

The previous mechanism tagged native getter functions and looked them up when
consumer code invoked them. Wrappers and bound copies have different function
identities. `native-read-runtime-v1.mjs` instead associates native nodes with
their exact accessor-construction witness, then opens a synchronous observation
scope around an admitted consumer call.

`native-read-hook-v1.mjs` inspects two exact retained rc.9 native artifacts:

- `dev.js` accessor constructor:
  `sha256:f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120`.
- `dev-shared.js` reader and `untrack` provider:
  `sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e`.

The second premise resolves the exact exported reader, observer, owner and
`untrack` declarations. Its five reader return paths record successful reads
and return the original value. A read that throws has no completed read ticket.
A consumer call that throws restores its scope and does not publish pending
tickets. Nested calls attribute reads to the innermost admitted call.

The hook also observes native `untrack` entry and exit with a `finally` guard.
This preserves explicit snapshot intent when an imported or local wrapper
calls native `untrack`. The guard is synchronous and does not attempt to carry
global context across Promise delivery.

`native-read-sites-v1.mjs` keeps exact declared memo/callback facts and the
earlier returned-expression grammar. It adds declared property calls with an
identifier receiver; computed and optional members remain open. This is
candidate enrollment, not guessed static member dispatch. Actual node reads
and runtime context are required for a hint. Unknown ordinary functions stay
quiet without being certified pure.

`async-read-transform-v6.mjs` wraps the original call in a thunk, preserving
receiver binding and property lookup order. It uses the existing project
session and an expanded necessary syntax gate. The native getter itself is
unchanged. The tests preserve getter identity/properties, returned object
identity, receiver behavior, exceptions, scope restoration and explicit intent.

`native-read-feedback-v1.mjs` authenticates current source/declaration facts,
mapped accessor creation, the exact native provider and mapped reader entry.
Only successful observed reads with owner and observer both absent produce
`OBSERVED_MEMO_CALLBACK_UNTRACKED_READ`. The channel is
`observed-native-read-in-call`, severity `info`, category `intent-open`, with
`staticDispatch: open`. The observed read need not flow into the returned
value, so intent and package result flow remain uncertifiable.

The helper returns the original consumer value and never restores a reactive
owner or observer. Stack collection and extra call frames are observable;
universal instrumentation neutrality and an overhead bound are not claimed.
Traces remain research evidence and can be mutated by application code.

## Populations

| Population | Target hints | Quiet new-hint controls | Quiet combined controls | Typing exclusions |
| --- | --- | --- | --- | --- |
| Earlier 34 cases, adapted replay | 14/15 | 14/15 | 13/15 | 4 |
| Fresh 31 cases after the freeze | 8/12 | 15/17 | 15/17 | 2 |

The replay newly catches the wrapper, bound-copy and member-call targets. Its
existing referenced-inspection hint remains noisy. The native pagination
warning remains identical to the plain run and is not counted as a new hint.

The fresh population uses real published typings and retained bytes from:

- `@solid-primitives/controlled-signal@1.0.0-next.3`;
- `@solid-primitives/promise@2.0.0-next.2`;
- `@solid-primitives/history@1.0.0-next.3`.

Together with the replay's pagination and media packages, the mechanism is
exercised across five distinct package artifacts. It adds no package-specific
contract, trust rule or callback timing model for these getters.

Fresh caught targets include the package's boolean, array and Set wrappers,
the toggle object's member getter, the `changed` wrapper, an imported wrapper,
a receiver-sensitive getter/method pair and a wrapper reading two sources.
All eight targets retain their initial displayed result after a source change;
their controls capture during memo compute and update correctly.

The receiver example preserves one lookup/call on the target and two on its
working control, with the correct `this` value in both runs. The two-source
target records two distinct native nodes and emits one call-site hint. Its
second raw event remains in the trace; the projector coalesces hints by site.

The four fresh misses are:

1. Undo history's store-backed `canUndo` call. The exact member candidate is
   enrolled, but produces no registered native read event. Store cells and
   fast reads are outside the current accessor-constructor/reader model; the
   precise missed store path still needs its own witness model.
2. An imported async wrapper that reads after `await`. Its synchronous call
   scope has ended before the read occurs.
3. A computed member call, outside the candidate grammar.
4. A callback defined in another module, outside the lexical memo/callback
   selection and active synchronous call scope.

All four targets remain stale and all four paired controls update. No silent
case is treated as proof of safety or non-reactivity. Ownerful untracked reads,
unknown argument/result flow and missing declarations also remain outside the
current hint claim.

Both deliberate `untrack` wrapper controls stay quiet and preserve their
initial snapshot. Ordinary non-reactive functions stay quiet. The two noisy
controls read a source for debugging inside a wrapper or member method but
return the intentional constant 9. Actual native reads cannot by themselves
establish that a source should drive the memo's result. These observations
remain informational and are not promoted to violations.

Two fresh typing cases are excluded before execution, with TS2322 and TS2339.
Separate `tsc --noEmit` runs confirm those errors against published typings
and pass the boolean target/control with no diagnostic. The earlier four
typing exclusions stay silent. No stub substitutes for the package types.

## Evidence and verification

The native-read detector's 296 source/support pins were sealed before fresh
case authoring. Every detector file remains unchanged after both populations.
The completed studies report `detectorFrozenBeforePopulation: true`. The
replay is explicitly adapted evidence; its consumers predate this detector.
Initial failed focused checks are retained, including a mistaken assertion
that Solid's wrapped computation error preserves the original Error object.
The corrected check compares actual native error name/message to the plain
runtime and preserves the failed-call exclusion.

Browser V6 retains native constructor, native reader and consumer frames with
source hashes. `native-read-audit-v1.mjs` imports neither selector, transform
nor projector. It independently checks exact declarations, memo aliases,
native binding/provider exports and mapped constructor/reader/call locations.
It audits 15 replay hints and 10 fresh hints, including all three noisy hints.

Plain/instrumented comparisons cover 65 cases and preserve source hashes,
package pins, published typing diagnostics, displayed values, callback counts,
native diagnostic deliveries and exceptions. Additional-file comparisons
preserve 16 replay file instances and seven fresh ones. There are no harness
failures. No real-app performance estimate is derived from these runs.

Primary retained artifacts:

- `rust/target/native-read-detector-freeze-v1.json`
- `rust/target/native-read-regression-preflight-v1/population.json`
- `rust/target/native-read-regression-browser-v1/browser/results.json`
- `rust/target/native-read-regression-study-v1.json`
- `rust/target/native-read-regression-audit-v1.json`
- `rust/target/native-read-regression-parity-v1.json`
- `rust/target/native-read-fresh-preflight-v1/population.json`
- `rust/target/native-read-fresh-browser-v1/browser/results.json`
- `rust/target/native-read-fresh-browser-plain-v1/browser/results.json`
- `rust/target/native-read-fresh-study-v1.json`
- `rust/target/native-read-fresh-audit-v1.json`
- `rust/target/native-read-fresh-parity-v1.json`
- `rust/target/native-read-additional-source-parity-v1.json`
- `rust/target/native-read-tsc-v1/results.json`

The prototype suite passes 244/244 tests, including 15 focused native-read
tests. All 296 prototype modules pass syntax checks. Seven historical/current
seals authenticate 296 distinct pins without changes. `make verify-fast`,
schema parsing, dialect manifest validation and whitespace checks pass. The
matching Type Facts producer is reused; formatting and pinned workspace
Clippy pass. Full `make verify`, production coverage/ownership, contract corpus
and certification gates are deferred for this research-only slice.

No production Rust rule, public contract, manifest, fixture stub or finding
snapshot changes. Generated observations are scoped under `rust/target/`.

## Next questions

The remaining store case tests how far native provenance can extend beyond
getter functions. Async wrapper/callback attribution requires a stronger
execution witness that does not restore tracking or share a global scope across
unrelated Promise deliveries. Deliberate debugging reads still need an intent
or value-flow distinction. Other core versions, HMR, server execution, project
references, real-app noise and stack retention/cost remain unverified.
