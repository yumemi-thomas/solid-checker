# A read after an await of a source already tracked

`reactive-read-after-await` (SC1002) reports a reactive read after the first
`await` of an async computation, because tracking ended there. A source that
the same function already read **on every run, before its first suspension**
is a dependency of the computation, so a later read of it loses nothing
(ADR 0195). rc.13's dev build agrees: its `UNTRACKED_READ_AFTER_AWAIT` stays
silent for a source already among the computation's dependencies (the rc.13
review, runtime § 3.4.2, probe `PA_memo_already_tracked`).

The earlier read must be a call in the function's own straight-line flow
(`AstFacts::straight_line_calls`): not under a branch, logical operator,
loop, switch, try or optional chain, and not in a nested function. It must
also precede every suspension of the function, a conditional `await`
included.

- Lines 16 and 23 (`count()` read before the await, plainly): clean.
- Line 30 (earlier read under `if`): **SC1002**.
- Line 39 (a conditional await first): **SC1002**.
- Line 46 (a different accessor before the await): **SC1002**.
- Line 54 (earlier read inside `maybe?.use(...)`): **SC1002**.
- Line 62 (earlier read inside a nested arrow): **SC1002**.

The stub `solid-js.d.ts` is `fp-owner-nested-async-read`'s, copied verbatim
from the published typings (`createSignal`, `createMemo` and their types are
unchanged from rc.9 to rc.13).
