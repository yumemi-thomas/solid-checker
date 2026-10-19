# solid2-precision

Solid 2.0 precision regressions pinned both ways: built-in `filter` literal callbacks after a dominating await carry the reads while `Promise#then`, project `filter`, unresolved and wrapper-built callbacks are not SC1002 (their reads have no role and are SC1001 uncertifiable); primitive cleanup returns are SC3004, `unknown`/`any`/generic returns SC9002, provably-`undefined` returns silent; `store.name = next` is a write only; inline owned `onSettled` reports SC3001 without a duplicate SC4002; leaf rules need a literal callback in the owner's own synchronous extent.
