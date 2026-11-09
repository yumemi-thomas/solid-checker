# leaf-scope-by-value-callback

**Claim (ADR 0918).** A same-file leaf callback named by value is walked as if inline: a forbidden operation is reported where the run reaches it, not at the argument.

V: `AliasForEach`, `SharedWrapper`, `ConstCleanup`, `DeclarationPrimitive`. U: `BundleTrackedFlush`, `SharedAssign`, `BundleWritten`, `BundleEscaped`. Clean: `AliasLegal`, `BundleUnrun`.
