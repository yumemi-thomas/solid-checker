# ADR 0050: A written binding whose every value is rooted

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`uncensused_invoking_forms.go`,
  `invocation_transcripts.go`)
- Relation: takes the case ADRs 0034, 0040 and 0043 each refused and each gave
  the same reason for. Handshake protocol 33 → 34.

## Context

Every root premise in this family requires the binding to be **written
nowhere**, and each ADR that met the restriction gave the same justification:

> Make the root premise flow-sensitive so a reassigned alias qualifies.
> Rejected: it needs a definite-assignment analysis and a new class of premise.

That is true of a flow-*sensitive* reading and false of the question actually
being asked. On the corpus the shape is this, and it is 27 refusals in
`@corvu/utils` alone:

```js
var contains = (wrapper, target) => {
  let currentElement = target;
  while (currentElement) {
    if (currentElement === wrapper) return true;
    currentElement = currentElement._$host ?? currentElement.parentElement;
  }
};
```

`currentElement` holds the caller's argument, or a property of the caller's
argument, or a property of that — and nothing else, ever.

## Decision

**A binding the file writes is rooted at a parameter when every value it can
hold is rooted at that parameter.**

No flow analysis is involved and none is claimed. If *every* value the binding
can hold is the caller's, then whichever one it holds at the read is the
caller's, and which branch assigned it never comes up. That is a join over the
binding's sources, not a walk over its control flow.

The sources are the declaration's own initializer and the right-hand side of
every plain assignment to it. A reference to the binding **itself** is admitted
co-inductively, which is the chain rule ADR 0034 already applies to `a.b.c`
written as a loop; the least fixpoint is anchored by requiring at least one
source that is not the binding. A declaration with **no** initializer
contributes no source at all: the value before the first assignment is
`undefined`, and a member read of that throws before any lookup.

### The same join written as an expression

A conditional and the short-circuit operators hand back one of their arms, so
the subject walk follows them the same way: if every arm is rooted at the same
slot, the result is. `currentElement._$host ?? currentElement.parentElement` is
the assignment source above, and without this the binding's join had nothing to
stand on.

Arms rooted at **different** slots refuse for the reason the assignment
spelling does — `sourceBox ?? box` is the caller's either way and the receipt
names one slot. Naming a slot that would not be true is worse than refusing,
and a derivation that names no slot is a separate premise nobody has reviewed.

### What refuses the whole binding

A **compound** assignment yields a coercion result, an **update** a number this
walk has no expression for, a **destructuring** target a property of something
else, and a `for…of`/`for…in` head an element of an iteration. Each refuses the
binding outright rather than being skipped, because a source left out would
make the join a claim about only some of the values.

Sources rooted at **two different slots** also refuse. The value is the
caller's either way, but the receipt names one slot, and naming either would
say the caller passed something it did not.

## A soundness hole this found, and closed

Writing the negative fixture for the destructuring case exposed a defect older
than this ADR. The write scan skipped **declaration names** before asking
whether an identifier was an assignment target, and the target of
`({ current } = other)` is a `ShorthandPropertyAssignment`'s name — a
declaration name by the compiler's reckoning and a write by the language's. So
such a binding was reported **unwritten**, and every premise resting on
"written nowhere" — ADR 0034's root set included — silently covered it.

Two things were wrong and both are fixed:

- the declaration-name filter is asked *after* the assignment test, and is in
  fact redundant, since an ordinary declaration name is never an assignment
  target;
- `GetSymbolAtLocation` answers the object literal's **property** symbol for a
  shorthand, not the variable, so the scan matched nothing even once it looked.
  The checker has a dedicated resolution for that shape
  (`GetShorthandAssignmentValueSymbol`); the shim now exposes it and the scan
  uses it.

This is a widening of what counts as written, so it can only *remove*
dispositions — the safe direction — and `writtenByDestructuring` pins it.

## Alternatives considered

- **Definite-assignment analysis**, as the earlier ADRs described. Still not
  needed, and still the wrong shape: the question is what the binding can hold,
  not which assignment reached the read.
- **Admit sources rooted at different slots**, naming one. Rejected: the
  receipt would be false about the slot.
- **Admit a written parameter** the same way. Its sources are the caller's
  argument plus each assignment, so it is the same join — but the corpus case
  (`b = stringStyleToObject(b)`) has a source that is a local helper's result,
  which nothing here roots, so it would buy nothing today and is left unstated.

## Consequences

- Measured on the 2026-09-07 corpus: withheld candidates 528 → 506,
  `censusRefused` 471 → 449, property reads 74 → 46. **Twenty-two candidates**,
  and the whole of `@corvu/utils`'s `contains` loop among them. A new
  `PrefixUnaryExpression` coercion class of six appears, which is the usual
  unhiding. Statuses unchanged, 368 / 30.
- Handshake protocol 33 → 34; the `parameter` derivation widens to a written
  binding, so a consumer that reviewed "an unwritten binding" now receives one
  written in a way it has not seen.
- `implementation-census-creates` gains six exports: `writtenJoin` (the loop
  shape), `writtenFromUninitialized` and `joinedArms` certify; `writtenFromModuleValue`,
  `writtenFromTwoSlots` and `writtenByDestructuring` refuse.
- Tests:
  `the_probe_gate_tracer_census_closes_a_written_binding_whose_values_are_rooted`
  runs all five end to end, and
  `TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`
  pins each source shape at the producer.
