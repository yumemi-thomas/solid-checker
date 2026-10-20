# parameter-use-forms

**Claim (ADR 0320, round 6).** A non-rendering function's parameter use that no member invocation describes is an obligation at its callers: a member read whose value may be called (destructured, extracted, handed on) is a possible invocation resolved per call site; an alias reaching a call, wrapper, spread, array element, conditional or non-test logical value, an argument of a non-exact callee, and `arguments` are `unmodeled` (only a primitive or nullish argument settles them) and `SC9012`; tests and primitive arguments are none.
