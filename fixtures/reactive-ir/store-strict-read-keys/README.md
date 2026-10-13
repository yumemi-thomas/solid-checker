# store-strict-read-keys

**Claim (ADR 0293).** A store Get warns `STRICT_READ_UNTRACKED` only for a string key other than `"then"`: the rc.9 and rc.13 `@solidjs/signals` store trap tests `typeof key === "string" && key !== "then"` before warning (rc.13 `dist/dev.js:6660-6675`). An own function-valued member is an ordinary string key and warns like data. The head Get of a member chain decides: it runs first, on the store proxy. A `then` head is withheld; a key solid-checker cannot spell (a symbol, a dynamic expression), or a `then` head followed by another Get on its value, is an `SC9012` obligation, never a violation (`local_access::store_head_get`). Interprocedural summaries carry both as undecided rows, uncertifiable at the caller.

**Review round 1.** A `then` Get is withheld only when the store's own literal proves the property it reads runs nothing (`local_access::store_get_decision`): a same-file `const [q] = createStore({ ... })` with no used setter, a closed literal, `then` absent or a data value that is not code (or a function literal with no call, member, reference or keyword), and no write of a `then` member, prototype write or reflective property definition anywhere. Otherwise it is undecided. The cases are in `Review.tsx`.

| Case | Finding | Why |
| --- | --- | --- |
| `OwnFunction`, `ForwardedFunction` | `SC1001` violation | `q.refetch`, own function member, directly and through ADR 0287's forwarding proxy (`hook.ts`) |
| `DataMember`, `KnownComputedKey` | `SC1001` violation | `q.data`, `q["data"]` |
| `NestedThen` | `SC1001` violation | the head `q.nested` warns whatever follows |
| `ClosureData` | `SC1001` violation | through a closure the body calls |
| `ThenRead`, `ComputedThen`, `ClosureThen` | none from `SC1001` | a `then` Get does not warn, and `() => {}` runs nothing. `ComputedThen` keeps its existing `SC9012` for the dynamic call |
| `ForwardedThen` | `SC9012` uncertifiable | review round 1: the store is declared in `hook.ts`, so its backing is not proven here |
| `SymbolRead`, `UnknownComputedKey` | `SC9012` uncertifiable | the key may be a symbol or `then` |
| `ThenHeadChain`, `SymbolHeadChain` | `SC9012` uncertifiable | the head does not warn (or may not); the next Get warns only if the value is a store proxy |
| `DestructuredData`, `DestructuredThen` | `SC1003` violation | `no-destructure` is a different claim: the bindings are frozen, which holds for `then` too |
| `AliasedData`, `AliasedThen` | none | a plain alias is not followed (a recall gap; `AliasedThen` guards the key rule if it ever is) |
| `HelperThen` | `SC9012` uncertifiable (existing) | a parameter member call is a dispatch obligation, whatever the key |
| `HelperData` | none | a parameter member Get carries no store read (existing) |
| `PropParent` children | `SC1001` uncertifiable on `props.store` (existing) | the prop Get, not the store Get |
| `Review.tsx` `GetterThen`, `UnprovenThenBacking`, `CalledReadingThen`, `WrittenStoreThen` | `SC9012` uncertifiable | the backing `then` is a getter, an out-of-line object, a function that reads, or may be rewritten through the setter. `GetterThen` and `CalledReadingThen` also keep the uncertifiable `SC1001` at the `count()` written in the literal |
| `Review.tsx` `ThenData`, `AbsentThen` | none | `then: 1`, or no `then` at all |
| `Review.tsx` `HelperUndecidedKey`, `ClosureUndecidedKey` | `SC1001` uncertifiable at the call | an undecided key in a helper's summary |
| `Review.tsx` `HelperMixedKeys` | `SC1001` violation | the warning `q.data` beside an undecided `q[key]` keeps the call proven |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `returned-store-proxy` (claim-bearing signatures byte-faithful to the published 2.0.0-rc.13 typings). `tsc --noEmit` passes against them and against the real installed rc.13 packages (`strict`, `skipLibCheck: false`).
