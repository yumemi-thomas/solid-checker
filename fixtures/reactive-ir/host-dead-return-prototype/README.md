# host-dead-return-prototype

**Claim (ADR 0282, narrowed in ADR 0288).** `host-dead-return`'s proven
`Loop`, in a project that also holds
`Object.assign(Array.prototype, { [Symbol.iterator]: iterator })`. Destructuring
runs `Array.prototype[Symbol.iterator]`, which ADR 0282 premises stable. A
visible iteration mutation veto (ADR 0282 amendment) withdraws the tuple
slot's identity and retains SC9012 uncertifiable at `start()`.

`host-dead-return-wildcard` is the counterpart: a wildcard write on any
provably separate target cannot reach a fresh returned array. An unresolved
target may instead name its intrinsic iterator prototype and must veto.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | SC9012 uncertifiable | the project writes `Array.prototype` reflectively |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
this copies unchanged.
