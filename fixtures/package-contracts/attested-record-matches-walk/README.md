# The attested closure record equals what the walk found, byte for byte

The negative case for attestation: a package where the generator's syntax walk
and the analyzing program agree completely, so nothing about the record may move
just because its *source* changed.

`index.js` writes `export { thing } from "./impl.js"` and the checkout ships
`impl.ts`. Both sides resolve that: the walk substitutes `.js` → `.ts` from its
own extension table, and TypeScript resolves the ESM-spelled specifier against
the source that exists. So `expected-generation.json` names `impl.ts` and
`index.js`, in that order, with no notes and no `runtimeNotes` — and it must keep
naming exactly those two after any change to the reconciliation in
`attestedClosure`.

**What this pins that the other four cannot.** Every other closure fixture here
pins a *difference* between the walk and the attestation. A reconciliation bug
that dropped modules, or that emitted a seeding-disagreement note for every
transitively-resolved module rather than only for one the walk never seeded,
would leave all of those green and break this one. It is the fixture that says
attestation is not merely noisier than the walk.

The record names two modules, so the review plan still carries the
artifact-binding note — schema v1 pins one implementation artifact and the
summaries depend on `impl.ts` too. That note is not a closure note and does not
mean the enumeration failed; see docs/package-contracts.md.
