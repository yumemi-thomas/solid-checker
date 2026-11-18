# after-await-hook-member-setter

**Claim (ADR 1098).** A call after an await of a signal or store setter a hook's returned literal carries (`thing.useIt(7)` in a promise handler) runs no project code, as a plain setter call does (ADR 0976's `library-runs-nothing`): the callee's symbol is a setter of the table, declared by an exact `const [get, set] = createSignal(…)` with no `equals` function and an inert argument, or by `createStore(…)` in the same file with an updater ADR 1089 D models (a function that writes only literals under static keys). Before, `setter_runs_nothing` required the callee to be the setter's own identifier, so the site was SC1002 uncertifiable (`read-after-await-unaccounted`).

| component | outcome |
|---|---|
| `Carried` | proved: no SC1002 row |
| `Compared` (an `equals` function) | SC1002 uncertifiable |
| `StoreDraft` (a store's setter, updater writing a literal) | proved: no SC1002 row |
| `StoreCarried` (a store's setter given a value) | SC1002 uncertifiable |
| `InSetterCarried` (part 2: `setS1(() => { thing.useIt.v = 7; })`, `thing.useIt` the hook's `s1`) | proved: no SC1002 row |
| `InSetterOther` (the carried store is another one) | SC1002 uncertifiable |
| `InSetterSpread` (the hook's literal spreads) | SC1002 uncertifiable |
| `AsyncRead` (an async memo's read after its await) | SC1002 violation |

`solid-js.d.ts` is `async-creation-pass`'s with `createStore`.
