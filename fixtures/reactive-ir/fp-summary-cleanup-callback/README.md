# fp-summary-cleanup-callback

**Claim.** A read inside an `onCleanup` callback is not a strict-window read:
the callback is registered on the owner and runs when that owner is disposed or
re-runs, so it is attributed `Deferred`, as `onSettled`'s is. The Solid 2
dialect's attribution table (`Solid2::callback_executions`) had no `OnCleanup`
row, so the callback fell through to the lexical component-body role and every
read in it was reported as a proven untracked read in the body.

Measured on the published `solid-js@2.0.0-rc.9` dev bundle: a read directly in
the component body raises `STRICT_READ_UNTRACKED`; the same read in an
`onCleanup` callback raises nothing.

The row lives in the dialect (`rust/crates/solid-dialect/src/solid_2.rs`), not
in shared code: whether a cleanup registrar's callback runs inside the
registering body's window is a runtime fact of the dialect. The contract word
for `onCleanup` (`deferred`) is stated separately and is unchanged.

| Case | Finding | Why |
| --- | --- | --- |
| `onCleanup(() => { if (n()) ... })` | none | deferred callback |
| `onCleanup(() => console.log(n()))` | none | same, expression body |
| `onCleanup(() => console.log(label()))` through a derived helper | none | same |
| `onCleanup(stop)` with a named callback | none | named function, never called in the body |
| `n()` in the body next to an `onCleanup` | `SC1001` violation | body read, unchanged |
| a snapshot taken in the body and captured by the callback | `SC1001` violation | the read is in the body |

**Stub.** `solid-js.d.ts` copies `createSignal`, `createMemo`, `Disposable` and
`onCleanup` verbatim from the rc.9 typings (`signals.d.ts`, `core/types.d.ts`),
as its header lists; `App.tsx` passes `tsc --noEmit` (strict) against the stub
and against the real rc.9 install.
