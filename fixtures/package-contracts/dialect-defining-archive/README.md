# The generator does not invent operations inside the dialect's own archives

Two fixtures, one claim, and the *only* difference between them is
`package.json`'s `name`.

Both sit under a `@solidjs` path component and both declare
`createTrackedEffect` locally, with no import. That is deliberate: primitive
identity in `solid-reactive-ir` is granted by declaration **path**
(`declaration_path_is_solid_package`, `symbols.rs`), which is correct for a
consumer — TypeScript resolved the symbol into the package, so the path *is* the
import edge — and wrong inside a dialect's own archive, where the package's own
`createSignal` / `createTrackedEffect` / `onCleanup` become "primitive calls".
`find_missing_owners` then pushes an owner requirement for a call in the
primitive's *own implementation*, and `owner_requirement_operation`
(`inferred_contract.rs`) decides what, if anything, is published for it.

**[2026-09-03] Where the difference now shows.** Both fixtures publish no
`creates` operation, because the `effect` role has no home in schema version 1
and is withheld for *every* package (`semantic-model.md` § creates). The pair's
claim is intact — the two answers are still different — but the distinguishing
artifact moved from the document to the generator's refusal sidecar, which
names each withholding:

| fixture | `package.json` name | `creates` | `withheldClaims` |
| --- | --- | --- | --- |
| `@solidjs/signals` | `@solidjs/signals` | **absent (open)** | **empty** — the scope decision refuses the whole derivation, so there is no role to withhold |
| `@solidjs/router-shaped` | `@solidjs/router` | **absent (open)** | `onSettled`, role `effect` — the derivation ran and its one role has no domain |

`@solidjs/router` is the control: it lives under the same `@solidjs` scope and
receives the same path bootstrap, but it is a *consumer* of the dialect's
packages, not one of them, so the derivation runs for it and its result is
recorded by name.

## Why the positive case is `open` and not `creates: []`

`creates: []` *closed* is a negative claim, and the generator has no evidence
for it — "no requirement was derived" is missing knowledge, not proof of
absence. Only a hand audit asserts that closure, and the one that does is
already checked in: `pkg/contracts/bundled/solid-v2/solidjs-signals.json` closes
`creates` for the real `onSettled` and models its leaf owner as a
`resources` entry instead. The real bytes branch on an ambient owner rather than
requiring one, so the audited answer is also the correct one; the generated
`requires: required` was `find_missing_owners`' owner lattice not being
guard-sensitive.

So the fix withholds, never asserts. Contract generation opens the
owner-requirement and reactive-read domains for these archives, which is the
weakest thing the model can say. Since 2026-09-03 the *consumer* side withholds
too, for an unrelated reason — a free-standing owner requirement has no domain
in schema version 1 — so read this fixture pair's answer off `withheldClaims`
rather than off `creates`.

## The predicate is a name, and that is on purpose

`solid_dialect::primitive_defining_package` compares an **exact package name**
with no version and no integrity behind it, so it is not a proof of identity.
It is admissible only because it acts in exactly one direction: it removes
claims. A name match can therefore never *grant* anything — see
`docs/adr/0005-dialect-axioms-about-the-dialects-own-package.md`, where the
axiom that would have granted an owner discharge is refused for precisely the
missing-integrity reason.

The fabricated `0.0.0-fixture` versions make that visible: no audited tuple
exists for these bytes, and the scope decision still applies, because it does
not depend on one.

## Traps

ADR 0017 extends the same scope decision to callback proposals. The existing
`onSettled` pair now also pins an explicit unknown callback domain for signals
and the unchanged callback operation for router-shaped. This does not certify
callback absence. Since summaries lose bootstrap provenance, independent
direct callbacks in primitive-defining packages are also withheld until that
provenance or an independent execution proof is available.

- Both fixtures **must** stay under a directory component named `@solidjs`.
  Move either one to a flat `fixtures/package-contracts/<name>/` and the path
  bootstrap stops firing, no primitive is recognized, the control stops emitting
  its owner requirement, and the pair silently proves nothing.
- Neither fixture ships a `node_modules/solid-js` stub: `createTrackedEffect` is
  2.0 vocabulary and 2.0 is the default dialect. Adding a 1.x stub above them
  would select the v1 catalog, which does not declare that primitive, and again
  the pair would prove nothing.
