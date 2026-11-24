# after-await-closed-callers

**Claim (ADR 1197, the owner's addendum to ADR 0973's ruling 5).** A read past an await in an async function every call of which is enumerated and none escapes is not under any async computation's flight: SC1002 is silent there. The function is same-file and unexported, every reference to it is a direct call, and each call runs (as the caller's own code) from an intrinsic element's `on…` handler, a component body, or a helper for which the same holds. Clean: `BodyCall`, `HandlerCall`, `HelperChain` (uncertifiable on main, `read-after-await-unaccounted`). Uncertifiable as before: an exported function (`ExportedCall`), one handed as a callback (`PassedAsCallback`), one a memo's compute calls (`CalledFromMemo`) or that a helper a memo calls calls (`HelperFromMemo`).

Chrome: `fixtures/runtime-oracle/after-await-closed-callers` (silent at every clean). Stubs: `after-await-for-await-finally`'s. `tsc --noEmit` clean.
