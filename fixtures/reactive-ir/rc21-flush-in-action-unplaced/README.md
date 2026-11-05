# rc21-flush-in-action-unplaced

Claim (ADR 0708): `flush()` in a function no call edge places (a member, slot, getter, tag or hand-off) that an action's body reaches throws `FLUSH_IN_ACTION` (Chrome): an SC2006 obligation, blocker `function-run-unplaced`. Kept: `flush()` in the generator is a violation. Silent: a member nothing reaches. SC2008 asks at the same layers (ADR 0709).
