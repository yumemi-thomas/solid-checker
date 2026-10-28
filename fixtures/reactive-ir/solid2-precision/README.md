# solid2-precision

Solid 2.0 precision regressions pinned both ways: built-in `filter` literal callbacks after a dominating await carry the reads while `Promise#then`, unresolved and wrapper-built callbacks are not SC1002 (a project `filter` method is, where its exact body calls the callback synchronously: ADR 0485) (their reads have no role and are SC1001 uncertifiable); primitive cleanup returns are SC3004, `unknown`/`any`/generic returns SC9002, provably-`undefined` returns silent; `store.name = next` is a write only; inline owned `onSettled` reports SC3001 without a duplicate SC4002; leaf rules need a literal callback in the owner's own synchronous extent.

`WrappedSettledCleanup` (App.tsx): the `onSettled` callback is `wrap(...)`'s result, a call's value, so the `onCleanup` inside `wrap`'s literal is no longer reported as SC4001 (ADR 0345 review D; docs/open-work.md records the lost recall). The App.tsx comment above it still names an `SC4002` that this catalog does not have; the claim that changed is the SC4001 on that line.

`ComputedAssignmentTarget`: `rows[props.index].done = true` writes through an element-access link, whose type has no `undefined` without `noUncheckedIndexedAccess`, so SC2003 there is uncertifiable (ADR 0465).
