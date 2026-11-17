# leaf-scope-by-value-callback

**Claim (ADR 0918).** A same-file leaf callback named by value is walked as if inline: a forbidden operation is reported where the run reaches it.

V: `AliasForEach`, `SharedWrapper`, `ConstCleanup`, `DeclarationPrimitive`, `BundleTrackedFlush` (ADR 1044: the closed literal's `box.cb` runs where `createTrackedEffect` runs it; U before). U: `SharedAssign`, `BundleWritten`, `BundleEscaped`. Clean: `AliasLegal`, `BundleUnrun`.
