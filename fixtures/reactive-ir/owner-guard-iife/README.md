# owner-guard-iife

**Claim (ADR 0347 amendment).** An operation that runs only after the same invocation of its function has seen a non-null owner (`getOwner() && …`, `if (getOwner()) …`, `getOwner() ? … : …`) is guarded, even when it sits in a function literal the guarded branch invokes synchronously (an IIFE, nested or not). The guarded branch then runs under that owner, and nothing is reported. The guard must be Solid's `getOwner` by symbol, and no `await` may separate it from the operation: an owner seen before an `await` is not current after it.

Silent, guarded:
- `guardedIife`, `guardedAnd`, `guardedTernary`, `guardedNested`: the operation is in an IIFE (or two) that the guard invokes. These helpers are called at module scope, where the guard is false, so nothing runs.
- `Comp`: the same guarded IIFE in a component body, where the owner is current.

Reported, the controls:
- `unguardedIife`: no guard.
- `elseBranch`: the literal sits in the branch the guard does not take; module scope runs it with no owner.
- `shadowedGuard`: `getOwnerLocal` is a same-named local function, not Solid's `getOwner`, so it proves nothing.
- `Host` → `awaitedGuard`: an `await` between the guard and the operation, so the owner seen before it is not current.
- `Host` → `guardedAsyncIife`: an async literal's code after its first `await` is a continuation with no owner.

Stub declarations are copied from published rc.13 typings (`solid-js.d.ts`, the same declarations as `component-owner-binding`).
