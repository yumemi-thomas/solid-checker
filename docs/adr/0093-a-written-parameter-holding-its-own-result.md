# ADR 0093: A written parameter holding a value this program allocated

- Status: accepted and implemented (2026-09-12); written with the
  implementation
- Date: 2026-09-12
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the `creates`
  implementation census (`contract_certification/type_facts.rs`)
- Relation: ADR 0091's named open shape, closed with ADR 0090's structure and
  ADR 0044's premise. Handshake protocol 52 → 53.

## Context

ADR 0091 roots a written parameter when **every** value assigned to it is
rooted at that same slot, and recorded what that leaves refusing:

> 26 of the 30 have a value the join cannot call the caller's, and they are one
> shape: `b = localHelper(b)`. The premise for that value exists
> (`localLiteralResultLocked`) and cannot reach here, because
> `LocalLiteralResultPremise` names a single call while the binding has two
> provenances — the caller's argument and the call's result.

The shape is the commonest one in compiled output. `@corvu/utils`, published
verbatim by several packages, is the canonical instance:

```js
function stringStyleToObject(style) {
  const object = {};
  let match;
  while (match = extractCSSregex.exec(style)) { object[match[1]] = match[2]; }
  return object;
}

function combineStyle(a, b) {
  if (typeof b === "string") { b = stringStyleToObject(b); }
  return { ...a, ...b };            // reads own properties of b
}
```

Measured across the 418-probe corpus, `written-parameter` is the largest
premise-shaped refusal leg left in the census: **33 distinct claims across 15
packages**, of which 26 are the `property-access-unknown-accessor` family.

## Decision

A written parameter roots at its own slot under the new derivation
**`parameter-or-own-result`** when every value assigned to it is either

- rooted at that same slot — ADR 0091's condition, unchanged; or
- the result of a call carrying a **local-literal-result premise**: a call whose
  every normal completion returns the same unwritten, data-only literal binding
  this program's own code allocated (ADR 0044's machinery,
  `localLiteralResultLocked`).

The binding then holds one of two values, and an own-property read is excused on
each by a derivation already reviewed — `parameter` on the caller's argument
(ADR 0034/0041), and ADR 0044's own-literal argument on the allocated one,
reached through a call rather than named directly. The arms are **exhaustive**
because they are the binding's enumerated sources: `bindingValueSourcesLocked`
refuses the whole binding rather than skipping a write it cannot read a single
value out of.

That is exactly ADR 0090's structure, and the spelling is apart from
`parameter` for exactly ADR 0090's reason: only one arm is the caller's, and a
consumer reading this derivation as `parameter` would say the caller installed
whatever ran.

`subjectLocalLiteralResults` carries one premise per allocated source, and the
census binds **every** one of them to a call row of the same transcript — the
same work ADR 0044 does for its single premise — before admitting the
derivation. The form therefore defers past the call walk, as ADR 0044's does.

## What refuses, and why each is a refusal rather than a skip

- **A helper that hands back its own argument** (`passThrough(b)`). Nothing was
  allocated, so no premise exists. The value is arguably still the caller's, but
  that is a *different* claim — about a callee propagating provenance — and this
  ADR does not make it.
- **A helper that allocates a literal carrying an accessor.** This is the case
  that would be unsound if admitted: reading an own property of the result runs
  a getter this program wrote. `localLiteralResultLocked` requires a data-only
  literal and states no premise, so the binding refuses. Pinned by
  `writtenParameterAccessorResult`.
- **A premise that does not bind** to a call row of this transcript, to a callee
  in the artifact's own runtime source, or whose allocation or returns lie
  outside the callee it names. The refusal says so in its own words rather than
  calling the derivation unreviewed.
- **Any non-accessor form kind.** An iteration protocol or an `instanceof` over
  such a binding refuses, exactly as under ADR 0090: the two-arm argument has
  been made here for property reads alone.
- **A derivation with no premises at all.** A `parameter-or-own-result` with an
  empty list is the producer's two walks disagreeing, and the consumer refuses
  rather than reading it as the stronger `parameter`.

## Consequences

`writtenParameterOwnResult` certifies in
`fixtures/package-contracts/implementation-census-creates`, with
`writtenParameterPassthroughResult` and `writtenParameterAccessorResult` beside
it as the two refusals above.

**On the corpus this closes nothing: 5,275 certified closures before and after,
11,552 withheld before and after, no row moved.** The premise fires — eleven
`written-parameter` refusal sites disappear, including every copy of
`@corvu/utils::combineStyle`, the instance this ADR was written from — and each
export it unblocks is still withheld one level deeper, inside the very helper
the premise excuses. In `stringStyleToObject` the remaining blocker is
`match[1]`: `match` is a `let` with no initializer, written once per loop
iteration from `extractCSSregex.exec(style)`, and a written local binding
refuses at `local-binding-written` before anything asks what it holds. It is
this ADR's own argument one level down, with `undefined` and a standard-library
call result as its two arms.

The 33 claims § 75 counted were the refusal the census reported *first*; they
did not separate the two levels. What this ADR bought is a strictly narrower
frontier, a refusal that points inside the helper rather than at the parameter,
and a next leg with eight withheld `creates` exports behind it — an upper bound,
for the reason this paragraph just demonstrated. § 76 of
`phase21/2026-09-10-reads-veto-observation-design.md` carries the table and the
eight.

What this does not reach: a callee that propagates its caller's provenance
(the pass-through shape), the `ambient-declaration` (26 claims) and
`nested-parameter` (22) legs beside it, and any form kind but the accessor pair.

It also does not reach the **`reads` census**, and that is structural rather
than an omission. That census walks the export's own implementation and recurses
into nothing, so it has no call walk to bind the premises against; a form
carrying this derivation refuses there exactly as it did before, only with a
better sentence. Reaching it would mean giving the `reads` census a call walk,
which is its own decision.
