# ADR 0047: Whose `Symbol.hasInstance` an `instanceof` reaches

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`uncensused_invoking_forms.go`) and the policy-2
  `creates` census (`type_facts.rs`)
- Relation: the same provenance question ADRs 0034 and 0040–0044 answer, asked
  of the one operator none of them covered. Handshake protocol 30 → 31; the
  `instanceof` class of lever G in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

`instanceof` has been recorded and refused since the form vocabulary was
written, with the producer's own comment saying why:

> `x instanceof C` reaches `C[Symbol.hasInstance]` when C defines it. Whether it
> does is a property of the runtime value, so the form is always recorded.

That is right about the operator and stops one question short. What
InstanceofOperator does is `GetMethod(C, @@hasInstance)` and, when there is one,
call it; otherwise OrdinaryHasInstance reads `C.prototype` and walks `x`'s
prototype chain, which runs nothing at all. So the operator's **entire** reach
into user code is one method on the **right** operand — and asking whose it
could be is the same question this family has answered five times.

On the corpus it is 29 refusals at 15 sites, and every one of them has an
answer:

```js
if (elementOrSelector instanceof EventTarget) …          // the engine's
const scope = scopeOrUpdateDom instanceof Document ? … ; // the engine's
const ofClass = (v, c) => v instanceof c || …            // the caller's
```

## Decision

**An `instanceof` form states its constructor's provenance in the same
`subjectRoot` vocabulary the accessor forms use, over its right operand**, and
three answers are reviewed.

- **The caller's** (`parameter`, `parameter-default`). Under the root premises
  ADR 0034 established, whatever `Symbol.hasInstance` the constructor carries,
  the caller installed it on a value it passed — the identical argument that
  excuses a getter. Disposition `parameter-rooted-has-instance`.
- **The engine's** (`default-library`). Every declaration of the constructor's
  symbol is the default library's, so the method it would find is
  `Function.prototype[Symbol.hasInstance]`. Disposition
  `default-library-has-instance`.
- **This program's, and provably empty** (`own-class`). A class this artifact
  declares, with `subjectDeclaration` naming it. Disposition
  `own-class-has-instance`.

Only the parameter derivations carry a `subjectParameter`; only `own-literal`
and `own-class` carry a `subjectDeclaration`, and the consumer refuses either
fact beside a derivation that must not carry it.

### What makes the own class provably empty

`C[Symbol.hasInstance]` is an ordinary property lookup along **C's own
prototype chain**, which for a class means the superclass constructor. So:

- a **heritage clause** disqualifies the class outright — the chain then runs
  through a value this walk does not have;
- any **computed member name** disqualifies it, because `[Symbol.hasInstance]`
  is exactly how one is written and a computed key is not statically a name.
  The test is asked of every member rather than of the static ones alone, which
  costs a handful of classes and spares this premise a modifier test it would
  have to get exactly right;
- the binding must be written nowhere and declared once, as everywhere else in
  this family.

### Where the trust sits

The `own-class` premise is checked on both sides: the producer inspects the
class, and the consumer places its declaration in the artifact's own runtime
source. The `default-library` premise rests on the producer's **resolution** —
the same word the census already takes for a `standard-library` call
disposition, and it is worth naming rather than implying. What the consumer
checks there is the form's shape, and that neither companion fact accompanies a
derivation that must not carry one.

A **multi-declaration** global is admitted for the library arm and refused for
the others: the library declares `Error` twice, as an `interface` beside a
`var`, so the arm asks that *every* declaration be the library's. That is also
the stronger reading — a global the program augments is not purely the engine's.

## What still refuses

- **An imported constructor**, whose class this artifact does not declare.
- **A class with a superclass** and **a class with a computed member**, per the
  decision above. Pinned by `instanceOfDerivedClass` and
  `instanceOfComputedClass`.
- **A constructor read off anything** — a member access, a call result, a
  module value. Pinned by `instanceOfModuleValue`.
- `Symbol.hasInstance` patched onto a default-library constructor at runtime.
  That is the same global-environment assumption ADR 0042 makes about
  `Array.prototype[Symbol.iterator]` and the standard-library disposition makes
  about every reviewed member; this ADR does not widen it.

## Alternatives considered

- **A new form field for the constructor** instead of reusing `subjectRoot`.
  Rejected: the question is literally the same one — whose code can this form
  reach — and a second vocabulary for it would let the two drift.
- **Admit a class with a superclass this artifact also declares**, walking the
  chain. Sound and unmeasured: no corpus case needs it, and the walk would need
  its own cycle and depth rules.
- **Check `default-library` from the consumer side.** There is nothing to check
  against: `lib.*.d.ts` is not in the artifact snapshot, and the census has no
  TypeScript reader. Naming the trust is the honest alternative.

## Consequences

- Handshake protocol 30 → 31; the schema digest and the Rust client move with
  it, and `subjectRoot`'s enum gains two spellings.
- Measured on the 2026-09-07 corpus: withheld candidates 582 → 568,
  `censusRefused` 525 → 511, the `instanceof` class 29 → 9 at 15 → 6 sites.
  **Fourteen candidates**, and the class is now three quarters closed. The
  element-access class rose 46 → 52 as bodies that used to refuse at an
  `instanceof` reached their next form. Statuses unchanged, 368 / 30.
- `implementation-census-creates` gains six exports: `instanceOfParameter`,
  `instanceOfLibrary` and `instanceOfOwnClass` certify;
  `instanceOfDerivedClass`, `instanceOfComputedClass` and
  `instanceOfModuleValue` refuse.
- Tests: `the_probe_gate_tracer_census_closes_an_instance_of_by_its_constructor`
  and `…_stops_at_the_has_instance_boundary` run all six end to end;
  `TestUncensusedFormSubjectParameterIsStatedOnlyUnderTheParameterRootPremises`
  gains the three derivations and now asserts, for every stated subject, that
  it carries exactly the companion fact its derivation is allowed.
