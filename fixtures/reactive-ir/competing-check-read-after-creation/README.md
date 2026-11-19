# competing-check-read-after-creation

**Claim (ADR 1111).** SC1001 (strict-read-untracked) is uncertifiable `creation-throws-first` for a read written after a creation that the effect-callback guard refuses or may refuse (an SC6002 violation or obligation at the call), in the same function or in a function nested in it. PRIMITIVE_IN_EFFECT_CALLBACK throws before the read on that run, and no fact orders a nested function's runs against the throw.

- `ReadAfterCreationInHelper`: a helper an effect's apply calls runs `createMemo(f)` and then `f()`. SC6002 is a violation; the row through `use` is uncertifiable.
- `ReadAfterGuardedRenderCreation`: a render effect's apply that the first run leaves early. SC6002 is an obligation, and so is the read.
- Kept as violations: `ReadBeforeCreation` (the read runs first) and `ReadInApply` (no creation).

Chrome: `fixtures/runtime-oracle/competing-check-read-after-creation`. Stubs: `competing-check-sync-memo`'s layout, plus `createSignal`.
