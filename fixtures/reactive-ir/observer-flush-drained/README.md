# observer-flush-drained

**Claim (ADR 0614).** ADR 0613's veto holds for `resolve` and `until`: a `flush()` under a computation drains an apply with that computation's observer active, so a call there is an obligation. Measured on rc.17: `RESOLVE_IN_REACTIVE_SCOPE`.
