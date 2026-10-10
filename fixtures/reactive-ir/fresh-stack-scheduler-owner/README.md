# fresh-stack-scheduler-owner

**Claim.** A callback handed directly to a reviewed fresh-stack host scheduler
runs with no owner, wherever it was scheduled: in a component body, a
`createRoot` callback or at module scope. An owner-requiring operation there
(`onCleanup`, an effect, an `onSettled` returning a cleanup) is a proven
`SC4001` violation.

The schedulers are the analyzer's `FRESH_STACK_SCHEDULERS`
(`rust/crates/solid-reactive-ir/src/runtime_semantics.rs`), matched by the
compiler-selected standard-library declaration: `queueMicrotask`,
`setTimeout`, `setInterval`, `requestAnimationFrame`, `requestIdleCallback`,
`Promise.then`/`catch`/`finally`, `Scheduler.postTask`, and the
`IntersectionObserver`, `ResizeObserver`, `MutationObserver`,
`PerformanceObserver` and `ReportingObserver` constructors. That list is the
host half: each invokes its callback from a task or microtask queue, on an
otherwise empty stack. The owner half is the dialect's
(`Dialect::fresh_stack_callback_owner`): Solid 2.0's owner is a synchronous
dynamic scope, so none is current on an empty stack, and the callback gets a
`CallbackOwner::None` owner edge (`owners::fresh_stack_scheduler_edges`).

Measured on the published bytes, `solid-js`/`@solidjs/signals` `2.0.0-rc.3`
(the kobalte sweep install, the same rc.3 bytes as
`benchmarks/package-contract-v2/phase0/rc3/`) and `2.0.0-rc.9`, dev and prod
builds, loaded through an import map in headless Chromium 153, for every one of
the fourteen schedulers, each scheduled inside a `createRoot`, inside a
`createMemo` compute (the owner a component body runs under) and at module
scope: `getOwner()` is `null` in the callback; an `onCleanup` there raises
`NO_OWNER_CLEANUP` (dev) and never runs, not even when the root is disposed; a
`createEffect` there raises `NO_OWNER_EFFECT` (dev); an `onSettled` returning a
cleanup raises `SETTLED_CLEANUP_UNOWNED` (dev) and the cleanup never runs.
Control: a `createRoot` created in the callback owns its `onCleanup`, which
runs on that root's disposal.

| Case | Finding | Why |
| --- | --- | --- |
| cleanup in a `setTimeout` callback at module scope | `SC4001` violation | no owner at the call, none in the callback |
| cleanup in a `setTimeout` callback in a `createRoot` callback | `SC4001` violation | the root owns only its synchronous extent |
| cleanup / effect / `onSettled` returning a cleanup in a `setTimeout` callback in a component | `SC4001` violation (the `onSettled` one an error) | the callback runs from the task queue |
| the namespace spelling of `onCleanup` | `SC4001` violation | resolves to the same primitive |
| a named function handed to `setInterval` | `SC4001` violation | the identifier is the callback |
| cleanup in the callback of each other scheduler in a component | `SC4001` violation | the same, per scheduler |
| a root created in a timer callback | none | the inner root owns what it contains |
| a timer callback in a root that only reads | none | no owner requirement |
| `onCleanup(() => clearInterval(id))` in the component body | none | owned by the component |
| cleanup in a `PromiseLike.then` callback | none | not a fresh-stack scheduler: a thenable may call back synchronously |
| cleanup in an `addEventListener` listener | none | not a fresh-stack scheduler: a synchronous `dispatchEvent` runs it on the dispatcher's stack |
| cleanup in a callback of a local function named `setTimeout` | none | not the standard-library declaration: it runs the callback inline, under the component's owner |
| cleanup in `setTimeout(wrap(() => ...))` | none | **not claimed, a known false negative**: the scheduler receives what `wrap` returns, and the arrow inside may run on `wrap`'s own stack, so it gets no owner edge |

`Scheduler.postTask` has no case: TypeScript 5.9's `lib.dom.d.ts` declares no
`scheduler`, so no program here can select that declaration.

The read-only control is scheduled from a root. A signal read in a timer
callback scheduled from a *component body* is not a strict-read violation
either; `fresh-stack-scheduler-read-role` owns that claim.

**Stub.** `solid-js.d.ts` is `detached-callback-in-root`'s, whose declarations
are verbatim from the published rc.3 typings. `App.tsx` type-checks cleanly
against that stub and against the real `solid-js`/`@solidjs/signals`/
`@solidjs/web` rc.3 and rc.9 typings (`tsc --noEmit`, TypeScript 5.9.3,
`jsxImportSource: "@solidjs/web"`).
