# primitive-in-leaf-owner

`SC3002` · **error** · violation

An owner-attaching reactive primitive (`createMemo`, `createEffect`,
`createRoot`, a derived `createSignal(fn)`, …) is created inside a leaf owner
(`createTrackedEffect`, or an owner-backed `onSettled`).

## What it does

Flags calls that create owner-attaching reactive primitives when they are
lexically contained in a `createTrackedEffect` callback, or in an `onSettled`
callback whose call is proven to execute under a live children-capable owner.

Value-form `createSignal(0)` / `createStore(obj)` are **not** flagged and do not
throw at runtime — they register no work on the owner. Only the derived forms
(`createSignal(fn)`, `createStore(fn, …)`) and owner-attaching primitives such
as `createMemo` throw.

`onSettled` is only a leaf owner when it is called owner-backed. Called
out-of-band (event handler, no owner, inside another leaf scope) the rc.0
runtime enqueues a plain callback and primitives created inside it do not
throw, so this rule stays silent there; an unprovable call site (exported
helper) is reported as **uncertifiable** instead. `createTrackedEffect` is a
leaf owner unconditionally.

## Why is this bad?

Leaf owners own no children. A primitive created inside one has no owner to attach
to: it is never tracked into the graph and never disposed, so its subscriptions
leak and its updates go nowhere. Solid throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` in
dev when this happens.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function Widget() {
  onSettled(() => {
    // Throws in dev: a memo attaches to the owner chain, and a leaf
    // owner cannot own children.
    const label = createMemo(() => `count is ${count()}`);
    console.log(label());
  });
  return <div />;
}
```

Examples of **correct** code for this rule:

```tsx
// Create owner-attaching primitives in the component body; use their
// accessors in the leaf owner.
const label = createMemo(() => `count is ${count()}`);

onSettled(() => {
  console.log(label());
  return () => console.log("disposing");
});

// Value-form signals need no owner and are fine anywhere:
onSettled(() => {
  const [local] = createSignal(false); // does not throw
  void local;
});
```

## How to fix

Create the primitive in the component body (or another owning scope) and read its
accessor inside the leaf owner. If the primitive's lifetime really is tied to the
callback, the logic probably belongs in a computation rather than a leaf owner.

## Related

- [cleanup-in-forbidden-scope](cleanup-in-forbidden-scope.md) — the same constraint for `onCleanup`
- [primitive-in-directive-application](primitive-in-directive-application.md) — the directive analogue
