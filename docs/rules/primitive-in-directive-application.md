# primitive-in-directive-application

`SC6001` · **warning** · violation

An owner-attaching reactive primitive is created inside a directive's apply
callback — the function a directive factory returns for each element.

## What it does

Flags creations of primitives that register a computation an owner would have
to dispose (`createMemo`, `createEffect`, `createRoot`, function-form
`createSignal`/`createStore`, …) inside callbacks that the compiler
recognizes as `ref`/directive applications, including callbacks returned from
a directive factory.

Value-form state (`createSignal(0)`, `createStore({...})`) is **not**
flagged: it allocates plain state that needs no owner and misbehaves in no
way.

## Why is this bad?

Directives in Solid 2.0 compose through `ref` with a two-phase factory
pattern: the factory body (setup) runs in an owned scope, while the returned
callback (apply) runs once per element **with no owner** — `@solidjs/web`'s
`ref()` invokes it under `runWithOwner(null, ...)` (rc.0). A computation
created there is never attached to the ownership graph and never disposed: it
leaks for every element the directive is applied to. The dev runtime answers
with the same `NO_OWNER_*` warnings as any other unowned creation (probed on
the rc.0 dev bundle — a `createEffect` in the apply phase warns
`NO_OWNER_EFFECT`; nothing throws), which is why this rule is a warning like
the [no-owner-effect](no-owner-effect.md) family, not an error: the leak is
real and proven, but the runtime keeps working.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const tooltip = (options) => (el) => {
  // Apply phase: unowned, runs per element.
  createEffect(
    () => visible(),
    (on) => (on ? show(options) : hide()),
  ); // Never disposed: leaks once per element.
};
```

Examples of **correct** code for this rule:

```tsx
const tooltip = (options) => {
  // Setup phase: owned scope — computations and subscriptions live here.
  const [visible, setVisible] = createSignal(false);
  createEffect(
    () => visible(),
    (on) => (on ? show(options) : hide()),
  );
  // Apply phase: DOM work only.
  return (el) => {
    el.addEventListener("mouseenter", () => setVisible(true));
    el.addEventListener("mouseleave", () => setVisible(false));
  };
};

<button ref={tooltip({ content: "Save" })}>Save</button>;
```

```tsx
const marker = () => (el) => {
  // Value-form state in the apply phase is fine: nothing to dispose.
  const [seen] = createSignal(el.dataset.seen === "true");
  el.classList.toggle("seen", seen());
};
```

## How to fix

Use the two-phase factory: create computations and subscriptions in the
factory body, and keep the returned callback to DOM reads, writes, and
listener wiring only.

## Related

- [primitive-in-leaf-owner](primitive-in-leaf-owner.md) — the leaf-owner analogue
- [no-owner-effect](no-owner-effect.md) — the same unowned-leak class outside directives
