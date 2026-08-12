# invalid-affects-target

`SC7003` · **error** · violation

`affects()` is called with the wrong number of arguments, or with something other
than the original Solid source binding.

## What it does

Flags `affects()` calls where the argument count is not one or two, or where the
target is a call result, wrapper function, literal, or other expression that is not
an identifier bound to a proven Solid source.

## Shared code

SC7003 identifies the proven-invalid `refresh()`/`affects()` target family;
the rule name identifies which API surface reported it. A suppression or
filter by code therefore silences both `invalid-affects-target` and
`invalid-refresh-target`. Project-wide enablement uses exact rule names, so
either surface can be disabled without disabling the other.

## Why is this bad?

`affects()` declares which Solid source a function invalidates, so tooling and the
runtime can scope recomputation precisely. Like `refresh()`, it identifies the
source by the brand on the binding itself; a read value or wrapper carries no
brand, so the declaration cannot be resolved and throws.

## Examples

Examples of **incorrect** code for this rule:

```tsx
affects(todos()); // A read value, not the source.
affects(todos, ["items"], extra); // Wrong arity.
```

Examples of **correct** code for this rule:

```tsx
affects(todos); // The source binding itself.
affects(store, ["todos"]); // Store target scoped to specific keys.
```

## How to fix

Pass the accessor or store exactly as returned by its create call — uncalled and
unwrapped. The optional second argument is an array of store keys and is only valid
when the target is a store.

## Related

- [affects-keys-on-accessor](affects-keys-on-accessor.md) — keys on a signal target
- [affects-target-unresolved](affects-target-unresolved.md) — when the target cannot be traced
- [invalid-refresh-target](invalid-refresh-target.md) — the same shape rules for `refresh()`
