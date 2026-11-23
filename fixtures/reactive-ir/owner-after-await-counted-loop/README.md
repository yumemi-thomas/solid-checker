# owner-after-await-counted-loop

**Claim (ADR 1185).** An owner-requiring call (an effect, a cleanup) in the consequent of `if (i > 0)` in the body of `for (let i = 0; i < N; i += 1)` (`<=`, `i++`, `++i`; `N` lets a second pass run), where the body awaits at its own statement level and holds no `break` or `continue`, and only the update writes the counter, runs only on a pass after one whose body completed past that await: no owner is current, a violation (`EffectLaterPass`, `CleanupLaterPass`, `HelperLaterPass`, `IifeLaterPass` through an in-place layer; uncertifiable on main, `owner-varies-by-caller`). Not proven, uncertifiable: no guard (the first pass is owned), a `continue`, a conditional await, a counter the body writes, a guard on a shadowing binding, a single pass (dead site), a `getOwner()` probe.

Chrome: `fixtures/runtime-oracle/owner-after-await-counted-loop`. Stubs: `owner-after-await-for-await`'s. `tsc --noEmit` clean.
