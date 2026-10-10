# ADR 0049: An omitted argument slot receives `undefined`

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`declared_signature_premise.go`)
- Relation: repairs the place ADR 0038's helper premise loses its grip, which
  ADR 0046 exposed by fixing the other one. Handshake protocol 32 → 33.

## Context

After ADR 0048 the motion-dom geometry cluster is *still* refusing, and in a
place that should be impossible: inside `scalePoint`, whose own declared
signature types all three parameters `number`.

```js
function scalePoint(point, scale, originPoint) {
    const distanceFromOrigin = point - originPoint;   // refuses
    const scaled = scale * distanceFromOrigin;        // refuses
```

The chain explains it. `applyBoxDelta` is the root, premised by its declared
signature; it calls `applyAxisDelta`, which calls `applyPointDelta`, which
calls `scalePoint` — twice:

```js
function applyPointDelta(point, translate, scale, originPoint, boxScale) {
    if (boxScale !== undefined) {
        point = scalePoint(point, boxScale, originPoint);
    }
    return scalePoint(point, scale, originPoint) + translate;
}
```

`applyAxisDelta` calls `applyPointDelta` with **four** arguments, so slot 4,
`boxScale`, carries no premise and stays `any` on the twin. It is then handed
to `scalePoint`'s `scale` slot, so *that* premise is `any` too, and
`scale * distanceFromOrigin` records a coercion under a premise set the census
demands a second transcript for. One omitted optional argument, four levels of
premise lost.

## Decision

**A `callArgumentPremise` covers the argument slots the call does not write, as
`undefined`.**

A call with fewer arguments than the callee has parameters leaves the rest
`undefined` — that is the specification, not an inference — and `undefined` is
a **primitive**, so it is a strictly stronger premise than the `any` an
unannotated JavaScript parameter would otherwise carry. It is also a *semantic*
fact rather than a type one, which is why it is available exactly where the
declarations are not.

### Three slots it will not speak for

- A parameter with an **initializer**: the value is then the default, not
  `undefined`.
- A **rest** parameter, and any binding that is not a plain identifier: there
  is no single slot to describe.
- A parameter the callee's **own file writes**: `bias = 1` before the body
  reads it means the premise would describe a value that is already gone, and
  a type of `undefined` would classify that read wrongly.

A skipped slot leaves the list strictly increasing, which is what the JSDoc
annotation and the consumer's echo both require.

### The identity

`undefined` is an intrinsic type with no symbol and no alias, so its
`typeDeclarationIdentity` is its flags alone. The caller's twin has no
expression of that type to ask the checker about, so the identity is built from
the flag constant — and `TestOmittedArgumentSlotsArePremisedAsUndefined` pins
that the two agree by demanding the premise back and requiring the helper's
twin to bind it. A drift between the constant and the checker's own type would
fail there rather than silently refusing every premise.

## Alternatives considered

- **Premise the helper by its own declared signature** when it is also an
  export. Stronger, and much larger: the census reaches these declarations *by
  location*, not by export name, so the verifier would need that export's
  declared signature to bind the premise against — which it does not hold. Left
  as the next step for this chain if one is needed.
- **Record the callee's declared parameter type for the omitted slot.** That is
  a type fact and needs the declarations the JavaScript module cannot see; the
  point of this ADR is that the semantic fact needs nothing.
- **Treat the omitted slot as `undefined` on the consumer's side** rather than
  stating it. Rejected: it would be the consumer inventing a premise, and every
  premise in this family is stated by the producer and echoed back.

## What it does not fix, measured

Measured on the 2026-09-07 corpus: **no movement at all** — withheld 528, every
class count identical. The premise is produced, and
`TestAnOmittedSlotSurvivesTheHelperPremiseChain` pins it surviving two hops of
the exact motion-dom chain above. What still blocks that cluster is one line
further in:

```js
point = scalePoint(point, boxScale, originPoint);   // `point` is now `any`
return scalePoint(point, scale, originPoint) + translate;
```

`applyPointDelta` **assigns** its own `point` parameter from a call to an
unannotated helper, so the argument type at the second call's slot 0 is `any`,
that slot carries no premise, and `point - originPoint` records a coercion
inside `scalePoint` under a premise set that leaves slot 0 open. Closing it
needs the caller's twin to know that `scalePoint` returns a number — the
"annotate the callee in the caller's twin" alternative ADR 0045 rejected as too
large, and the honest next step for this cluster.

Keeping this ADR without its measured gain is deliberate: the premise is a
strictly stronger fact, it is what every deeper hop will rest on once that
blocker moves, and a chain that silently degrades an optional argument to `any`
is a defect whether or not any row happens to depend on it today.

## Consequences

- Statuses unchanged, 368 certified / 30 refused; withheld unchanged at 528.
- Handshake protocol 32 → 33; the schema digest and the Rust client move with
  it. `callArgumentPremises` changes meaning — it covered the written slots and
  now covers the unwritten ones too — so a protocol-32 consumer's echo would
  differ.
- Tests: `TestAnOmittedSlotSurvivesTheHelperPremiseChain` adds motion-dom's
  own `applyBoxDelta` → `applyAxisDelta` → `applyPointDelta` → `scalePoint`
  chain to the premise fixture and pins the omitted slot surviving two hops;
  `TestOmittedArgumentSlotsArePremisedAsUndefined` adds
  `omitsTrailingArgument`/`scaleWithin` and `omitsWrittenArgument`/`widenWithin`
  to the premise fixture, pins the stated slot, pins that demanding it back
  binds and clears the helper's coercions, and pins that a parameter the callee
  writes is skipped.
