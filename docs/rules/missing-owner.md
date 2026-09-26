# missing-owner

`SC4001` · **warning** by default · violation (uncertifiable, reported as an error, when required ownership facts are unresolved)

An owner-requiring operation executes without a reactive owner. The rule covers
effects, cleanup registration, `Loading`/`Errored` boundaries, and an `onSettled`
callback that returns cleanup. Its message identifies the specific operation.

## What it does

The checker reports an operation when no component, computation, or root owner
dominates it. Typical sites are module scope, bare helpers called from module
scope, and detached callbacks. Exported functions with unseen callers, nullable
`runWithOwner` values, unresolved component identity, and unresolved runtime
allocation paths are uncertifiable rather than proven violations.

The apply callback of `createEffect(compute, apply)` is always a detached
callback: the runtime queues it, and it runs from the flush with no owner. The
apply callback of `createRenderEffect(compute, apply)` is not. Its first run
happens before `createRenderEffect` returns, under the caller's owner, and only
its later runs come from the flush with no owner. An operation in that callback
is therefore owned on the first run whenever the call site is owned, and is
reported as uncertifiable, because whether a later run happens depends on its
compute's sources changing. That holds under a `createRoot` callback, or any
other owner-creating callback, as much as under a component body: the root owns
the first run only. Under an unowned call site every run is unowned, and the
operation stays a proven violation.

An owner-creating callback owns only what runs in its synchronous extent. A
callback written inside it that runs with no owner on every run -- a
`createEffect` apply, an event handler, a `createReaction` invalidation, a
`runWithOwner(null, fn)` callback -- is not owned by it, so an operation there
inside a `createRoot` callback is the same proven violation it is inside a
component, and an `onSettled` there is out-of-band. A root created inside such
a callback, or a compiled JSX child, which the render effect the compiler
generates for it owns, answers for what it contains.

One approximation remains: a nested callback the owner graph gives no owner
edge at all, such as a `setTimeout` callback or a callback handed to a function
the analysis does not model, is still treated as owned by the owner-creating
callback it is written in. A `setTimeout` callback runs with no owner, so that
is a false negative.

The proven `onSettled` cleanup variant has **error** severity because Solid 2.0
throws `SETTLED_CLEANUP_UNOWNED` in development and silently drops the cleanup
in production. Other proven variants are warnings: an effect keeps its
subscriptions, `onCleanup` has nowhere to register, or a boundary subtree can
never be disposed.

## Examples

Incorrect:

```tsx
createEffect(() => syncTheme(theme()));
onCleanup(() => window.removeEventListener("resize", resize));

const orphan = <Loading fallback={<Spinner />}><Profile /></Loading>;

onSettled(() => {
  const timer = setInterval(poll, 5000);
  return () => clearInterval(timer);
});
```

Correct:

```tsx
function App() {
  createEffect(() => syncTheme(theme()));
  onCleanup(() => window.removeEventListener("resize", resize));
  return <Loading fallback={<Spinner />}><Profile /></Loading>;
}

const dispose = createRoot((dispose) => {
  onSettled(() => {
    const timer = setInterval(poll, 5000);
    return () => clearInterval(timer);
  });
  return dispose;
});
```

## How to fix

Create the operation inside a component or computation, or wrap deliberate
standalone setup in `createRoot` and retain its dispose callback. Do not return
cleanup from an ownerless event-handler `onSettled`; tear it down explicitly.
For exported library helpers, describe the ownership requirement in the
package's reactivity contract so callers can be certified.

## Related

- [leaf-owner rules](leaf-owner-forbidden-call.md) — calls made under an owner whose lifetime is nevertheless unsuitable
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — async work that needs an owned loading boundary
- [package-contract-incomplete](package-contract-incomplete.md) — missing external ownership facts
