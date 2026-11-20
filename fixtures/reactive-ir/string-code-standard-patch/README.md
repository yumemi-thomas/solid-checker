# string-code-standard-patch

**Claim (ADR 1137; the review of ADR 1124, round 2, N1).** Code built from a string (`(0, eval)("…")`, `new Function("…")()`, a timer handed a string) may replace any standard member, here `Array.prototype.forEach` by one that defers its callbacks while a flag is set. The inline `[1].forEach(() => { base(); })` inside `untrack` then runs its read later, in the component body, where rc.21 warns STRICT_READ_UNTRACKED. The standard-call veto (`indexes/callees.rs` `standard_call_withheld`) withholds every standard call while the program holds such code, so the read is an SC1001 uncertifiable row, not clean.

- `App.tsx` (indirect `eval`), `variants/new-function`, `variants/string-timer`: the SC1001 row at `base()` in the callback.
- `variants/function-timer`: timers handed functions (a literal's name, a parameter typed `() => void`): no string code, so the inline callback is placed inside `untrack` and the read is clean.
- `variants/none`: no patch: clean.
