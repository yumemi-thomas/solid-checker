# owner-after-await-for-await

**Claim (ADR 1165).** An owner-requiring call (an effect, a cleanup, an `onSettled` whose callback returns a cleanup) written directly in the body of a `for await` whose iterable is an array literal led by a number, boolean or `null` literal runs past the loop's await on every run, and the body runs: no owner is current, a violation (`EffectInBody`, `CleanupInBody`, `SettledInBody`, `HelperInBody`; uncertifiable on main, `owner-varies-by-caller`). `CleanupInFinally` is ADR 1164's `Promise.resolve` try head. Not proven, uncertifiable: an empty literal (dead site), a named iterable, a function that probes `getOwner()`, a `finally` around the loop.

Chrome: `fixtures/runtime-oracle/owner-after-await-for-await`. Stubs: `owner-after-await`'s declarations, `for-await-body`'s package stubs. `tsc --noEmit` clean.
