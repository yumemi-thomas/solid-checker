# owner-guard-iife

**Claim (ADR 0347).** An operation that runs only after the same invocation has seen a non-null owner (`getOwner() && …`, `if (getOwner()) …`, `getOwner() ? … : …`) is guarded, even inside a function literal the guarded branch invokes synchronously. The guard must be Solid's `getOwner` by symbol, with no `await` between it and the operation.

Silent: `guardedIife`, `guardedAnd`, `guardedTernary`, `guardedNested` (module scope, where the guard is false); `Comp` (a component body).

Reported: `unguardedIife`; `elseBranch`; `shadowedGuard` (a same-named local); `Host` → `awaitedGuard`; `Host` → `guardedAsyncIife` (code after an async literal's first `await`).

Stubs: published rc.13 `solid-js.d.ts`, as in `component-owner-binding`.
