# A member of a function is not a call of it (ADR 0290)

An owner edge comes from a call that runs a function; a member call of it (`register.bind(...)`, `register.reset()`) does not, except the standard library's `call`/`apply`, and an entity at a member callee's composite span can name its receiver. `Bound` (`register.bind(...)` under `createRoot`) is clean; `Direct` (an event handler call) and `Called` (`call` runs its receiver) are SC4001 at the `onCleanup`. Stub `detached-callback-in-root`'s.
