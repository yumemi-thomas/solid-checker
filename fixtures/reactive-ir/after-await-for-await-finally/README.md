# after-await-for-await-finally

**Claim (ADR 1164).** Two placements past an await an async computation reaches on every run of a site:

- A read written directly in the body of a `for await` whose iterable is an array literal starting with a number, boolean or `null` literal runs only after the loop awaited its first `next()` (ADR 1038), and the body runs: SC1002 is a violation (`ForAwaitRead`, `ForAwaitReadGuarded`; uncertifiable on main). Not proven, uncertifiable: an empty literal (a dead site, ADR 1018), a named iterable, a leading spread, a promise element, a read in a closure the body runs, a read after the loop. A source read before the loop is a dependency (`ForAwaitTracked`, Chrome quiet, uncertifiable).
- A `catch` or `finally` of a `try` whose block is `await Promise.resolve(…)` of the global `Promise`, with literal arguments only, runs only past the await: a write there has no owner (`FinallyWrite`, `FinallyWriteLiteral`: uncertifiable to clean). Not proven: a name argument, a shadowed `Promise`, `globalThis.Promise`, code before the await in the `try`.

Chrome: `fixtures/runtime-oracle/after-await-for-await-finally`. Stubs: `for-await-body`'s. `tsc --noEmit` clean.
