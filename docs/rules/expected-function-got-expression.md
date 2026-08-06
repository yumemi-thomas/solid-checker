# expected-function-got-expression

`SC1007` · **warning** · violation

A reactive expression is passed where a function is expected, so it is evaluated
once instead of staying live.

## What it does

Flags positions that Solid treats as functions — native-element event-handler
bindings foremost — when they receive an already-evaluated reactive expression
instead of a function. Shared with the 1.x catalog as
[v1/expected-function-got-expression](v1/expected-function-got-expression.md)
under the same code, so a suppression comment survives a migration.

## Why is this bad?

A function-expecting position defers execution: Solid calls the function later,
in the scope that gives it meaning. Handing it the result of a reactive
expression evaluates the expression immediately, during setup — the value is
captured once, and whatever reactivity it carried is severed.

## Examples

Examples of **incorrect** code for this rule:

```tsx
// handler() runs during setup; the result is bound as the handler.
<button onClick={handler()}>Save</button>;
```

Examples of **correct** code for this rule:

```tsx
// The function itself is bound; it runs on click.
<button onClick={handler}>Save</button>;
<button onClick={() => save(id())}>Save</button>;
```

## How to fix

Wrap the expression in a function — `onClick={() => ...}` — or pass the function
reference itself without calling it.

## Related

- [uncalled-accessor](uncalled-accessor.md) — the inverse defect
