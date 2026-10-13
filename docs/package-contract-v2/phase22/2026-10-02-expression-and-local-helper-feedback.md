# Feedback through expressions and local helpers

This exploration closes the earlier computed-property getter miss and adds
feedback through ordinary expressions and simple local returns. The original
population still receives matching feedback on **18/18 targets**. Its twenty
controls stay quiet; the existing pagination package defect remains a failed
control. The four snapshot additions remain informational, not proven
violations.

Another **46 consumers** were authored and executed in two stages. Two real
published-typing errors were excluded before execution. Historical consumers,
labels, prototype versions and observations were preserved.

## Results

| Population and detector | Matching targets | Quiet, correct controls | Typing exclusions |
| --- | --- | --- | --- |
| Previous nine-row transfer, adapted v7 replay | 3/3 | 6/6 | 0 |
| New thirty-row expression population, v7 sealed before authoring | 9/10 | 18/18 | 2 |
| Same expression population, adapted v9 replay | 10/10 | 18/18 | 2 |
| New sixteen-row helper population, v9 sealed before authoring | 4/6 | 10/10 | 0 |
| Earlier twenty-nine-row challenge population, adapted v9 replay | 10/10 | 15/17 | 2 |

There were no harness failures, unrelated-feedback successes or failed
controls in the two new populations. Every target demonstrates the authored
stale-output behavior. The fresh helper misses stay in the denominator.
Adapted replays are distinguished from detectors sealed before authoring.

## A broader value-flow mechanism

V7 joins an authenticated executed tracking-skip guard to a setup initializer
whose value is later used in JSX. Supported paths include computed properties,
arithmetic, conditionals, arrays, object fields and template substitutions.
The detector has no export-specific recipes or target/control labels.

The new population exercises mouse position and window dimensions, plus two
packages outside the earlier snapshot populations: element bounds and active
element. The first seven targets use observed package guards; bounds and
active-element produce native strict-read feedback. All nine controls display
updated values after actual mouse, resize, scroll or focus events.

Value retention matters. A comma expression can discard a read, and a helper
can ignore an eagerly evaluated argument. Those observations cannot establish
that the displayed value retains the read. V7 rejects discarded operands,
unknown argument flow, deferred functions and unsupported expression paths.
The corresponding real consumers remain quiet and display their intended
constants.

Exact native `untrack` calls suppress observed hints. TypeScript resolves the
actual Solid export, renamed imports, namespace members and immutable local
aliases. An API spelling alone is not a snapshot marker. Explicit scalar and
object snapshots remain quiet.

V6 exposed a regression while closing computed properties: a dynamic method's
source map pointed to its opening parenthesis, outside the callee token span.
V7 admits the call punctuation while continuing to reject reads inside unknown
arguments. Both results are retained; the corrected earlier transfer reaches
3/3 targets and 6/6 quiet controls.

## Following a local return

The tenth expression target captures `readX()`, where a named local helper
returns the package getter. Its recorded guard location is inside the helper,
outside the displayed initializer. V7 therefore leaves the connection open.

V8 follows one exact local symbol edge: a zero-argument function declaration or
immutable arrow with a single property return, one visible reference that is
the setup call, and a later JSX use of the captured value. The authenticated
guard must occur inside that return expression. The hint points to the original
call and records the helper, return and call spans plus the source hash. No
runtime observation is relocated to pretend it occurred at the caller.

A retained falsifier shows why the outer return shape is insufficient:

```ts
function readX() {
  return ignore(position['x']).x; // ignore returns { x: 9 }
}
const frozen = readX();
```

V8 gives this an unwanted hint. V9 refuses returned-member paths containing an
unknown call, constructor or deferred function. Its browser counterexample
remains quiet and displays `9`. The adapted expression population then reaches
10/10 matching targets with 18/18 quiet controls.

After sealing v9, sixteen new consumers test arrow helpers, transparent return
wrappers, renamed resize imports and a reactive map size getter. Those four
target/control pairs transfer successfully. Discarded returns, ignored return
arguments and explicit helper snapshots remain quiet.

Two targets still miss: a helper with multiple visible callers and a helper
that escapes through an application property. Both remain stale. The detector
does not guess which call produced the recorded guard. Multiple-call,
escaped, async, branching, parameterized and richer return paths remain open.
This extension claims same-file resolution only.

