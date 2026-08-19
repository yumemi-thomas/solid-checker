# v1/expected-function-got-expression

`SC1007` · **warning** · violation or uncertifiable handler shape

A native event position receives either a reactive handler read that freezes
during setup, or a TypeScript-unchecked value that is not proven to be a valid
runtime handler.

## What it does

Flags a native event-handler binding that reads its callable value through
reactive props or store state. Solid installs that value once during DOM setup,
so later reactive updates cannot replace the handler. A prop every proven call
site passes statically stays silent; unresolved prop backing is
**uncertifiable**.

An uppercase function name does not by itself prove component execution. When
no JSX call site or exact `Component` type resolves the function, a handler
read through its possible props is also **uncertifiable**.

The old call-result arm is deliberately absent. `onClick={handler()}` is valid
when the accessor holds a function: the JSX expression tracks the accessor and
updates the installed handler. A non-callable result is already TypeScript's
diagnostic.

There is one TypeScript-unchecked event domain: JSX attribute names containing
a hyphen. The 1.x compiler still lowers every native `on` prefix as a listener,
so `on-event={12}` is a proven SC1007 violation even though TypeScript says
nothing. A callable/non-callable union, `any`, or unresolved array shape is
uncertifiable because the value may instead be a function or Solid's valid
`[handler, data]` pair. Callable and absent values remain clean. Assertions are
peeled before this runtime classification.

## Why is this bad?

A function-expecting position defers execution: Solid calls the function later,
in the scope that gives it meaning. Handing it the result of a reactive
expression evaluates the expression immediately, during setup — the value is
captured once, and whatever reactivity it carried is severed.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function SaveButton(props) {
  // A reactive prop getter is consumed once while the listener is installed.
  return <button onClick={props.onSave}>Save</button>;
}

const invalid = 12;
<button on-event={invalid}>Save</button>;
```

Examples of **correct** code for this rule:

```tsx
// The function itself is bound; it runs on click.
<button onClick={handler}>Save</button>;
<button onClick={() => save(id())}>Save</button>;
<button onClick={handlerAccessor()}>Save</button>;
```

## How to fix

Pass a stable function reference or wrap the action in a function. If replacing
the handler reactively is intentional, render through a tracked structure that
recreates the binding rather than reading a props/store getter during listener
setup.

## Related

- [v1/uncalled-accessor](./uncalled-accessor.md) — the inverse defect
