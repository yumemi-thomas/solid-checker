# rc21-flush-in-action-unplaced

Claim (ADR 0708): `flush()` in a function no call edge places (object or class method, getter, `Map` slot, a function handed to a helper, `toString`) that an action's body reaches throws `FLUSH_IN_ACTION` (Chrome, rc.21, witnesses SC2006 a02-a10): an SC2006 obligation, blocker `function-run-unplaced`. The tag was one already (ADR 0538). Kept: `flush()` in the generator is a violation. Silent: a member nothing reaches. SC2008 asks at the same layers (ADR 0709).
