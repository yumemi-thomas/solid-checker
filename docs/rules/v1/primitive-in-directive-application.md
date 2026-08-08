# v1/primitive-in-directive-application

`SC6001` · **error** · violation

A reactive primitive is created inside a directive application callback — a
`use:` directive function or a callback passed to `ref`.

## What it does

Flags creations of reactive primitives (`createSignal`, `createMemo`,
`createEffect`, …) inside callbacks that the compiler recognizes as
`ref`/directive applications, including callbacks returned from a directive
factory.

## Why is this bad?

The application callback is the per-element half of a directive: Solid calls it
once for every element the directive is applied to, as an unowned leaf. Primitives
created in that phase have no owner — they are never tracked into the graph and
never disposed, leaking once per element.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// use:tooltip runs this function once per element, as an unowned leaf.
function tooltip(el, options) {
  const [visible, setVisible] = createSignal(false); // Never tracked or disposed.
  el.addEventListener("mouseenter", () => setVisible(true));
  el.addEventListener("mouseleave", () => setVisible(false));
}

<button use:tooltip={{ content: "Save" }}>Save</button>;
```

Examples of **correct** code for this rule:

```tsx
const tooltip = (options) => {
  // Setup phase: owned scope — primitives and subscriptions live here.
  const [visible, setVisible] = createSignal(false);
  createEffect(() => (visible() ? show(options) : hide()));
  // Apply phase: DOM work only.
  return (el) => {
    el.addEventListener("mouseenter", () => setVisible(true));
    el.addEventListener("mouseleave", () => setVisible(false));
  };
};

<button ref={tooltip({ content: "Save" })}>Save</button>;
```

## How to fix

Use the two-phase directive factory: create primitives, computations, and
subscriptions in the setup phase — the factory body, which runs in an owned
scope — and keep the returned `ref` callback to DOM reads, writes, and listener
wiring only.

## Related

- [v1/primitive-in-leaf-owner](./primitive-in-leaf-owner.md) — the leaf-owner analogue
