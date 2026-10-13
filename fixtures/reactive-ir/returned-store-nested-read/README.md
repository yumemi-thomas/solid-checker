# returned-store-nested-read

**Claim (ADR 0281).** ADR 0222's returned-source proof refused a hook whose body names the store root anywhere inside a call argument, so the `@tanstack/solid-query` `useMutation` shape (a render effect that reads `state.isError` and throws) lost its store, and every caller's read went silent. A root reference inside an argument now escapes only when it can hand the root or a sub-proxy to other code: the bare root, a member chain that is itself an argument or a callee (which receives the proxy as `this`), or a chain a function returns (`source_discovery::member_chain_escapes`).

| Case | `SC1001` on `thing.search` | Why |
| --- | --- | --- |
| `ThrowingEffect` | violation | the render effect consumes `state.failed` in its own body and throws |
| `MemoRead` | violation | `const search = state.search` inside the memo |
| `NestedThrow` | violation | a throw that touches no store (pin) |
| `EscapedRoot`, `EscapedInArrow`, `EscapedInObject` | none (refused) | the root itself reaches `sink` |
| `EscapedSubProxy` | none (refused) | `sink(state.items)` hands a sub-proxy over |
| `EscapedReturnedMember` | none (refused) | `() => state.items` returns a sub-proxy |
| `EscapedThis` | none (refused) | `state.items.indexOf(1)` passes the sub-proxy as `this` |

`EscapedSubProxy` also reports the hook's own `state.items` read through the call, an existing rule. `solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `returned-store-source`, with `createRenderEffect` and `createMemo` added from `fp-owner-loading-cover`; `tsc --noEmit` passes.
