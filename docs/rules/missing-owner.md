# missing-owner

`SC4001` · **warning** by default · violation (uncertifiable, reported as an error, when required ownership facts are unresolved)

An owner-requiring operation executes without a reactive owner. The rule covers
effects, cleanup registration, `Loading`/`Errored` boundaries, an `onSettled`
callback that returns cleanup, and a `useContext` read. Its message identifies
the specific operation.

A `useContext(context)` read with no owner is the `context-read` operation (ADR
0352). rc.13's `getContext` checks the owner before it reads, so the read throws
`NoOwnerError` whatever default the context has, and the message says so. The
owner walk places it as it places an effect: an event handler, a timer callback,
module scope, code after an `await`, or a `runWithOwner(null)` callback is
unowned. A package hook that reads a context is followed through its source, and
the finding sits at the project call that runs it, as for any package owner
requirement (ADR 0290). A context with no provider throws `ContextNotFoundError`
instead, which is [missing-context-provider](missing-context-provider.md)'s claim,
not this rule's.
A callback of an `onSettled` called inside another leaf body runs with no owner
(rc.13 queues it), so an `onCleanup` there is this rule's claim.

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

A callback handed directly to a host scheduler that runs it from a task or
microtask queue -- `setTimeout`, `setInterval`, `queueMicrotask`,
`requestAnimationFrame`, `requestIdleCallback`, `scheduler.postTask`,
`Promise.prototype.then`/`catch`/`finally`, and the `IntersectionObserver`,
`ResizeObserver`, `MutationObserver`, `PerformanceObserver` and
`ReportingObserver` callbacks -- also runs with no owner, wherever it was
scheduled: in a component body, a computation, a `createRoot` callback or at
module scope. An `onCleanup`, an effect or a cleanup-returning `onSettled` there
is a proven violation. The scheduler is identified from the compiler-selected
standard-library declaration, never from its name. A `PromiseLike.then` (any
thenable, which may call back synchronously), an `addEventListener` listener
(a synchronous `dispatchEvent` runs it on the dispatcher's stack) and a
Geolocation callback are not in that list.

Package owner requirements must come from exact analyzed source or dialect
runtime premises. A call proved to allocate without an owner is a violation;
unresolved execution or ownership remains uncertifiable. No contract or receipt
can supply the missing behavior.

A ref expression on an intrinsic element runs in two places. The value a ref
application receives (the expression itself, or an element, a conditional branch,
an operand of `||` or `??`, or the right operand of `&&`) runs with no owner: a
violation. The left operand of `&&` is only tested, since a function is always
truthy, so it is not applied. Code a call in the expression evaluates runs while
the component renders, under the owner of the code around the expression: an IIFE,
the body of a factory the expression calls, a default parameter initializer, and
a callback a dialect slot runs during its call (`untrack`) are owned, so an
effect there is not reported. Two placements change that. A call inside a
component's children runs inside the component's own memo, so it is
uncertifiable. A call made at module initialization, where no owner is current,
runs with none, so it is a violation. A call whose enclosing function no owner
edge reaches (a method or constructor no resolved call names, a getter, an
exported function that no namespace import hands out in a closed program) is uncertifiable. A
generator never runs when called, and a callback no dialect slot describes may
run with or without an owner. A function the expression evaluates without
yielding it (a getter's body, a tagged template's substitution, an operand of
`void`) is not placed. Each of these uncertain cases is uncertifiable, never a
violation (ADR 0345).

Two approximations remain. A nested callback the owner graph gives no owner
edge at all, such as a callback handed to a function the analysis does not
model, or handed to a scheduler through a wrapper call
(`setTimeout(wrap(() => ...))`), is still treated as owned by the
owner-creating callback it is written in, and is not reported inside a
component. Where it runs with no owner, that is a false negative.

An invoked function literal is an owner edge (ADR 0347): the call runs its body
under the owner of the code around the call, so a plain IIFE at module scope or
inside an unowned callback is a violation, and one in a component body is owned.
The call forms that run the literal are a direct call, `.call` and `.apply` on
the literal, `new` on a plain function expression (an arrow, an async function
or a method throws before its body runs), a comma expression whose last operand
is the literal (`(0, () => …)()`), and a static field or static block
initializer, which runs at class evaluation under the code around the class.
Withheld, and so uncertain or silent: a generator (calling it runs no body), and
a call placed under another owner, which is an instance field initializer (it
runs at construction) or JSX in the owner's own body (a child or an attribute).
The code after an async IIFE's first `await` is a continuation with no owner
(ADR 0333), as for any async function. An operation in a literal that a
synchronous guard invokes is guarded: `if (getOwner()) (() => { … })()` runs
only under an owner, so it is not reported; the guard must be Solid's
`getOwner`, and no `await` may separate it from the operation (ADR 0347).

In a closed program (a project whose boundary is declared closed, or whose manifest publishes nothing), an export the program reads by name has a caller the analysis cannot see (ADR 0347). A name is read by a static member read `ns.fn` used as a value rather than called, or by a dynamic import's named read: `const { fn } = await import(x)`, `(await import(x)).fn`, or `lazy(() => import(x))`, which reads its `default` export, or the `export` option of a `lazy` call that is Solid's. Such an export is seeded as an open program's export is: a detached callback it passes to `runWithOwner(null, …)` is a violation, and a requirement in its own body is uncertifiable. An export that no read names stays dormant, unless its module's namespace escapes with no readable name: passed along, spread, given to `Object.keys` or `Object.entries`, bound to an alias, read by an element access, re-exported with `export * as`, loaded by `require`, or imported in a shape the census does not name. Such an escape could reach any export of the module, so an export only that escape reaches is uncertain: a detached callback in it is uncertifiable, not a violation (ADR 0347). The one case this leaves unreported is `Object.entries(ns)` whose values the program calls; see `docs/open-work.md`.

A cleanup registered in a leaf whose leaf is itself owned runs in that leaf, which `leaf-owner-forbidden-call` reports, so it is not reported here. A `createTrackedEffect` body runs under its tracked node whatever the effect's owner, so its cleanup is that rule's claim too. A cleanup in an `onSettled` callback queued with no owner never runs, and that claim stays here (ADR 0347).

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
For exported library helpers, keep the implementation available to source
analysis; unresolved callers and ownership remain uncertifiable.

## Related

- [leaf-owner rules](leaf-owner-forbidden-call.md) — calls made under an owner whose lifetime is nevertheless unsuitable
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — async work that needs an owned loading boundary
- [package-behavior-unavailable](package-behavior-unavailable.md) — missing external ownership facts

## Which callback an argument names

A callback passed to a primitive, a handler or a ref is the function the argument is: a function literal, a bare identifier, element 0 of an event handler's tuple, or the literal an identity wrapper hands back (`wrap(() => …)`, where `wrap` returns its parameter unchanged on every completion and reads it no other way; ADR 0346). A TypeScript wrapper keeps the identifier's identity (`h as X`, `h!`, `h satisfies X`), and a wrapper around the identity wrapper's call is that call. A declaration written later in its module (`wrap = function …`) is no wrapper. A call, an IIFE or `untrack(…)` names no callback: its value is not the literal it contains.
