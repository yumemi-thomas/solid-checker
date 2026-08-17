# v1/no-array-handlers

`SC8007` · **error** · violation

A DOM or custom-element event prop receives an array- or tuple-shaped value.

## What it does

Checks `on:*` and conventional `onEvent` props on lowercase JSX elements. It asks the
compiler to classify the value's type (`arrayShape`), so an imported or aliased
tuple is recognized however it is spelled; without that classification it
conservatively recognizes local array literals and their bindings.

## Why is this bad?

Solid supports the delegated `[handler, data]` shorthand, and `BoundEventHandler`
types its first member as `(data: any, ...e) => void` — `any`, so the data the
handler receives is never checked against the data the tuple carries. The pair
type-checks and then fails when the event is dispatched. The checker therefore
treats this as a type-safety boundary, not merely a style preference, and it is
the only thing that can: the tuple is legal per Solid's own types.

## Examples

Incorrect:

```tsx
type SaveHandler = [(data: Record, event: MouseEvent) => void, Record];
const click: SaveHandler = [save, record];
<button onClick={click}>Save</button>
```

The alias is the point: `click` renders as `SaveHandler`, so nothing about its
spelling reveals the tuple. `tsc` accepts all of this.

Correct:

```tsx
<button onClick={(event) => save(record, event)}>Save</button>
```

## How to fix

Pass a plain function whose parameters and captured data TypeScript can check.
If a tuple abstraction is essential, wrap it behind a function at the JSX
boundary. The rule does not rewrite handlers automatically because it cannot
infer the intended handler signature.

## Configuration

For a project that deliberately accepts the tuple tradeoff, disable only this
rule with `{ "v1/no-array-handlers": { "enabled": false } }` in the project
rule-options document.
