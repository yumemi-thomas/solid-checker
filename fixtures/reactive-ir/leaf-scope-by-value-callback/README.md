# leaf-scope-by-value-callback

**Claim (ADR 0918).** A leaf callback (`onSettled`, `createTrackedEffect`) named by value, a `const` or function declaration, a chain of `const` copies or a receiver-closed object literal's data slot, whose function is in the same file, is walked as the same function written inline is: each forbidden operation is reported where the callback's run reaches it (the call in its body, or the inline carrier that runs a closure), not at the argument. That is where rc.21 raises the code (Chrome; mx-pairs `cb-alias`, `cb-bundle`, `cb-shared` × the hand-off layers).

Violations: `AliasForEach`, `SharedWrapper` (one finding for two owner calls), `ConstCleanup` (no safe fix: a named body may have other callers; `LiteralCleanup`, inline, has it), `DeclarationPrimitive`. Uncertifiable: `BundleTrackedFlush` (SC3001 at the forEach call, `computation-error-unobserved` as inline, ADR 0893), `SharedAssign` (SC9012 at `ho.m?.()`, as inline), `BundleWritten` and `BundleEscaped` (the slot is not proven, SC9012 at the argument; a write to a member of the slot's name anywhere vetoes it, ADR 0273). Clean: `AliasLegal`, `BundleUnrun`.

Stub: `leaf-scope-named-callback`'s.
