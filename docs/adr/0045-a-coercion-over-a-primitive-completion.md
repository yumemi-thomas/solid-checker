# ADR 0045: A coercion over a completion the callee proved primitive

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`uncensused_invoking_forms.go`,
  `declared_signature_premise.go`) and the policy-2 `creates` census
  (`type_facts.rs`)
- Relation: completes what ADR 0038 and its helper-premise amendment began —
  the premise reached a helper's parameters but never came back out. Handshake
  protocol 28 → 29; the coercion half of lever G in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

After ADR 0044 the coercion class is the largest census refusal at 107, and it
is concentrated in one place: seventeen exports of `motion-dom`'s projection
geometry, six artifact cases each. Their declarations are not the problem —
the package types them precisely:

```ts
declare function applyPointDelta(point: number, translate: number, scale: number,
                                 originPoint: number, boxScale?: number): number;
declare function scalePoint(point: number, scale: number, originPoint: number): number;
```

and ADR 0038 already classifies the body under that signature. What refuses is
this:

```js
function applyPointDelta(point, translate, scale, originPoint, boxScale) {
    return scalePoint(point, scale, originPoint) + translate;
}
```

`translate` is a `number` under the premise. `scalePoint(…)` is **`any`** —
because `scalePoint` is a module-local function in compiled JavaScript, and
nothing in that JavaScript module refers to the `.d.ts` that types it. So the
sum may coerce an object, and the form is recorded.

**The premise flows in and never flows back.** The helper amendment carries the
caller's argument types to `scalePoint`'s own census, which clears its body;
the one thing it does not carry is what `scalePoint` hands back.

## Decision

**A coercion is dispositioned when every operand it applies ToPrimitive to is
provably a primitive or the result of a call whose callee's own census proved
its completion primitive.** Two halves, stated by two different transcripts,
and the census grants nothing from either alone.

### What the form states

`coercionPremise` on a `coercion` form lists the **calls** the clearance would
rest on. Every other operand is provably a primitive by its own type and is not
listed, because a primitive has no `Symbol.toPrimitive`, `valueOf` or
`toString` for a coercion to reach — that is the same test the classifier
already applies to decide whether to record the form at all.

Either every operand is covered or nothing is stated. A premise naming some
operands and leaving others unexplained would read as a claim about the whole
form.

Four shapes carry a value without changing where it came from, and each is
followed: a conditional and the short-circuit operators, where every arm must
qualify; and a reference to a local binding the file declares once, writes
nowhere and initializes — naming an intermediate does not change whose value it
is, exactly as in ADR 0043.

### What the callee states

`primitiveCompletion` on an implementation transcript says the checker's return
type for that declaration — **on the very program the census was classified
over**, the premise twin included — is a union of primitive types alone. The
completion form needs no separate test: an async function's return type is a
`Promise` and a generator's a `Generator`, and neither is a primitive.

It is a fact about the census's *premise*, not about the declaration. The same
helper censused under two argument premises may state it under one and not the
other, so a consumer may read it only from the transcript it demanded under the
premise it recorded.

### How the census grants it

The two halves meet on evidence the census already holds. A coercion's operand
call is also a row of the same transcript's `calls`, dispositioned
`local-recursion`, so the callee's transcript has already been demanded — under
that row's own argument premise. Granting the premise is then: match each named
call to its row, bind that row's callee to a declaration in the artifact's own
runtime source exactly as the local-recursion disposition does, and read one
boolean from the transcript that row produced. No new evidence, no new demand,
and nothing re-derived.

The deferred forms are decided **after** the call walk, and only the coercions
that state a premise are deferred, so no refusal this census already made moves
behind a walk it used to precede.

## Where the trust sits, stated plainly

The type is the compiler's answer, on a program whose parameter types the
consumer bound to the export's declared signature (`census_root_premises`) or
carried from a call it verified. What this ADR adds is not a new oracle but a
**binding**: the caller's classification and the callee's classification are
made to rest on the same stated condition, and the receipt names both. That is
the strength the helper premise already had, extended to the value coming back.

It is weaker than the ADR 0044 style of premise, where the specification
settles the question outright, and the ADR does not pretend otherwise. What
keeps it honest is that a callee whose body the census could not clear under
that premise states no primitive completion, so the caller refuses too.

### One gate had to move with it

