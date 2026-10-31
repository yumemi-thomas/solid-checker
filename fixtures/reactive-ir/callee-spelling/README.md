# callee-spelling

**Claim (ADR 0468).** A callee spelled in a runtime-equivalent way is the function it names, and a callee whose value no fact names is never read as a call of nothing.

Violations, each matching the plain call: parentheses (`(f)()`, nested), an inert comma prefix (`(void 0, f)()`, `(0, f)()`), an optional call of a proven non-nullish binding (`setX?.()`), a TypeScript wrapper over the identifier (`setX!(1)`, `(setX as T)(1)`), for SC2001 writes (`Write*`, in a memo and in render), SC2002 actions (`Action*`), SC4001 cleanups through one or two helpers (`Cleanup*Helper`, `CleanupTwoHelpers`, `CleanupCommaInThen`, `CleanupCommaPrimitive`), SC1001 reads (`ReadComma`) and a callback a helper calls by any of these spellings (`Callback*`); ADR 0515: a plain assignment to a binding as the callee, which calls its right operand (`(slot = setX)(1)`: `WriteAssignedInMemo`, `ReadAssignedInBody`; silent before, the fuzzer's `callee:assign-callee`).

ADR 0516: a callee that always yields one setter is that setter's call (`WriteEffectfulPrefix`, `(note(), setX)(1)` in a memo: a violation). Uncertifiable: a logical or conditional callee over a local setter (`WriteLogical`, `WriteConditional`: SC9012), and a function that may run there with an owner nothing places (`CleanupLogical`, the member tail `(0, o.m)()`, a reassigned or nullified `let`, a mention before its declarator: SC4001).

Silent: the legal controls (a comma-spelled or assigned-callee write in a timer, `WriteAssignedInTimer`, an action in a handler, a cleanup in a root, a tracked read), a shadowing name, a nullish optional target, `(f && h)()` with a falsy left, `(f = h)()` (it calls `h`; `f`'s first function never runs), a receiver-reading method run without its receiver (throws), and `(void 0, q.bind(null))()` (known miss: a returned callable).

tsc: clean against the stub and against the published rc.17 typings. Runtime: each component alone on the fuzzer's rc.17 fast path; every violation and uncertifiable call that runs reports its code; the controls and `CleanupAndLeft`, `CleanupAssignedCallee`, `CleanupReceiverMethod` report none; `CleanupBind` reports `NO_OWNER_CLEANUP` (the known miss).
