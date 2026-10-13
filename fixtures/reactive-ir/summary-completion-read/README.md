# summary-completion-read

**Claim (ADR 0276).** A strict read attributed through a called function's summary is a proven untracked read only when it runs on every normal completion of the call. Its origin must be a completion site of the function that holds it, and each hop down to that function a completion site of its caller. A read on some paths only, such as behind a branch, after an early return or a thrown guard, in a logical assignment or in a default's own branch, stays uncertifiable: the path may depend on this call's arguments. The call itself, in the component body, stays under the statically-present-code premise (`interproc::site_on_every_completion`, `AstFacts::completion_sites`).

| Case | Finding | Why |
| --- | --- | --- |
| `CallsReadAlways` | `SC1001` violation | `readAlways` reads on every completion |
| `CallsReadThroughAlways` | `SC1001` violation | both hops unconditional |
| `CallsReadWhen` | `SC1001` uncertifiable | the `MaybeAccessor` shape: a number returns before the read |
| `CallsReadGuarded` | `SC1001` uncertifiable | the read is a ternary arm |
| `CallsReadStoreGuarded` | `SC1001` uncertifiable | a store path behind a branch |
| `CallsReadThroughWhen` | `SC1001` uncertifiable | the second hop is conditional |
| `CallsReadDefaultGuarded` | `SC1001` uncertifiable | the default reads only on one of its own paths |
| `CallsReadAfterThrowGuard` | `SC1001` uncertifiable | a thrown guard skips the read, and a caller may catch it |
| `CallsReadAfterFail` | `SC1001` uncertifiable | a `never`-returning guard may throw into the caller's `catch` (ADR 0278) |
| `CallsReadAfterAssert` | `SC1001` uncertifiable | an `asserts` guard may throw into the caller's `catch` (ADR 0279) |
| `CallsReadAfterFailingDefault` | `SC1001` uncertifiable | a `never` call in a default runs before the body (ADR 0279) |
| `CallsReadWhenUnset` | `SC1001` uncertifiable | `??=` reads its right side only when the target is nullish |
| `CallsReadAfterLoop` | `SC1001` uncertifiable | a `break` is read as a possible exit, which only proves less |

`solid-js.d.ts`, `tsconfig.json` and `node_modules` are copied from `summary-direct-read`. `createStore` and its types are added from `async-call-prefix`'s rc.13 declarations. `tsc --noEmit` passes.
