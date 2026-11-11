# leaf-owner-forbidden-call

`SC3001` · **error** · violation · 🛠️ safe fix available for trailing `onCleanup`

A call that requires child ownership or scheduler re-entry occurs inside a
Solid 2.0 leaf owner. The rule covers `onCleanup`, owner-attaching reactive
primitives, and `flush()` inside `createTrackedEffect` or an owner-backed
`onSettled` callback.

Written JSX counts as a creating call (ADR 0883): an intrinsic's dynamic child, attribute
or spread, a fragment's dynamic child and a component element, in the callback, its
parameterless IIFEs or a helper it calls.

## What it does

Leaf owners manage work through their callback's return value and cannot own
children. Consequently, `onCleanup` must be returned, derived primitives such
as `createMemo`, `createRoot`, and function-form `createSignal` cannot attach,
and `flush()` cannot re-enter the flush cycle. `Loading`/`Errored` called as
functions and `resolve`/`until` create an owner too (rc.17 only, measured; ADR
0614; on rc.21 their throw under `createTrackedEffect` is the observer guard's, so there they are
[no-await-primitive-in-tracked-scope](no-await-primitive-in-tracked-scope.md)'s, ADR 0641). Solid throws for all three
operation variants in its development build only: the `@solidjs/signals`
rc.17 production bundle (`dist/prod`) carries no such guard, so there the
call does not throw (ADR 0434). Value-form state such as `createSignal(0)` and
`createStore(object)` is allowed because it attaches no child work.

The callback must be a directly written function literal, an exact function
reference, or a closed local callback return. For the last form, the adapter
must have one unconditional return of a function literal or its exact callback
parameter; the returned function is then checked in its own synchronous extent.
A function named by value in the same file (a `const` or function declaration,
a chain of `const` copies, a receiver-closed object literal's data slot) is
walked as the literal is, each operation reported where it runs in the body,
once however many leaf calls take it (ADR 0918); one in another file is
reported at the argument.
Exactly resolved project helpers are followed transitively: a function or
`const` arrow, an import a path or alias resolves inside the program, and a
method declared in `C` of a `const` bound to `new C(...)`. Conditional,
wrapper-built, package, or otherwise unresolved callbacks (a slot written or
an object that escapes) produce `reactive-dispatch-unresolved` instead of being
guessed. A nested function
merely created by the callback executes outside the leaf extent, except a
callback a project helper calls in its own body (`run(() => createMemo(…))`,
through further such helpers, not in a parameter list or after an `await`):
it runs during the helper call, so an operation written in it is reported
where it is written (ADR 0492). A helper that defers, stores or hands on its
callback is not followed. A tagged template's tag (``f`x` ``) is followed as
a call of `f`, resolved through its declaration (ADR 0538).
`(function* () { … })().next()` runs its first step in place, as an IIFE; U
where a `yield` may stop it first (ADR 0929).

The callback of a primitive the dialect says runs in place is such a layer too
(`Dialect::slot_run`, ADR 0944): `untrack`, `latest`, `isPending`, `flush(fn)`
and `runWithOwner(owner, fn)` keep the leaf as the owner, so `untrack(() =>
onCleanup(…))` is reported at the `onCleanup`, in the leaf's text, in a helper's
body, or in a function named by value (`untrack(cb)`, reported at that call).
Tracking is off in `untrack` and `runWithOwner`, so `resolve` and `until` there
create a root under the leaf (this rule's, not the observer guard's). A call
that takes what its callback throws (`isPending`) leaves a plain throw
(`flush()`, the observer guard) unreported, while a cleanup or a creation the
leaf refuses emits its diagnostic first and is reported. `runWithOwner(null |
undefined, …)` and an owner captured in a component body have no leaf owner
(silent); the `getOwner()` of the leaf's own function is the leaf's (reported);
any other owner value, a primitive attributed without the binder's
confirmation and a callback the walk cannot read are uncertifiable.
`createRoot` is refused at its own call and its callback is not crossed, nor is
a computation's or a deferred one's.

An action called in that extent runs its generator's first step there, under
the leaf owner (ADR 0520): a `const`-held action's body is followed as a
helper's is, up to its first `yield`. A site after a yielded promise runs with
no owner (`missing-owner`'s claim); one after a plain `yield`, on a branch, or
in an action no `const` here holds is an obligation; `flush` in the step throws
`FLUSH_IN_ACTION` first (`flush-in-action`'s claim) (ADR 0550).

A standard-library call in that extent is judged by its exact declaration,
optional (`el?.scrollIntoView()`) or not (ADR 0439). An inline callback
(`forEach`, the `Promise` executor) is guaranteed only where its carrier
provably runs it: over an empty or unknown-length receiver (`items().forEach`,
a `NodeList`) the finding is uncertifiable (ADR 0563). One that dispatches an
event synchronously (`focus`, `click`, `dispatchEvent`, dialog and popover
toggles, `AbortController.abort`, and since ADR 0705 `form.submit`,
`reportError`, `replaceWith`, `replaceChild`, `replaceChildren`,
`History.go`, `Location.assign`/`reload`/`replace`) keeps the obligation,
since a listener it reaches may call `onCleanup`; so does an optional
`getContext`.

`onSettled` becomes a leaf owner only when its call is proven owner-backed.
Out-of-band calls enqueue a plain callback and remain outside this rule; an
unprovable caller or nullable owner is uncertifiable. An `onSettled` called inside
another leaf body is always queued out of band, because that body's owner is
children-forbidden: it is never claimed here, and its callback is judged as
out-of-band (`missing-owner`).

An `onCleanup` in an owned leaf is this rule's claim: the runtime throws
`CLEANUP_IN_FORBIDDEN_SCOPE` there, and it does so wherever the cleanup sits in
the leaf's extent, a helper the leaf calls included. A `createTrackedEffect`
body runs under the tracked node whatever the effect's owner, so its cleanup is
this rule's claim even when the effect itself has no owner. The same site is not
also reported as `missing-owner` (SC4001), which claims only a cleanup that never
runs: one in an `onSettled` callback queued with no owner, or with an owner that
may be absent (ADR 0347).

The operation's identity is proven from the binder (ADR 0711): a named import
whose imported name is the primitive's, or a member of a namespace import of a
Solid module. Any other attribution (a renamed re-export through a project
barrel, an import whose target the producer could not read) is uncertifiable.

A function entered twice under different contexts is walked twice (ADR 0961): the walk's visited set is keyed by how a
body is entered (the entry kind, whether it is guaranteed, the argument standings the call proves, the class `this` is
exact for), so `make(true); make(false)` reads the second entry and `isPending(g); g();` reads the direct call after the
one `isPending` catches; a slot callback is walked on a copy, so its visits never suppress a later direct entry. A cycle
still ends where a body is entered again the same way.

A call written in a callback of a primitive that runs it in place, where the callback takes a parameter, is async or is a
generator (`untrack((p) => onCleanup(…))`, `untrack(async () => { createMemo(…); })`), is an uncertifiable row of this
rule (ADR 0959, the `UnmodelledSlot` operation, blocker `leaf-owner-unproven`): the slot climb does not cross such a
callback, and what it runs is not read. A call that cannot operate is not claimed (a primitive the leaf owner never
refuses, an accessor read, a setter call); any other call is, a helper or a member too. Never a violation.

A site no execution reaches is uncertifiable (ADR 0458). Not yet: a site in a
tracked-effect body whose every creation is inside an owned leaf body, where
the creation itself throws first (`docs/open-work.md`).

## Examples

Incorrect:

```tsx
function Widget() {
  onSettled(() => {
    onCleanup(dispose);
    const label = createMemo(() => count());
    flush();
    console.log(label());
  });
  return <div />;
}
```

Correct:

```tsx
function Widget() {
  const label = createMemo(() => count());
  onSettled(() => {
    console.log(label());
    return dispose;
  });
  return <div />;
}
```

## How to fix

Return cleanup from the leaf callback; a trailing `onCleanup(fn)` has a safe
rewrite to `return fn`. Move owner-attaching primitives to the component body
or another children-capable scope and read their accessors inside the leaf.
Remove `flush()` because leaf callbacks already run during settlement, or move
the write and flush together to the event handler or imperative boundary that
triggered the work.

## Related

- [missing-owner](missing-owner.md) — operations with no owner at all
- [primitive-in-directive-application](primitive-in-directive-application.md) — owner-attaching work in a directive application callback
- [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) — callback bodies whose exact execution cannot be certified
