# after-await-store-alias

**Claim (ADR 0976; S4 of the review of #233).** `const local = store.n; await …; local.x` reads through the store proxy after the suspension: Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at `local.x`. The read universe lists a read of a store or props root and its same-binding siblings, so `local` (a module-scope alias, a destructuring, an element of a store array, a function's return, a cell of a literal, an alias made before the `await`, a props alias) held no site, and main was silent.

The rule (the full rule, fail-closed; the owner ruled it over a census-only variant on 2026-11-15): a member read, a destructuring, an iteration or a spread written after an async function's own suspension is a site unless its object is proven plain (a primitive proven from its own expression, a fresh built-in value, an unshadowed ECMAScript global in a program that leaves every built-in alone). A type proves nothing: `{ x: number }` is what the store proxy has too.

| function | outcome |
|---|---|
| `viaAlias`, `viaDestructured`, `viaArray`, `viaReturned`, `viaCell`, `viaLet`, the alias made before the `await` of a recognized `createMemo(async …)`, the props alias in `Child` | SC1002 uncertifiable |
| `viaParameter` (a parameter: no fact proves it plain) and `viaPlain` (a spread of a project object literal, kept: a project literal is not proven plain, open-work) | SC1002 uncertifiable |
| `viaPrimitive` (the read is before the `await`, arithmetic on a number after it) | no row |

The process test `a_member_read_of_a_value_not_proven_plain_after_an_await_is_a_row` pins the exact set. The oracle case `fixtures/runtime-oracle/after-await-store-alias` is the Chrome twin.
