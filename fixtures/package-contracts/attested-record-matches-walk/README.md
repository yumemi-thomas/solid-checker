# The attested closure record equals what the walk found, byte for byte

The negative case for attestation: a package where the generator's syntax walk
and the analyzing program agree completely, so nothing about the record may move
just because its *source* changed.

`index.js` writes `export { thing } from "./impl.js"` and the checkout ships
`impl.ts`. Both sides resolve that: the walk substitutes `.js` → `.ts` from its
own extension table, and TypeScript resolves the ESM-spelled specifier against
the source that exists. The exact closure digest in `expected.json` therefore
binds both files, and the proposal plan binds that artifact case without a
reconciliation hazard.

**What this pins that the other four cannot.** Every other closure fixture here
pins a *difference* between the walk and the attestation. A reconciliation bug
that dropped modules, or that emitted a seeding-disagreement note for every
transitively-resolved module rather than only for one the walk never seeded,
would leave all of those green and break this one. It is the fixture that says
attestation is not merely noisier than the walk.

The runtime file and transitive implementation are distinct closure members.
That is exact artifact identity, not a review note or inline evidence field;
see docs/package-contracts.md.
