# after-await-read-later-pass

**Claim (ADR 1191).** In an async computation's compute, a reactive read runs past the function's own await, in the flight, when it is:
- written directly in a counted loop's later pass (ADR 1185's shape: `if (i > 0)` in `for (let i = 0; i < N; i += 1)`, an `await` at the body's statement level, no `break` or `continue`; `LaterPassRead`), or
- in function layers run in place (an IIFE, a closure bound to a name and called once, a project helper's callback) whose outermost call is written there or in a `for await` body that runs (ADR 1164; `LaterPassIife`, `ForAwaitClosure`, `ForAwaitHelper`).

These are SC1002 violations (uncertifiable on main). Not proven, uncertifiable: no guard (the first pass reads before any await; Chrome quiet), a `continue`, a guard on a shadowing binding, a standard-library callback (`forEach` may run zero times), `untrack`'s callback, and a closure the body stores instead of calling.

Chrome: `fixtures/runtime-oracle/after-await-read-later-pass`. Stubs: `after-await-for-await-finally`'s. `tsc --noEmit` clean.