## Precision and scalability

Replaying the older twenty-nine-row population identifies a cost of broader
observations. Its prototype-inspection control deliberately keeps a snapshot
but has no intent marker. The earlier static class channel refused the path;
the new observed channel has positive execution evidence and emits an
informational hint. Together with the existing implicit-intent control, this
gives **two noisy controls out of seventeen**, up from one. Both retain their
passing desired behavior and stay in the score.

This is evidence for reusable value-flow feedback rather than whole-package
certification. It also supports keeping observed intent hints separately
controllable from proven errors. They must not become errors solely because
an execution observed an untracked read. Explicit snapshot intent resolves
the tested controls; desired test output never supplies a hidden suppression.

The next concrete source of evidence for the two helper misses is the actual
caller stack mapped back to original source, combined with an exact return
path. The current guard profile maps only its nearest consumer frame. A richer
profile would need fresh observations and controls; the current data does not
prove the missing caller connection.

Unexecuted paths, server behavior, CommonJS guard loading, ambiguous or mutable
dispatch, cross-file helper flow, default/rest bindings and other untested
rule families remain open. Unknown package behavior acquires no certification
authority. These results do not establish feedback coverage for every package
or rule.

## Evidence, cost and verification

All consumers use retained published packages and Solid/signals/web
`2.0.0-rc.9`. Package versions are:

- Mouse `4.0.0-next.3`, resize-observer `4.0.0-next.3`, bounds
  `1.0.0-next.2`, active-element `3.0.0-next.2` in the expression population.
- Mouse and resize-observer at those versions, and map `1.0.0-next.2`, in the
  helper population.

The thirty-row population freezes 316 resolved declarations before execution;
the sixteen-row population freezes 252. Missing mouse property access produces
TS7053 and readonly bounds assignment produces TS2540 against actual published
types. Both receive no checker feedback.

The expression browser pass takes 32.64 seconds, followed by an 8.41-second
combined original-plus-expression replay. The helper pass takes 18.69 seconds,
followed by a 7.24-second combined replay. Original native results are reused
only after authenticating their frozen inputs; fresh consumers reuse none.
These timings describe research runs, not editor latency or cold native
analysis.

`snapshot-challenge-validation-v4.mjs` checks source/flow/label and declaration
pins, package bytes, profile stability, spans and independently recomputed
scores. It also checks that the *used detector* belongs to the population's
earlier freeze, so an adapted replay cannot be labeled a fresh frozen result.
A separate local-return audit resolves all five helper witnesses through the
actual TypeScript symbols and verifies their unique reference and source spans.

Retained outputs under `rust/target/` include:

- `snapshot-refinement-old-transfer-v6.json` and
  `snapshot-refinement-old-transfer-v7.json`: regression and correction.
- `snapshot-v7-detector-freeze.json`, `snapshot-expression-preflight-v1/`,
  `snapshot-expression-browser-v1/`, `snapshot-expression-study-v7.json`,
  `snapshot-expression-study-v8.json` and `snapshot-expression-study-v9.json`.
- `snapshot-v9-detector-freeze.json`, `snapshot-helper-preflight-v1/`,
  `snapshot-helper-browser-v1/` and `snapshot-helper-study-v9.json`.
- `snapshot-expression-validation-v7.json`,
  `snapshot-expression-validation-v9.json`, `snapshot-helper-validation-v9.json`,
  `snapshot-old-challenges-validation-v9.json` and
  `snapshot-local-return-audit-v9.json`.

All **112 prototype tests** pass, including seven new tests for retained value
paths, exact snapshot markers, call punctuation, local symbol edges and the
discarded-argument falsifier. All **195 prototype modules** pass syntax checks.
`make verify-fast` passes producer freshness, Rust formatting and pinned
workspace Clippy. Diff whitespace, schema and dialect-manifest checks pass.

Only versioned research modules and documentation change. Generated browser,
freeze and validation outputs remain research observations. No production
Rust rule, contract, finding snapshot or public manifest changes. Full
verification, coverage, ownership and certification gates are deferred for
this isolated prototype slice.
