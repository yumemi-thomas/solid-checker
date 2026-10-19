# component-owner-binding

**Claim (ADR 0206).** `const owner = getOwner()` written directly in a proven component's body holds that component's owner, never `null`, so `runWithOwner(owner, fn)` runs `fn` under an owner wherever called (`owners::component_owner_binding`) and an `onCleanup` there is owned (`Carried` clean); `getOwner()` in a non-component helper, a `let` binding and a nested function's `getOwner()` are `SC4001` uncertifiable. `getOwner`/`runWithOwner` verbatim from the published typings in `owner-after-await`.
