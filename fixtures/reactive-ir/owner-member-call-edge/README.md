# A member of a function is not a call of it (ADR 0290, review round 1)

Claim: an owner edge comes from a call that runs a function. A member call of
it (`register.bind(…)`, `register.reset()`) does not, except the standard
library's `call` and `apply`, which run it. An entity at a member
callee's composite span can name its receiver, which once gave such calls an
edge to the receiver, so `register`'s `onCleanup` was taken as running
unowned from the handler.

| Case | Expected |
| --- | --- |
| `Bound` | clean: `register.bind(…)` under `createRoot`, and kept unrun |
| `Direct` | SC4001 at `registerDirect`'s `onCleanup`: a direct call from an event handler |
| `Called` | SC4001 at `registerCalled`'s `onCleanup`: the standard library's `call` runs its receiver |

`solid-js.d.ts` is `detached-callback-in-root`'s stub. `App.tsx` passes
`tsc --noEmit`.
