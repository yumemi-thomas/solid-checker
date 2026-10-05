# component-owner-binding

**Claim (ADR 0206).** `const owner = getOwner()` written directly in a proven component's body holds that component's owner, never `null`, so `runWithOwner(owner, fn)` runs `fn` under an owner wherever it is called (`owners::component_owner_binding`). An `onCleanup` there is owned.

| Case | Finding | Why |
| --- | --- | --- |
| `Carried` | none | the component's owner, carried across a promise |
| `CallsHelper` | `SC4001` uncertifiable | `getOwner()` in a helper that is not a component |
| `Reassignable` | `SC4001` uncertifiable | a `let` binding may hold something else when it is used |
| `NestedGetOwner` | `SC4001` uncertifiable | `getOwner()` in a nested function returns the owner it is called under |

`solid-js.d.ts` is copied from `owner-after-await`; `getOwner` and `runWithOwner` are verbatim from the published typings there.
