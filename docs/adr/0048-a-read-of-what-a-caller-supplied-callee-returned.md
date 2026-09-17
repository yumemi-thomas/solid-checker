# ADR 0048: A read of what a caller-supplied callee returned

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: ADR 0042's `parameter-rooted-element` argument by a shorter route.
  Handshake protocol 31 → 32.

## Context

After ADR 0047 the largest single refusal left in the corpus is eighteen, and
it is this:

```js
function transformBoxPoints(point, transformPoint) {
    if (!transformPoint) return point;
    const topLeft = transformPoint({ x: point.left, y: point.top });
    return { top: topLeft.y, left: topLeft.x, … };
}
```

`transformPoint` is a parameter. `topLeft` is whatever the **caller's own
function** handed back, and reading `.y` on it may run a getter — one the caller
installed, on an object the caller's code produced.

ADR 0042 already decided the neighbouring case: a value the caller's *iterable*
yielded is the caller's, so calling it runs the caller's code
(`parameter-rooted-element`). A value the caller's *function* returned is the
same claim by a shorter route, and it had no derivation.

## Decision

**A form whose subject is the value of a call to a caller-supplied callee is
rooted at the slot that callee came from**, under the derivation
`parameter-result`.

The subject walk already follows a chain of property and element reads to its
innermost receiver; it now also stops at a **call**, and asks whether that
call's callee is rooted at a plain unwritten parameter. Because ADR 0043 roots a
local binding from an already-rooted initializer, `const topLeft = transform(p)`
followed by `topLeft.y` is covered by the same step — which is how compiled code
actually writes it, and why the fixture pins both spellings.

Only a callee rooted as a plain `parameter` qualifies. The result of a call to a
**defaulted** or otherwise derived callee is a second hop, and this build
reviews one.

## What still refuses

- **A call to a module-local helper**, whose result is this module's value and
  which nothing here speaks for. Pinned by `readLocalResult`. Closing that is
  a different premise — what the callee's own census can say about the value it
  returns, which for a *primitive* is ADR 0045's `primitiveCompletion` and for
  an object is unreviewed.
- **A call to a default-library member**, for the same reason: the
  standard-library disposition of the call says nothing about the value.
- **A call to a written or defaulted callee**, and any subject shape the walk
  does not follow.

## Consequences

- Handshake protocol 31 → 32; the schema digest and the Rust client move with
  it, and `subjectRoot`'s enum gains one spelling on the caller-provenance side
  — so it carries a `subjectParameter` and no declaration, like the other two.
- Measured on the 2026-09-07 corpus: withheld candidates 568 → 528,
  `censusRefused` 511 → 471, property reads 98 → 74 and element reads 52 → 34.
  **Forty candidates** from one derivation, because the shape it covers is how
  every compiled body threads a caller's callback result. Statuses unchanged,
  368 / 30.
- `implementation-census-creates` gains `readCallerResult` and
  `readBoundCallerResult`, which certify, and `readLocalResult`, which refuses.
- Tests: `the_probe_gate_tracer_census_closes_a_read_of_a_caller_supplied_result`
  runs all three end to end;
  `TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`
  pins both spellings and the derivation, and its companion-fact invariant now
  covers the third caller-provenance derivation.
