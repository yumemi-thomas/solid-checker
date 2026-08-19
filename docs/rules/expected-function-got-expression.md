# expected-function-got-expression

`SC1007` · **warning** · violation or uncertifiable handler shape

A native event position receives either a reactive handler read that freezes
during setup, or a TypeScript-unchecked value that is not proven to be a valid
runtime handler.

## What it does

Flags native-element event-handler bindings in two exact domains. A callable
value read from reactive props/store state is installed once during DOM setup,
so later updates cannot replace it. Separately, TypeScript deliberately skips
hyphenated JSX attribute names, while Solid still lowers a native `on*` name as
a listener: a proven non-callable, non-array value is a violation, and a mixed,
`any`, or unresolved array/bound-pair shape is uncertifiable. Shared with the
1.x catalog as
[v1/expected-function-got-expression](v1/expected-function-got-expression.md)
under the same code, so a suppression comment survives a migration.

Handler props follow the component's caller classification (see
[strict-read-untracked](strict-read-untracked.md)): `onClick={props.onSave}` on a
native element installs the handler once, which only misbehaves when the prop is
signal-backed. When every visible call site passes the handler statically the
binding is exactly right and stays silent; a proven-reactive handler prop is a
**violation**; an unenumerable component (exported, spread into, referenced
outside JSX) makes the finding **uncertifiable**. When this rule claims a handler
expression, the strict-read finding on the identical span is suppressed — one
defect class, one rule.

Ordinary declared attributes remain TypeScript-owned: `onClick={12}` is TS2322
and receives no SC1007. `on-event={12}` is the checker's domain because
TypeScript does not inspect that name. Callable values, `undefined`, `null`, and
`false` are certified as valid/absent handlers; an array is not guessed because
it may be Solid's `[handler, data]` representation.

## Why is this bad?

A function-expecting position defers execution: Solid calls the function later,
in the scope that gives it meaning. Handing it the result of a reactive
expression evaluates the expression immediately, during setup — the value is
captured once, and whatever reactivity it carried is severed.

## Examples

Examples of **incorrect** code for this rule:

```tsx
function SaveButton(props: { onSave: () => void }) {
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
<button on-event={() => save(id())}>Save</button>;
```

## How to fix

Pass a stable function reference or wrap the operation in a function. For an
uncertain bound-handler value, use an explicit two-slot tuple or a direct
function so the runtime representation is provable.

## Related

- [uncalled-accessor](uncalled-accessor.md) — the inverse defect
