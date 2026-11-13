# after-await-universe-depth

**Claim (batch 11, slice 9B; ADR 0973).** The callee summary (`callee-reads-nothing`) follows a chain of exactly resolved project calls at most 64 functions deep (`MAX_CLOSURE_DEPTH`): a bound on the search's stack, never a conclusion. A chain beyond it is not concluded about (`blocked:depth`; a stopped answer is never memoized), and the call keeps the uncertifiable row of SC1002.

| site | outcome | why |
|---|---|---|
| `e0()` in `Within` (the head of a chain of 60 pure project functions) | no row: `after_await_accounting.account.callee-reads-nothing` | the whole closure is within the bound, every call resolves exactly to a project function again, and nothing in it reads a source |
| `d0()` in `Beyond` (the head of a chain of 67 pure project functions) | SC1002 uncertifiable, `read-after-await-unaccounted` | the chain is deeper than the bound: the search stops, nothing is concluded |

The program holds a tracked async computation (`Tracked`), so the callee reason's premise holds (the member-route net of ADR 0963 runs); a program with none keeps the row of every call (`after-await-universe-census`). The program boundary is open (`main` in `package.json`): main's proof log, a gate-only mode, grows exponentially with the depth of a call chain in a closed (`private`) program (about 2.5 s at 24 functions, more than 20 s at 40, measured on main 177402dfb), so a chain this long cannot be a fixture of a closed program. The process test `after_await_accounting_process::a_chain_within_the_closure_bound_ends_and_one_beyond_it_keeps_its_row` pins both sides and the mutation of the reason.
