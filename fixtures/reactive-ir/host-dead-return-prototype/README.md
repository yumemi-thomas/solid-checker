# host-dead-return-prototype

**Claim (ADR 0282, narrowed in ADR 0288).** `host-dead-return`'s `Loop` in a project that also holds `Object.assign(Array.prototype, { [Symbol.iterator]: iterator })`: destructuring runs `Array.prototype[Symbol.iterator]`, which ADR 0282 premises stable, and a visible iteration mutation veto withdraws the tuple slot's identity, so `Loop` is SC9012 uncertifiable at `start()`. `host-dead-return-wildcard` is the counterpart. Stubs and `node_modules` copy `host-dead-return`.
