# flush-later-continuation

**Claim (ADR 1201).** A `flush()` no `await` dominates still runs only past a suspension of its own async function in two shapes the census now places as code after an await (`FlushSafe::LaterContinuation`): a counted loop's later pass (ADR 1185's `in_counted_loop_later_pass`: the `if (i > 0)` consequent of `for (let i = 0; i < N; i += 1)` whose block body awaits at its own statement level, with no `break` or `continue`) and the `catch` or `finally` of a `try` whose block starts with an await of an operand that cannot throw synchronously (`follows_try_head_await`: an async function declaration's call, or `Promise.resolve` of literals with the intrinsics intact, ADR 1164). The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason ends any site here.

Clean: `MemoLoop`, `EffectFinally`, `HelperLoop` (the helper's one call is the site asked, ADR 1199), `HandlerLoop` (a closure's call in the later pass of an event handler's async function: the census class of the call is the delivery's, which a dominating await already overrides, and so does this).

Slice 2: `NamedLoop` and `NamedFinally` are clean too: a compute handed by name is a function no call edge places, and the effect-phase walk's SC2008 row for it (ADR 0709) asks whether the flush runs off every delivery (`flush_runs_off_any_delivery`), which now takes the same two shapes. `NamedFirstPass` keeps its row.

Uncertifiable (fail closed): `HandlerFirstPass` (the first pass flushes on the delivery's stack), `FirstPass` (no `i > 0` guard: the first pass flushes before any await), `Breaks` (a `break` in the body), `RiskyFinally` (the `try` head's operand may throw before it suspends).
