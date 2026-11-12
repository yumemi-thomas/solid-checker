# owner-call-effect-creation

**Claim (batch 11, slice 6; ADR 0969).** A creation the dialect says throws PRIMITIVE_IN_EFFECT_CALLBACK on an effect's apply (`inEffectCallback && context === null`: `createMemo`, `createEffect`, `createRoot`, `render`, ...) is a site of SC6002. It ends in a row of the rule, an SC9012, the compiler's discard, or a silence that names the fact the throw needs and the code lacks; with none it gets `effect-creation-unaccounted`, an uncertifiable row, never a violation. Before this, a creation no walk rowed was silent by omission.

| site | outcome | why |
|---|---|---|
| `createMemo` in `InApply`'s apply | SC6002 violation | the effect-phase walk proves it: no owner is set on the way |
| `createRoot` in `RootInApply`'s apply | SC6002 violation; what its callback creates: no site that runs | a refused creation enters nothing (ADR 0475) |
| `createMemo` in `RootInMemo`'s root callback | silent, `effect-creation-owner-set-around` | an exact primitive that creates an owner (`createRoot`) wraps it, itself inside a computation's compute |
| `Body`, `Handler`, `Timer` | silent, `effect-creation-runs-off-apply` | the role engine places the code off every apply: rendering, an event handler (no synchronous dispatch in this program), a deferred callback |
| `top` at module scope | silent, `module-scope` | module evaluation runs it once, on a stack of its own |
| `dead` | silent, `never-runs` | a private function nothing references: ADR 0458's query proves no execution runs it |
| `Holder.memo`, a class field's initializer | no silence (the ADR 0710 row stands) | the initializer runs at each construction, which may be anywhere: it is no module scope |
| `shared`, called by `SharedBody` and by `SharedTimer`'s timer | `effect-creation-unaccounted` | the owner is current on one entry only (the timer's has none), and no role places the helper's own code |
| `stored`, kept in an array | the ADR 0710 row (`function-run-unplaced`) | the function is entered by a reference that is no direct call |

`variants/sync-dispatch`: the same handler, in a program that dispatches an event synchronously (`document.body.click()` in an effect's apply): a DOM dispatch runs the handlers on its caller's stack, so the handler is no longer placed off every apply, and its creation keeps `effect-creation-unaccounted`. The program names no leaf opener, so SC3001's two codes end in `no-opener-in-program` here (`owner-call-no-leaf-opener`). `node_modules` holds a reduced `solid-js`, `@solidjs/signals` (the package layout gives the callee a package root, which the effect-phase walk asks) and `@solidjs/web`'s `render`. `owner-call-effect-creation-unresolved-package` is the same `InApply` with the declarations ambient (no package root): certified before, uncertifiable now.
