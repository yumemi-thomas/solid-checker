# `createRoot` and `runWithOwner` inherit the strict-read window of their call (ADR 0323, review round 10)

`createRoot(init)` is `runWithOwner(createOwner(), () => init(dispose))`
(`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:3478-3481`); `runWithOwner`
swaps `context` and sets `tracking = false` around `fn()` and touches nothing
else (`:6919-6947`). `strictRead` is set by the component body's dev
`untrack(() => Comp(props), label)` (`:5863-5888`) and every reactive read warns
when it is set (`:6511-6516`). So a read in either callback warns exactly where
the same read written at the call would, and nowhere else.

Before this change the dialect listed `createRoot` as a deferred callback, so
`createRoot(() => count())` in a body was clean, and a hook that ran its
callback in a root made the callback clean too. `runWithOwner` was reported
wherever it sat, including in an event handler and a timer, where no window is
open. Both now take the role of the call (`callback_inherits_calling_window`):
a violation in the body (`InlineCreateRoot`, `InlineRunWithOwner`,
`NestedRoots`); silent in a memo (the listener is cleared), an event, a timer
and a promise continuation; `untrack(fn)` stays silent because it sets
`strictRead = false` (explicit intent). A hook that runs its callback in a root
makes the read an obligation (`HookRootSlot`, `HookRunWithOwnerSlot`,
`HookMemoAndRoot`), and silent when the hook is called from an event.

The tree passes `tsc --noEmit` against the published rc.13 declarations
(copied unchanged from `standard-runtime-configuration`).
