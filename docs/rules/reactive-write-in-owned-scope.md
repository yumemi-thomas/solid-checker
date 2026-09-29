# reactive-write-in-owned-scope

`SC2001` · **error** · violation

A signal or store setter (or `refresh()`) is called inside an owned scope — a
component body, a `createRoot` body, or a children-capable computation.

## What it does

Flags calls to setters returned by `createSignal`/`createStore` and to `refresh()`
when they execute under a live children-capable owner: a component body, a
`createRoot` body, a memo, or an effect's compute function. Writes are allowed
in event handlers, actions, `createEffect` apply callbacks, directive apply
callbacks, and the children-forbidden leaf scopes `onSettled` and `createTrackedEffect` — the runtime's write guard
explicitly exempts leaf imperative scopes.

A `createRenderEffect` apply callback is only partly such a scope. Its first run
happens before `createRenderEffect` returns, under the caller's owner, so a
write there throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a component body, a
`createRoot` body or a computation exactly as a write at the call site would.
That run is withheld, though, when the call passes `defer` or `schedule`, when the compute returns a
promise or reads a source that is still pending, and (on rc.9) when the first
pass was staged into a transaction; it then runs later from the flush, where
the write is legal, as every later run is. The checker cannot prove the compute
settles synchronously, so it reports nothing for such a write rather than
claiming either answer. A `createRenderEffect` created where writes are legal
(an event handler, module scope) leaves every run of its apply legal.

A `createRoot` body is not a write region either. It runs during the call
with the new root as the ambient owner, and a root is a children-capable
owner, so a signal setter or `refresh()` directly in the body throws
`REACTIVE_WRITE_IN_OWNED_SCOPE` wherever the root is created: at module scope,
in a component, in a memo compute, or in an effect apply. (Probed on every
published `2.0.0-rc.0` to `rc.9` triple, dev client builds.) A **store** setter
there depends on the installed `@solidjs/signals`. `rc.1` through `rc.8` exempt
a root from the store setter's guard (`!context._root`), so the write is legal
and not reported. `rc.9` removed the exemption, so it throws and is reported.
For `rc.0`, or a signals release this checker has not read, it is not reported.
Callbacks nested in the body keep their own answer: an effect apply,
`onSettled` or an event listener inside the root stays legal, and a memo compute
inside it is reported as any memo compute is. A function passed to `createRoot`
by name, `createRoot(init)`, is the root body exactly as an inline arrow is; it
is resolved by the argument's symbol to a `function` declaration or an
arrow-bound `const` in the same file, so an `init` parameter never stands for a
same-named module function. A root body taken from another module, or from a
value the checker cannot resolve to one declaration, is not judged as a root
body.

The same exemption covers component bodies. In dev builds `solid-js` runs every
component body under `createRoot(…, { transparent: true })`, so a store setter
directly in a component body, or reached from it through `untrack`,
`flush(fn)` or a helper it calls, answers exactly as one in a `createRoot` body
does: legal on `rc.1` through `rc.8` and not reported, reported on `rc.9`
(probed on every published triple). A signal setter there is reported on every
release, and a store setter in a memo compute inside the component is reported
as any memo compute is. On `rc.0` the dialect keeps the exemption, because its
`createStore` setter throws under a root while its `createOptimisticStore`
setter does not, and a write carries only "a store setter": the `createStore`
case there is a miss, not a claim.

`rc.0`'s `createOptimisticStore` setter meets no owned-scope guard anywhere:
its writes take the optimistic engine's path, and `rc.1` is the first release
whose store setters are guarded at their entry. So on an installation that
resolves `@solidjs/signals@2.0.0-rc.0`, a `createOptimisticStore` setter
(plain or derived) in a memo or effect compute is legal and not reported, and a
store setter the checker cannot trace to `createStore` (an alias it knows only
by type) is not reported either, since it may be the optimistic one. A setter
traced to `createStore` is reported there as on every release (probed on every
published `rc.0`-`rc.9` dev client build).

`flush(fn)` is not a write region either. It runs `fn` inline between a
scheduler depth increment and the drain, keeping both the owner and the
listener, so a write in it is exactly as legal as at the `flush` call: it
throws in a memo compute, a component body or a root body and is legal at
module scope or in an event handler (probed on every published triple). The
callback may be inline or a same-file function passed by name; a function that
also runs from other positions is reported when any `flush` or `untrack` call
passing it is in an owned scope.

