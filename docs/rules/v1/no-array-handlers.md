# v1/no-array-handlers

`SC8007` · **error** · violation

A DOM or custom-element event prop receives an array- or tuple-shaped value.

## What it does

Checks `on:*` and conventional `onEvent` props on lowercase JSX elements. It
uses TypeScript's resolved type when available, so an imported or aliased tuple
is still recognized; without type evidence it conservatively recognizes local
array literals and their bindings.

## Why is this bad?

Solid supports the delegated `[handler, data]` shorthand, but a broadly typed
array does not prove that its first element is callable or that its data matches
the handler. Invalid tuples can compile through `unknown[]` or loose inference
and then fail when the event is dispatched. The checker therefore treats this
as a type-safety boundary, not merely a style preference.

## Examples

Incorrect:

```tsx
const click: unknown[] = [save, record];
<button onClick={click}>Save</button>
```

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
