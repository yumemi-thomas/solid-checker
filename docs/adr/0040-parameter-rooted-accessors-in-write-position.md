# ADR 0040: A parameter-rooted accessor is the caller's code in write position too

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: extends ADR 0034's disposition to the position it deferred. Leaves
  every other form exactly as ADR 0034 left it. Handshake protocol 23 → 24; the
  first slice of lever G in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

ADR 0034 stopped refusing a read accessor whose subject the producer roots at
an unwritten parameter of the censused declaration. Its argument was about
*whose code runs*:

> Code the caller attached to an object it passed is not this export's
> registration any more than a callback it passed is; if such a getter calls
> `createEffect`, that call is in the caller's file, where ordinary analysis
> already sees it with its own owner and timing.

It then admitted exactly two kinds **in read position**, and deferred the rest
with one parenthetical: *writes into a caller's object are a `writes`-domain
question this ADR does not open.*

That deferral is now the largest census refusal class on the corpus. After
ADR 0038 and ADR 0039 the accessor class is **404 refusals at 72 distinct
sites**, and the shape that dominates it is a write into an object the caller
handed over:

```js
function applyAxisDelta(axis, translate = 0, scale = 1, originPoint, boxScale) {
    axis.min = applyPointDelta(axis.min, translate, scale, originPoint, boxScale);
    axis.max = applyPointDelta(axis.max, translate, scale, originPoint, boxScale);
}
function copyAxisInto(axis, originAxis) {
    axis.min = originAxis.min;
}
```

The read of `axis.min` on the right-hand side is dispositioned; the write to
`axis.min` on the left is refused. Both touch the same property of the same
object, which the caller passed to this invocation, and the code that may run
in either case — a getter, a setter, a `Proxy` trap — was installed by the
caller and is analyzed in the caller's own artifact.

## Decision

**For the `creates` census, an accessor form whose subject is rooted at an
unwritten parameter of the censused declaration is dispositioned in write
position exactly as in read position, and the receipt says which of the two it
was.**

The premise is ADR 0034's, unchanged: the accessor is the caller's code. What
this ADR corrects is that the premise was stated as if it turned on *reading*,
when the argument it rests on turns on *provenance*. A setter the caller
installed on an object it passed is no more this export's registration than a
getter it installed, or a callback it passed.

### What the producer states

`accessorFormSubjectParameterLocked` now states a subject for three kinds
rather than two — `get-accessor`, `set-accessor`, and
`property-access-unknown-accessor` — on a property or element access node,
under exactly the root premises ADR 0034 already reviewed (a plain identifier
parameter binding, no initializer, no rest token, written nowhere in its file,
in a declaration mentioning neither `arguments` nor `eval`). Beside it the
producer states `subjectWrite`: true when the access is an assignment target, a
compound assignment, or an update expression.

`delete obj.p` stays unstated. It reaches a `deleteProperty` trap rather than
an accessor, and no ADR has reviewed that reach.

### Why the position is on the wire at all

Because it is one fact for one question and a different fact for another. For
`creates` the two positions are the same — the accessor is the caller's code
either way. For `writes` and `invalidates`, when those domains gain a census,
the assignment is **this export's own operation**, and whose accessor runs is
beside the point: such a census must refuse exactly the sites this field marks.
Folding the position into the subject would have left a later census unable to
tell the two apart, so the handshake moves (23 → 24) although the field is
additive: a consumer that cannot read the position must not read the subject.

### What the receipt says

A write-position site records the disposition
`parameter-rooted-accessor-write`, a read-position site
`parameter-rooted-accessor`, so a receipt states which premise each site held
under and the two are separable by a reader who cares about the difference.

## Alternatives considered

- **Leave the refusal to the `writes` domain.** This is what ADR 0034 said, and
  it conflates two questions. The `writes` domain will have to decide whether
  the assignment is an operation of this export; that decision is about the
  *assignment*, not about whose accessor runs, and it is not made easier by
  `creates` refusing here.
- **Admit the write but reuse the read disposition.** Rejected: the receipt
  would then not distinguish a site that a `writes` census must refuse from one
  it need not look at.
- **Admit `delete` too.** Rejected: a different trap, unreviewed.
- **Make the root premise flow-sensitive** so a reassigned alias qualifies.
  Rejected here for the same reason ADR 0034 rejected it: it needs a
  definite-assignment analysis and a new class of premise.
- **Treat a module-level object literal's members as data properties.** Not
  needed: the compiler already binds them, so no form is recorded. It is a
  different sub-class of lever G, over receivers the compiler leaves untyped.

## What still refuses

- **Any accessor whose subject is not parameter-rooted**, in either position —
  a module-level untyped receiver, a captured value, a call result, a nested
  callable's own parameter, or a parameter the declaration writes. The boundary
  is provenance, and it has not moved. Pinned by `setterOnModuleValue`.
- **`delete` on a parameter-rooted member.**
- **A spread, a destructuring element, an iteration, a coercion, `instanceof`
  and `await`** on a parameter-rooted operand: each has its own protocol reach,
  and ADR 0034's reasoning about them is unchanged. Together they are the rest
  of lever G — `BindingElement` and `SpreadAssignment` account for 67 of the
  404 accessor refusals and are not addressed here.
- Everything the `writes` domain will own: this ADR closes nothing there and
  marks, rather than hides, the sites it will have to decide.

## Consequences

- Handshake protocol 23 → 24; the schema digest and the Rust client move with
  it.
- `implementation-census-creates`: `setterOnParameter` flips to certifying,
  `updateOnParameter` is added for the compound-assignment shape, and
  `setterOnModuleValue` is added as the negative — its receiver must stay
  untyped, because a module-level object literal binds as a data property and
  records no form at all.
- Measured on the 2026-09-06 corpus: withheld candidates 805 → 745,
  `censusRefused` 748 → 688, and the accessor class 404 → 284 refusals. As
  after ADR 0038, clearing the first refusal in a body exposes the next: the
  coercion class rose 30 → 84 and the sites behind `iteration-protocol` and
  `instanceof` roughly doubled without their counts moving, because bodies
  that never got past their first write now reach those forms. Statuses
  unchanged, 368 certified / 30 refused. What is left of the accessor class is
  110 property reads, 101 element reads, 42 destructuring elements and 31
  spreads.
- Tests:
  `creates_census_dispositions_a_parameter_rooted_accessor_in_write_position`
  pins both positions, the resolved-setter kind, and the unrooted refusal;
  `creates_census_parameter_rooted_accessor_stops_at_its_stated_boundary` keeps
  every other kind refusing and records that `set-accessor` left its list here.
