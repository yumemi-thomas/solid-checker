# ADR 0046: A helper premise spelled as an import type

- Status: accepted and implemented (2026-09-07); written with the
  implementation
- Date: 2026-09-07
- Owners: Type Facts producer (`declared_signature_premise.go`)
- Relation: closes the open item ADR 0038's helper amendment recorded and
  ADR 0045's measurement re-found as the largest single blocker in the corpus.
  Handshake protocol 29 → 30; lever C in
  `docs/package-contract-v2/accuracy-roadmap.md`.

## Context

ADR 0045 measured twelve candidates where a hundred looked possible, and the
shortfall said exactly why. `applyPointDelta` cleared as a **root** export and
refused as a **helper**, and so did the whole `motion-dom` geometry cluster
below it. The plainest case:

```js
function calcLength(axis) { return axis.max - axis.min; }
```

`calcLength` is reached from `calcAxisDelta(delta, source, target, origin)`,
whose declared signature types `source` as `Axis`. The caller's premised census
records `Axis` as the argument type at `calcLength(source)`, the verifier
demands `calcLength`'s census under exactly that, and the producer spells it:

```js
/** @param {Axis} axis */
function calcLength(axis) { … }
```

**`Axis` resolves to nothing there.** `dist/es/projection/geometry/delta-calc.mjs`
is a JavaScript module with no reference to the `.d.ts` that declares `Axis`;
the compiler prints an unresolved type reference by the name it was written
under, so the twin's falsifier sees `Axis` where it expected `Axis`, catches
the difference on the flags and the missing declaration identity — which is
what `typeDeclarationIdentity` exists for — and refuses. The helper is then
censused over `any`, and `axis.max - axis.min` refuses for a reason that is
about spelling rather than about the code.

The root premise already solved this for itself: it falls back to
`@type {typeof import("./pkg/index").clamp}`, which names the declaration
module directly. The helper had no such fallback.

## Decision

**A parameter premise may carry a `spelling`: a form of its type that resolves
from a module which cannot name it directly, `import("<specifier>").<Name>`.**

The caller's twin is where it can be computed, because that is the program in
which the name resolves. When an argument type's symbol — its **alias** first,
since that is what the printer prefers, and its own symbol otherwise — is
declared in a declaration file that the module specifier rule can name, and
that file exports it under an identifier name, the producer records the import
form beside the printed text. `demandedPremiseAnnotationLocked` then writes the
spelling into the helper's JSDoc instead of the bare name.

### It is a hint, and never the premise

The falsifier does not move. The twin's own printed type must still equal the
premise's `type` and its declaration identity must still equal the premise's
`identity`, so a spelling that resolved to a *different* type refuses the twin
exactly as a wrong printed text does — pinned by the test that forges
`).EasingFunction` in place of `).Axis` and gets a refusal. A consumer echoes
the spelling byte for byte and interprets none of it; the established premise
carries it back so the verifier's entry-for-entry comparison still holds.

A **root** premise carries neither identity nor spelling, exactly as before: it
is bound to the export's declared signature by its printed text alone, and
nothing echoes it.

## What this does not change

- **Which types are spellable.** A structural type, an anonymous literal, a
  generic instantiation and a type declared in a JavaScript file all answer no
  spelling, and the premise falls back to the printed text as before.
- **The comment-safety rule**, except to make it correct. It rejected any `/`,
  which was harmless for a printed type text and fatal for a spelling whose
  specifier is a path full of them. It now rejects the comment terminator
  `*/` and a line break, which is what it always meant.
- **The strength of a helper premise.** It was already a condition the verifier
  binds at every hop; this only stops the producer refusing to state one for a
  reason that has nothing to do with the code.

## Alternatives considered

- **Have the helper's twin import the declaration module** and use the bare
  name. Rejected: it adds a statement rather than a comment, changing what the
  module *is* rather than what the checker knows about it, and the twin
  machinery exists precisely to keep the bytes that run untouched but for one
  comment.
- **Spell the type structurally** — print `{ min: number; max: number }`
  instead of `Axis`. Rejected: the printed text is the falsifier, so a
  structural spelling would have to change what the premise *says*, and two
  distinct interfaces with the same shape would then compare equal.
- **Give up the identity check for named types.** Rejected outright: the
  identity is the only thing that separates a resolved `Axis` from an
  unresolved reference printed as `Axis`, which is the exact failure this ADR
  is about.

## Consequences

- Handshake protocol 29 → 30; the schema digest and the Rust client move with
  it.
- Measured on the 2026-09-07 corpus: withheld candidates 619 → 582,
  `censusRefused` 562 → 525, the coercion class 95 → 59. **Thirty-seven
  candidates**, the largest gain since ADR 0042, and the whole of it is helper
  bodies that were already correct and unreachable. Statuses unchanged,
  368 certified / 30 refused.
- What is left of the coercion class is a different question in each case, and
  none of them is spelling: an operand bound to a **call result** the premise
  does not follow (`relativeProgress - sourceAxis.min`), a helper reached at a
  depth the premise chain has not carried (`calcBezier(…) - x`), an
  **element access** on a parameter whose declared element type is not a
  primitive (`keyframes[i] + "px"`), a **template** substitution, and a
  compound assignment to a defaulted parameter (`originPoint -= translate`).
- Tests: `TestAHelperPremiseIsSpelledAsAnImportTypeWhenItsNameIsForeign` adds
  `viaAxisHelper`/`lengthOf` to the premise fixture and pins all of it — the
  recorded spelling, the coercion clearing under it, the two parameter-rooted
  reads that correctly remain, the primitive completion the helper now states,
  and the refusal when the spelling names another type.
