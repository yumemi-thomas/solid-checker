# after-await-universe-census

**Claim (batch 11, slice 9B; ADR 0973).** There is **no census reason**: a program that holds no tracked async computation the producers find (`holds_suspending`, the census of ADR 0963: no `createMemo(async ...)`, no effect compute that awaits) keeps the uncertifiable row of SC1002 at every read written after an await. "The program holds no computation" is no ending, because it is only as complete as the producers' index of computations, and Chrome shows starts that index does not find.

| site | outcome | why |
|---|---|---|
| `count()` in `load`, `count()` and `load()` in `loadTwice` (called at module level only) | SC1002 uncertifiable, `read-after-await-unaccounted` | nothing starts a walk and nothing here runs them as a computation, yet no proof says so: a reason there needs a proof that no computation runs the function (no value use of it, callers only module-level code or such functions), not the absence of a recognized start |
| `count()` in the exported `exported` | SC1002 uncertifiable, `read-after-await-unaccounted` | code this build cannot see may import and run an export |
| `count()` in `work` (a function **declaration** handed by identifier to `track`, which gives its parameter to `createMemo`) | SC1002 uncertifiable, `read-after-await-unaccounted` | the hazard: Chrome rc.21 runs `work` as an async computation and reports UNTRACKED_READ_AFTER_AWAIT at the read (the oracle case `after-await-universe-census-wrapper`); the producers' hand-off (ADR 0873) follows a function literal or an arrow through a wrapper, not a declaration, so no start is found (a recall follow-up in `docs/open-work.md`) and `holds_suspending` is false for the whole program |

The three other forms that start a computation the producers' index does not follow have oracle cases of their own, each a site that must keep a row: `createMemo.call(undefined, work)` (`after-await-universe-census-call`), `Reflect.apply(createMemo, undefined, [work])` (`-reflect`) and `[createMemo][0](work)` (`-element`).

The twin `after-await-universe` holds a recognized tracked computation. The process test (`after_await_accounting_process::a_program_without_a_recognized_computation_keeps_every_sites_row`) pins every site above, and that dropping each typed reason (`callee-reads-nothing`, `never-runs`) moves nothing here: no reason holds at any of them.
