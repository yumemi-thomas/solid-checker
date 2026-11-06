# rc21-flush-in-action-unplaced

Claim (ADR 0708): `flush()` in a function no call edge places (member, slot, getter, tag, hand-off) an action's body reaches throws in Chrome: an SC2006 obligation. Kept: `flush()` in the generator, a violation. Silent: a member nothing reaches. SC2008 asks there too (ADR 0709).
