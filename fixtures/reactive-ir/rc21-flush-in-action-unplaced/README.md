# rc21-flush-in-action-unplaced

Claim (ADR 0708): `flush()` in a function no call edge places (member, slot, getter, tag, hand-off) an action's body reaches throws in Chrome: an SC2006 obligation, blocker `function-run-unplaced`. Kept: `flush()` in the generator, a violation. Silent: a member nothing reaches. SC2008 asks there too (ADR 0709).

`Forwarded.tsx` (ADRs 0832, 0931): `flush()` in a closure handed through two `const` forwarders in the body runs in place: a violation (was an obligation); after the body's `await`, silent.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
