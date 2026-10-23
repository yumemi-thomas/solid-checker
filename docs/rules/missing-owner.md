# missing-owner

`SC4001` · **warning** by default · violation (uncertifiable, reported as an error, when required ownership facts are unresolved)

An owner-requiring operation executes without a reactive owner. The rule covers
effects, cleanup registration, `Loading`/`Errored` boundaries, an `onSettled`
callback that returns cleanup, and a `useContext` read. Its message identifies
the specific operation.

A `useContext(context)` read with no owner is the `context-read` operation (ADR
0352). rc.13's `getContext` checks the owner before it reads, so the read throws
`NoOwnerError` whatever default the context has, and the message says so. The
owner walk places it as it places an effect: an ownerless event handler, a timer callback,
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

The apply callback of `createEffect(compute, apply)` is a detached callback:
the runtime queues it, and it runs from the flush, which does not reset the
ambient owner, so it has no owner **unless the program may call `flush` from an
owned scope** (a `flush()`, `render()`, `hydrate()` or `action(...)` anywhere
but a module-level statement or an apply, or a call spelled `flush` with no
exact callee). Then the queued apply runs under that caller's owner, and the
apply is owned on some runs: an operation in it is uncertifiable, not a
violation, even inside a `createRoot` region (ADR 0362, the same program-wide
check as [primitive-in-effect-callback](primitive-in-effect-callback.md#when-the-program-may-flush-from-an-owned-scope);
the message is the first-run-only one, `later_run_unowned`). The
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
`createEffect` apply, an ownerless event handler, a `createReaction` invalidation, a
`runWithOwner(null, fn)` callback -- is not owned by it, so an operation there
inside a `createRoot` callback is the same proven violation it is inside a
component, and an `onSettled` there is out-of-band. A compiled JSX
child, which the render effect the compiler generates for it owns, answers for
what it contains. A root, memo or effect created directly in an effect apply
does not: on rc.13 the root runs and nothing owns it, and on rc.17 dev the
create call throws `PRIMITIVE_IN_EFFECT_CALLBACK` before it runs (the
production build leaks it), unless a flush from an owned scope drains the
apply under that owner (above). The effect constructors are this rule's claim
(`NO_OWNER_EFFECT`; on rc.17 dev the same effect also throws, which its message
does not say). A root, memo and deferred memo there are
[primitive-in-effect-callback](primitive-in-effect-callback.md)'s (SC4005,
rc.17 only), so the two rules never report one call (ADR 0362).

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

After a possibly skipped suspension, an effect, cleanup or context read may
retain its owner or resume without one (ADR 0357). The result is uncertifiable,
including from an owned leaf: the skip path may instead throw a leaf-scope
error, while the taken path warns or throws with no owner. This context follows
ordinary calls and owner-preserving callbacks. It cannot be answered by a
lexically enclosing root, but a newly created root still owns its callback.
Definite continuations keep their existing proof. Argument-specific take/skip
proofs are not implemented.

JSX event placement alone does not prove an ownerless invocation. A visible
synchronous DOM dispatch runs the handlers of the event types it fires on its
caller's owned stack (ADR 0356, 0361), so such a handler stays uncertifiable:
every ADR 0329 member is a set of event types (`focus()` fires `focus`,
`focusin`, `blur`, `focusout`, `change`; `checkValidity()` fires `invalid`;
`click()` its activation events; `abort()` fires at no element), and
`dispatchEvent(new CustomEvent("sync"))` fires `sync`, on any receiver (a
`click` of a `MouseEvent`, `PointerEvent` or any `…Event` but `Event` and
`CustomEvent` is an activation event and fires `click()`'s set). An
element handler is reached when its own `on`+uppercase type is in the union of
the sets of every dispatch in the program, so a `focus()` call leaves `onFocus`
and `onBlur` handlers uncertifiable and an `onClick` or `onScroll` handler keeps
its ownerless proof. A dispatch whose type is unknown may fire any: an event
argument that is not an inline `new …Event("T")`, `Reflect.apply`/`.call`/an
alias of a dispatching member, a computed key that is not a literal and
not provably no member (`target[name]()` on an untyped callee or with a union
or generic key type; an index typed `number`, a well-known symbol and a typed
callee with a known `lib.dom`-free target set are no member call), an
owner-specific member (`showModal`, `reset`) used through `.call`/`.apply`/`.bind`,
`Document.execCommand`. A member call spelled like an ADR
0329 member on an `any` or unresolved receiver fires that spelling's set (an
exactly resolved project method of that spelling is no dispatch). Bubbling and
the receiver do not matter to the type question. A web release whose runtime was
not read lets every dispatch reach every handler. One shape is narrower: a
`dispatchEvent` of an inline non-bubbling, non-composed `new Event(T)`/`new
CustomEvent(T, init)` with a literal `T` the web release does not delegate, on a
function-local `const` that is one intrinsic element read only through members,
reaches only that element's `T` handlers and its exact spreads' handlers, and
fires no type program-wide. A handler no element names (a forwarded prop, a
literal nested in a handler, a helper flow) stays uncertifiable while any
dispatch exists. No unsupported dispatch edge is guessed.
A `window.dispatchEvent(…)` or `document.dispatchEvent(…)` of the unshadowed
global, whatever the event, reaches no compiler handler: its path
(`[window]`, `[document, window]`) holds no element, and an element handler is
a listener on its element or a property the delegated walk finds from
`e.target` (web rc.13 and rc.17). It joins nothing and withholds nothing; a
callback registered with `window.addEventListener` stays
conditional (ADR 0319), and a function that is also an element handler keeps the
violation its element's own events prove.
An object handler (`onScroll={{ handleEvent, capture: true }}`) can be a
capture listener and is not a proof site: it is neither joined nor proven
unowned, and the delivery reasoning above depends on that.

Exact event members (including tuple slot zero) and intrinsic spreads join the
owner graph through the immutable object-literal census (ADR 0358). On web
rc.13, spread keys beginning with `on` and containing no colon install
listeners, including lowercase keys; on the audited rc.17 only `on` followed by
an uppercase letter does, and `onclick` is an attribute. Unknown holders, getters, generators and
unaudited web spread keys retain internal obligations. These placements obey
the synchronous-dispatch rule above; no unresolved target proves a violation.

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

A package function whose same-operation creation calls cover every exit between them (`if (c) createEffect(…); else createRenderEffect(…);`) requires an owner as one fact (ADR 0359). Any exit without a creation, async/generator bodies, try, loops, `never` calls and member/namespace callees keep the requirement uncertifiable.
