# after-await-net

**Claim (ADR 0976; S1 and R6 of the reviews of #233).** Code an async function runs after its `await` with no call written there is after the suspension too: a getter (also through a destructuring and a spread), a `toString` a template coerces, an iterator method a `for…of` runs, a constructor, a tagged template's function, a Proxy's `get` and `has` trap, and a stored generator object spread, iterated or destructured. Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read inside each member; main was silent, because no computation is recognized here (each function is a declaration handed by identifier to a project wrapper) and the member-route net (ADR 0963) opened only for a recognized async computation.

Now the net's gate is any suspension in the program, and a generator's body counts as code run by a route no call shows. The rows:

| site | row |
|---|---|
| the read in `get value()`, `toString`, `*[Symbol.iterator]`, the constructor, the tag function, the two Proxy traps | SC1002 uncertifiable, `function-run-unplaced` (the net) |
| `holder.value`, `const { value } = holder`, `{ ...holder }.value`, `for (… of iterable)`, `new Reads().v`, `proxy.x`, `[...stored]`, `for (… of storedLoop)`, `const [value] = storedPattern` | SC1002 uncertifiable, `read-after-await-unaccounted` (the full S4 rule: an object no fact proves plain) |

Every row is uncertifiable, never a violation. The stored-generator rows come from the full S4 rule (a generator's body is placed by the call that made the object, so the spread, loop and pattern are the sites). The process test `code_a_continuation_runs_with_no_call_written_there_is_a_row` pins the exact set. The oracle case `fixtures/runtime-oracle/after-await-net` is the Chrome twin.