`untrack` is **not** an allowed write region. The `2.0.0-rc.0` guard keys on the
ambient *owner*, not on tracking: `untrack` clears the tracking listener but
keeps the owner context, so a write inside `untrack(...)` within a component
body, memo, or effect compute still throws `REACTIVE_WRITE_IN_OWNED_SCOPE` at
runtime, while the same `untrack` write inside an event handler is fine because
no owner is live there. (The upstream RFC text claims writes in `untrack` blocks
are allowed; the published rc.0 runtime contradicts it — solid-checker follows
the runtime and reports these writes.)

Internal reactive sources created with `{ ownedWrite: true }` in their
source-creation options are exempt.

A function literal written in a component body runs in that body only when
something invokes it there, so a write in one is reported only when that
invocation is proven. It is proven for:

- a literal handed to a project function whose own body calls that parameter
  during the call;
- a standard-library inline callback (`[x].forEach(fn)`);
- an IIFE;
- a primitive's or a control-flow component's callback;
- a named closure whose call site is itself in the body.

Anywhere else the write takes the role its invocation sites prove, and with
none nothing is reported. That covers a literal handed to a function that
stores it (`later(() => setCount(1))` with a timer calling it), a closure a
callback returns (`keep(() => () => setCount(1))`), and a literal handed to a
package export no accepted contract describes. The write may throw, if the
function calls it during the body, or be legal, if it runs later. An unproven
write position is never claimed as a violation. The matching read there is
`strict-read-untracked` uncertifiable
(`fixtures/reactive-ir/write-in-deferred-callback`).

This is the static counterpart of Solid's dev-mode `REACTIVE_WRITE_IN_OWNED_SCOPE`
error.

## Why is this bad?

Writing under a children-capable owner creates feedback loops: the write
invalidates state the surrounding graph may depend on, which re-runs the scope
that performed the write. Solid 2.0 makes this a dev-mode error because such
loops are almost always a derivation expressed imperatively — and the 1.x
behavior of silently tolerating them hid real bugs. Leaf scopes are exempt
because they own no children to re-trigger: `createTrackedEffect` and
owner-backed `onSettled` run after the graph settles and are the intended home
for imperative writes.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [doubled, setDoubled] = createSignal(0);
// A derivation written imperatively — throws in dev.
createMemo(() => setDoubled(count() * 2));

// A root body runs under the root, a children-capable owner.
createRoot(() => setCount(1));

// flush(fn) keeps the memo's owner, inline or by name.
createMemo(() => flush(() => setCount(2)));

function Counter() {
  setCount(0); // Write in a component body.
  // untrack does not help: the owner context survives it, and this
  // still throws REACTIVE_WRITE_IN_OWNED_SCOPE at runtime.
  untrack(() => setCount(0));
  return <span>{count()}</span>;
}
```

Examples of **correct** code for this rule:

```tsx
// Derive instead of writing back:
const doubled = createMemo(() => count() * 2);

// Imperative writes belong in imperative scopes:
<button onClick={() => setCount((c) => c + 1)}>+1</button>;

// Leaf scopes are legal write regions — the guard exempts them:
createTrackedEffect(() => {
  setLastSeen(count());
});
onSettled(() => setReady(true));

// Internal reactive sources that must be written from owned scope opt in narrowly:
const [element, setElement] = createSignal(null, { ownedWrite: true });
```

## How to fix

First ask whether the write is a derivation in disguise — if the new value is
computed from other reactive values, replace compute-then-set with a `createMemo`.
Genuinely imperative writes move to an event handler, an `action`, the apply
function of `createEffect(compute, apply)`, or a leaf scope (`onSettled`,
`createTrackedEffect`). Wrapping the write in `untrack` does not fix it — the
guard keys on the owner, not on tracking. Reserve the source creation option
`{ ownedWrite: true }` for internal reactive sources such as element refs;
using it on application state reintroduces the feedback loops this rule prevents.

## Related

- [action-called-in-owned-scope](action-called-in-owned-scope.md) — the same constraint for actions
