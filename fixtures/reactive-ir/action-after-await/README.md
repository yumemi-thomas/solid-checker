# action-after-await (ADR 1089 part D)

rc.21's `action(genFn)` wrapper (`@solidjs/signals` 2.0.0-rc.21 `dist/dev.js:2286-2370`: `genFn(...args)` at `:2310`, the first `it.next` at `:2345`) runs the generator's first step on the
caller's stack. So after an await, a call of a `const` action runs no project code but that step. A body with no yield and no read,
whose every call is a setter that runs nothing, ends the site (`library_call::action_body_runs_nothing`).

- `Plain`: no SC1002 row at `a1()`.
- `ReadsSignal` (a signal read in the body), `Yields` (a `yield`), `Rebound` (a written `let`): the row stays.
