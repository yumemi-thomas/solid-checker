# returned-store-proxy

**Claim (ADR 0287).** A hook that returns `new Proxy(state, handler)` over the store it created returns that store when the handler defines only a `get` trap and every value the trap returns is `Reflect.get(target, prop[, receiver])` over its own unwritten parameters. Every other operation forwards to the target, and a throw returns nothing. A same-file `return useBase()` forward returns what `useBase` returns. The shape is `@tanstack/solid-query` 6.0.0-rc.0's `useQuery` → `useBaseQuery`, without its server branch.

| Case | `SC1001` on `query.search` | Why |
| --- | --- | --- |
| `ForwardingProxy` | violation | solid-query's trap: symbol and pass-through forwards, a throw, a final forward |
| `TwoParameterProxy`, `ArrowProxy` | violation | `Reflect.get(target, prop)`; a concise arrow trap |
| `Forwarded` | violation | `return useForwardingProxy()` in the same file |
| `SetTrap`, `HasTrap` | none (refused) | a second trap |
| `OtherReturn` | none (refused) | a live branch returns another object's member |
| `OtherStore` | none (refused) | the trap returns another store's member |
| `WrittenTarget`, `LeakedTarget` | none (refused) | the trap writes or passes on its `target` |
| `Fallthrough` | none (refused) | a completion returns `undefined` |
| `PlainTarget`, `SignalTarget` | none (refused) | the target is no store |
| `SpreadHandler`, `HeldHandler` | none (refused) | the handler is not a literal written in place |
| `EscapedRoot` | none (refused) | the store escapes outside the proxy (ADR 0222) |
| `ForwardedEither` | none (refused) | two forwards reach different sources |
| `CrossFileForward` | none (refused) | the forward is in another file (`forward.ts`) |

Each refusal is silent: the value has no source, as before ADR 0287. The proof's syntax half is unit-tested in `solid_facts::ast::proxy_get_forward`; the folded server branch is `dependency-source-proxy-store`.

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `returned-store-nested-read` (claim-bearing signatures byte-faithful to the published 2.0.0-rc.13 typings). `ProxyHandler` and `Reflect.get` come from TypeScript's own `lib.es2015`. `tsc --noEmit` passes.
