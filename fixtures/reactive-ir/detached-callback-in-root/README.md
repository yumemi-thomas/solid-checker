# detached-callback-in-root

**Claim.** An owner-requiring operation in a callback that runs with no owner
on every run is a proven `SC4001` violation when that callback is written
inside a `createRoot` callback, exactly as when it is written inside a
component. The root owns only what runs in its synchronous extent; it does not
answer for a callback it merely contains.

Measured on the published bytes, `solid-js`, `@solidjs/signals` and
`@solidjs/web` `2.0.0-rc.3` and `2.0.0-rc.9`, dev and prod client builds under
jsdom (the rc.3 install is the one `render-effect-apply-root` verified against
`benchmarks/package-contract-v2/phase0/rc3/`). Each callback below was created
inside `createRoot` (the delegated handler inside a `render` callback), and in
each one `getOwner()` is `null`, an `onCleanup` raises `NO_OWNER_CLEANUP` (dev)
and never runs, not even when the root is disposed:

- a `createEffect` apply, which the runtime queues and runs from the flush;
- an event handler, delegated (`addEvent(node, "click", fn, true)`, invoked by
  `@solidjs/web`'s `eventHandler` with a plain `handler.call(node, e)`) or
  native (`addEvent(node, "focus", fn, false)`);
- a `createReaction` invalidation, which runs from the flush;
- a `runWithOwner(null, fn)` callback.

An `onSettled` called in the `createEffect` apply runs out-of-band: its
callback sees `getOwner() === null`, and its returned cleanup raises
`SETTLED_CLEANUP_UNOWNED` (dev) and is dropped (prod).

The owner passes used to answer every call written lexically inside an
owner-providing region as owned without consulting the owner graph, so none of
these was reported. The lexical answer is now withheld wherever a function
between the region and the call is proven unowned in the owner graph, which is
what each of these callbacks is (a `CallbackOwner::None` position, a compiler
event-handler role, a `null` `runWithOwner` owner), and the operation is judged
on the graph (`owners::root_owned_at`).

| Case | Finding | Why |
| --- | --- | --- |
| cleanup in a `createEffect` apply | `SC4001` violation | the apply runs from the flush |
| the namespace spelling | `SC4001` violation | resolves to the same primitives |
| effect created in a `createEffect` apply | `SC4001` violation | the same, for an effect |
| cleanup in an event handler | `SC4001` violation | the handler runs from the event's dispatch |
| cleanup in a `createReaction` invalidation | `SC4001` violation | the invalidation runs from the flush |
| cleanup in a `runWithOwner(null, fn)` callback | `SC4001` violation | `fn` runs with no owner, whatever the call site |
| `onSettled` returning a cleanup in a `createEffect` apply | `SC4001` violation, error | out-of-band: `SETTLED_CLEANUP_UNOWNED` |
| cleanup in a helper declared in the root, called from the root body and from a `createEffect` apply | `SC4001` violation | the apply's invocation is proven unowned; one such invocation is enough |
| cleanup in a root created in a `createEffect` apply | none | the inner root owns what it contains (probed: its cleanup runs on the inner dispose) |
| cleanup in a compiled JSX child in a `createEffect` apply | none | the render effect the compiler generates for the child owns it (probed through `insert`: the cleanup runs on the child's re-run) |
| cleanup in a `createEffect` compute | none | the effect owns its compute |
| `onSettled` returning a cleanup in the root body | none | owned |
| cleanup in a `setTimeout` callback | `SC4001` violation | the callback runs from a task queue with no owner (probed). Once a pinned false negative here; the unowned edge a reviewed fresh-stack scheduler gives its callback is `fresh-stack-scheduler-owner`'s claim |

**Stub.** `solid-js.d.ts` is `render-effect-apply-root`'s, whose declarations
are verbatim from the published rc.3 typings, plus `createReaction`,
`onSettled` and `runWithOwner`, verbatim from the same package. `Owner` is an
empty interface: the only owner any case passes is the `null` literal, which
the published parameter type `Owner | null` accepts either way. `App.tsx`
type-checks cleanly against that stub and against the real
`solid-js`/`@solidjs/signals`/`@solidjs/web` rc.3 and rc.9 typings
(`tsc --noEmit`, TypeScript 5.9.3, `jsxImportSource: "@solidjs/web"`).
