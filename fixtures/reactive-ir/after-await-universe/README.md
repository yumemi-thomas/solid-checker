# after-await-universe

**Claim (batch 11, slice 9B; ADR 0973).** Every read and every call of a project function written after an async function's own suspension ends in a row of SC1002, an SC9012, the compiler's discard or a typed reason (`after_await_accounting.rs`); a site that ends in none gets an **uncertifiable row of SC1002, blocker `read-after-await-unaccounted`**: never a violation, never a silence. UNTRACKED_READ_AFTER_AWAIT's producers walk only from a tracked async computation, so a read or a call they never reach had no row and no record of why.

This program holds a tracked async computation with something after a suspension (`Tracked`), and no site below is that computation's own. Each site below is silent on main (SC1001's own rows are as before); the unaccounted row is the only change.

| site | outcome | why no reason ends it |
|---|---|---|
| `count()` in an async `onClick` handler, and `count()` in the exported helper `helper` | SC1002 uncertifiable, `read-after-await-unaccounted` | the read is after an await of a function no computation runs; nothing walks it. A role silence (a handler, a deferred callback) is left out on purpose: each needs a premise and a Chrome twin of its own (open-work) |
| `stringify(1)` | the same | a call of a project function after an await: no reason ends it |
| `readsCount()` | the same | the same |
| `ping(3)` | the same | the same |
| `callback()` (a parameter) and `reassigned()` (a reassigned `let`) | the same | a call of a function value no exact declaration resolves (ADR 0964's shape): never summarized |
| `count()` after `for await` | the same | an implicit suspension is a suspension |

The twin `after-await-universe-none` holds the sites the typed reason ends (no row), `after-await-universe-census` a program with no recognized computation, where every site keeps its row (there is no census reason). `node_modules/solid-js/package.json` selects the dialect; `solid-js.d.ts` declares what the program calls.
