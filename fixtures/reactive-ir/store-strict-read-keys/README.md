# store-strict-read-keys

**Claim (ADR 0293).** A store Get warns `STRICT_READ_UNTRACKED` only for a string key other than `"then"`: the rc.9 and rc.13 `@solidjs/signals` store trap tests `typeof key === "string" && key !== "then"` before warning (rc.13 `dist/dev.js:6660-6675`). An own function-valued member is an ordinary string key and warns like data. The head Get of a member chain decides: it runs first, on the store proxy. A `then` head is withheld; a key solid-checker cannot spell (a symbol, a dynamic expression), or a `then` head followed by another Get on its value, is an `SC9012` obligation, never a violation (`local_access::store_head_get`). Interprocedural summaries withhold both.

| Case | Finding | Why |
| --- | --- | --- |
| `OwnFunction`, `ForwardedFunction` | `SC1001` violation | `q.refetch`, own function member, directly and through ADR 0287's forwarding proxy (`hook.ts`) |
| `DataMember`, `KnownComputedKey` | `SC1001` violation | `q.data`, `q["data"]` |
| `NestedThen` | `SC1001` violation | the head `q.nested` warns whatever follows |
| `ClosureData` | `SC1001` violation | through a closure the body calls |
| `ThenRead`, `ComputedThen`, `ForwardedThen`, `ClosureThen` | none from `SC1001` | a `then` Get does not warn. `ComputedThen` keeps its existing `SC9012` for the dynamic call |
| `SymbolRead`, `UnknownComputedKey` | `SC9012` uncertifiable | the key may be a symbol or `then` |
| `ThenHeadChain`, `SymbolHeadChain` | `SC9012` uncertifiable | the head does not warn (or may not); the next Get warns only if the value is a store proxy |
| `DestructuredData`, `DestructuredThen` | `SC1003` violation | `no-destructure` is a different claim: the bindings are frozen, which holds for `then` too |
| `AliasedData`, `AliasedThen` | none | a plain alias is not followed (a recall gap; `AliasedThen` guards the key rule if it ever is) |
| `HelperThen` | `SC9012` uncertifiable (existing) | a parameter member call is a dispatch obligation, whatever the key |
| `HelperData` | none | a parameter member Get carries no store read (existing) |
| `PropParent` children | `SC1001` uncertifiable on `props.store` (existing) | the prop Get, not the store Get |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `returned-store-proxy` (claim-bearing signatures byte-faithful to the published 2.0.0-rc.13 typings). `tsc --noEmit` passes against them and against the real installed rc.13 packages (`strict`, `skipLibCheck: false`).
