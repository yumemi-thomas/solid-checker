# host-dead-return-prototype

**Claim (ADR 0282, narrowed in ADR 0288).** `host-dead-return`'s proven
`Loop`, in a project that also holds
`Object.assign(Array.prototype, { [Symbol.iterator]: iterator })`. Destructuring
runs `Array.prototype[Symbol.iterator]`, which ADR 0282 premises stable. A
reflective write whose target is spelled through a `prototype`, or is a
spread, is the visible veto. So no tuple slot resolves and `Loop` is silent.

`host-dead-return-wildcard` is the counterpart: a wildcard write on any
other target cannot reach a fresh returned array, and `Loop` is reported.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | none | the project writes `Array.prototype` reflectively |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
this copies unchanged.
