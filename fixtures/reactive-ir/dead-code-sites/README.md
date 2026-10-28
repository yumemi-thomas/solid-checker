# dead-code-sites

**Claim (ADR 0458).** A violation at a site no execution reaches is uncertifiable, never a violation and never silent. Unreached: the right of `false &&` and `true ||`, an `if (false)` arm and a `false ?` arm, code after a `return` or `throw` in the same block (a class declared there never runs its static block), a function nothing that runs calls, renders or names (`f1` called only from an unreferenced `Child1`), a function an IIFE returns into a discarded result, and a method of a class nothing names.

`Unreached.tsx` (13 uncertifiable): the five Chrome-confirmed fuzz shapes m1-m5 (`DeadAndOperand`, `IifeResultDiscarded`, `ClassAfterReturn`, `ClosureOfUnreferencedChild`, `NestedEffectsUnrendered`; every site had 0 hits and no diagnostic in Chrome, rc.17) and the controls `CleanupAfterReturn`, `AsyncUnreferenced` (same rows). The memo cases (`IfFalseInMemo`, `TernaryFalseArmInMemo`, `OrTrueInMemo`, `AfterThrowInMemo`, `UnreferencedClassMethod`) were run as memo computes on the `@solidjs/signals@2.0.0-rc.17` dev build: none throws.

`Live.tsx` (9 violations): the same sites reached. `HandleCalled`, `StaticBlockLive` and the rendered children are the Chrome rows' live variants (rc.17 raises `REACTIVE_WRITE_IN_OWNED_SCOPE`, `NO_OWNER_CLEANUP`, `NO_OWNER_EFFECT`). `IfTrueInMemo`, `RuntimeConditionInMemo` and `HoistedAfterReturnInMemo` (a hoisted `function` declared after the `return` and called before it) throw `REACTIVE_WRITE_IN_OWNED_SCOPE` on the dev build.

Both files type-check against the published `solid-js@2.0.0-rc.17` typings (tsrs, the runtime-oracle install). Stub: `write-cleanup-handle-call`'s, with `span`.