A local declaration's premise was applied only when it could clear a **form** —
its own, or one in a helper it reaches. The completion is a second answer the
consumer now reads, and a body like `scaleBy(value) { return value }` records
no form at all, so no twin was built and it answered `any`: not a primitive,
and the caller refused. The gate now also builds the twin when the declaration's
completion is **not already** a primitive over its own types, which is exactly
the case a demanded premise exists to settle. A declaration whose completion is
primitive either way still pays nothing.

## What still refuses

- **An operand from a call into the default library.** `JSON.parse("1") + base`
  refuses: the standard-library disposition of the *call* says nothing about
  the value it returns, and no ADR has reviewed a table of library members by
  return type. Pinned by `coerceLibraryResult`.
- **An operand from a callee the census cannot bind** — a written binding, a
  binding initialized by anything but a function literal, a parameter of a
  nested callable. The premise is never granted for a callee whose body the
  census does not have. Pinned by `coerceWrittenHelperResult`.
- **A callee whose completion is an object**, which is the whole point of
  asking. Pinned by `coerceObjectHelperResult`.
- **A written local** between the call and the coercion, and any operand shape
  outside the four followed above.
- **A coercion of a parameter-rooted value.** ADR 0034 listed coercion among
  the forms it did not review and this ADR does not review it either: the
  caller's `valueOf` is the caller's code by the same argument that excuses a
  getter, but that is a different premise and belongs in its own slice.

## Alternatives considered

- **Annotate the callee in the caller's own twin** so the checker infers the
  return there. Rejected: it needs a second twin per premised export and, for
  a callee in another module, a multi-file twin — and the fact it would produce
  is the *same* producer agreeing with itself, with no transcript to bind it
  to. Asking the callee is both cheaper and better evidence.
- **A table of default-library members by return type**, closing
  `JSON.parse`-shaped operands. Reviewable and worth doing, but it is a
  membership review like the invoker table's and deserves its own ADR.
- **Read the callee's declared signature from the artifact's `.d.ts`.** The
  strongest evidence available, and unreachable here: the verifier holds the
  demanded export's declared signature, not every export's, and nothing in
  this side parses TypeScript declarations.

## Consequences

- Handshake protocol 28 → 29; the schema digest and the Rust client move with
  it.
- Measured on the 2026-09-07 corpus: withheld candidates 631 → 619,
  `censusRefused` 574 → 562, the coercion class 107 → 95. **Twelve
  candidates**, and the shortfall is the useful part of the measurement: it
  names exactly one blocker, which is not this ADR's.

  `applyPointDelta` clears where it is the **root** export, whose premise comes
  from its own declared signature, and still refuses where it is reached as a
  **helper** — because the argument type at the reaching call is `Axis`, and
  `demandedPremiseAnnotationLocked` spells a helper premise as
  `@param {Axis} axis` inside a JavaScript module that cannot resolve that
  name. The twin's falsifier then refuses, correctly, and the helper is
  censused over `any`. `calcLength`'s `axis.max - axis.min` is the same thing
  at its plainest, and it rose 6 → 24 as bodies that used to refuse earlier
  reached it. That is lever C's own named open item — spell such a type as
  `typeof import("…").Axis` from the identity's declaration file — and it is
  the next slice, not a defect here.
- `implementation-census-creates` gains six exports: `coerceHelperResult`,
  `coerceBoundHelperResult` and `coerceConditionalHelperResult` certify;
  `coerceObjectHelperResult`, `coerceWrittenHelperResult` and
  `coerceLibraryResult` refuse.
- **The vacuity trap, a third time.** `coerceLibraryResult` first read
  `Math.min(base, 1) + base` and certified without the census reaching the
  premise: `Math.min` is declared to return `number`, so the operand is already
  a primitive and no form is recorded. Any negative in this class has to use a
  library member the declarations type as `any`. ADRs 0043 and 0044 each walked
  into the same trap in their own way; the fixture README now names all three.
- Tests: `creates_census_grants_a_coercion_only_from_the_callees_own_completion`
  pins that neither half clears alone — an unstated premise, a premise resting
  on nothing, a premise naming no row, and a callee whose completion is not a
  primitive all refuse;
  `TestCoercionPremiseNamesItsOperandCallsAndCompletionsFollowThePremise` pins
  the producer's two halves, including that a *stated* premise over an
  object-returning callee is not a verdict;
  `the_probe_gate_tracer_census_closes_a_coercion_over_a_primitive_completion`
  and `…_stops_at_the_primitive_completions_boundary` run all six end to end.
