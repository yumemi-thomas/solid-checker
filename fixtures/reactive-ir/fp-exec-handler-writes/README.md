# fp-exec-handler-writes

**Claim.** A signal write is a proven owned-scope write (`SC2001`) only where
the facts place it in a phase that runs under an owner. Code that runs later,
or that nothing proves runs during the body, produces no finding:

- an event handler (a function that is a JSX attribute's value) never runs
  while the component body runs, whatever expression wraps its JSX --
  `cond && <el onClick=... />`, a ternary, a `.map` callback, an IIFE, an
  argument to a helper call. The wrapper's tracked role described the
  *expression*, not the function inside it
  (`enclosed_by_jsx_attribute_function`);
- a closure kept in a value a tracked scope returns (`createMemo(() => ({ go:
  () => set(1) }))`) runs when something calls it, not while the memo computes
  (`enclosed_by_stored_function`);
- a closure nested in a leaf callback (`createTrackedEffect`, `onSettled`) -- a
  timer, a listener, a continuation -- is either deferred or runs inside the
  leaf, and the runtime's write guard exempts both
  (`nested_in_leaf_scope`);
- an `onCleanup` callback is kept for disposal, and the dialect models no
  invocation of it during the call (`nested_literal_runs_during_body`);
- code after an `await` in an async function runs on a later task with no
  owner, so neither the lexical role nor the role at the function's call sites
  describes it (`follows_await_in_async_function`). It is not proven to follow
  the `await` on every path, so the answer is no claim.

Runtime ground truth (2.0.0-rc.9, dev, defect triage
`triage-other.md`): a signal write throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a
component body, root body or memo; event handlers, cleanups, `createTrackedEffect`,
`onSettled`, effect apply and post-`await` continuations are legal.

| Case | Finding | Why |
| --- | --- | --- |
| `HandlerIn{LogicalAnd,Ternary,Map,Iife,CallArgument}` | none | the handler is not run by the body |
| `{Timer,Listener,Continuation}InTrackedEffect`, `TimerInSettled` | none | nested in a leaf scope |
| `WriteAfterAwaitInHook` (`hook.ts`, `createDataStream`) | none | after an `await` |
| `WriteInCleanup`, `ClosureStoredInMemo` | none | not run during the body |
| `props.settings?.(() => setOpen(false))` in JSX, `untrack(() => props.receive)?.(ask)` | none | handed to a callback prop / a parent, not proven to be called during the call (the second also: a call whose callee is itself a call is no primitive call) |
| `CallbackPropReadsAsyncMemo` | none | an async memo read in a callback prop's closure inside a tracked JSX region is not a render of the pending value (it was `async-outside-loading-boundary`) |
| `WriteInBody`, `HelperCalledInBody`, `WriteInMemoBody`, `WriteInImmediatelyInvokedFunction` | `SC2001` violation | runs in an owned scope |
| `WriteBeforeAwaitInHook` (`hook.ts`, `connect`) | `SC2001` violation | the first statement runs synchronously in the caller |

**Stub.** `solid-js.d.ts` copies `createSignal`, `createMemo`,
`createTrackedEffect`, `onCleanup`, `onSettled` and `untrack` from the published rc.9
typings; only the global JSX namespace is local. `App.tsx` and `hook.ts` pass
`tsc --noEmit` (TypeScript 5.9.3, `strict`) against the stub and against the
real rc.9 install with `jsxImportSource: "@solidjs/web"`.
