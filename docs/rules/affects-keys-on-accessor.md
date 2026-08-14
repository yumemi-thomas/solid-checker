# affects-keys-on-accessor

`SC7004` · **error** · violation

`affects()` is given a property key, but its target is a signal accessor.

## What it does

Flags two-argument `affects(target, key)` calls where the target resolves to a
signal accessor rather than a store.

## Why is this bad?

The key narrows an invalidation declaration to one property *inside a store*.
A signal accessor is already a single slot, so the key is meaningless. Either
the key is left over from a store refactor, or the wrong
binding is being passed; both deserve a hard error rather than silent acceptance.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [count, setCount] = createSignal(0);

affects(count, "value"); // Signals have no property key.
```

Examples of **correct** code for this rule:

```tsx
affects(count); // Signal target: no key.

const [store, setStore] = createStore({ todos: [], filter: "all" });
affects(store, "todos"); // Store target: key scopes the declaration.
```

## How to fix

Drop the property key for signal targets, or pass the store binding if you meant
to scope invalidation to one store property.

## Related

- [invalid-affects-target](invalid-affects-target.md) — target shape rules
- [affects-target-unresolved](affects-target-unresolved.md) — when the target cannot be traced
