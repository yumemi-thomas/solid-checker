# invalid-refresh-target

`SC7003` · **error** · violation

`refresh()` is called with the wrong number of arguments, or with something other
than the original Solid source binding.

## What it does

Flags `refresh()` calls where the argument count is not exactly one, or where the
target is a call result, wrapper function, literal, or other expression that is not
an identifier bound to a proven Solid source (a derived signal, store, or
projection). The family name says “target,” but this rule also owns malformed
`refresh()` arity because an absent or extra argument prevents a valid target
from being identified.

## Shared code

SC7003 identifies the proven-invalid `refresh()`/`affects()` target family;
the rule name identifies which API surface reported it. A suppression or
filter by code therefore silences both `invalid-refresh-target` and
`invalid-affects-target`. Project-wide enablement uses exact rule names, so
either surface can be disabled without disabling the other.

## Why is this bad?

`refresh()` is Solid 2.0's explicit recompute primitive — the replacement for
`resource.refetch()`. It identifies what to recompute by the brand on the source
binding itself. A read value (`refresh(user())`), a locally re-wrapped function, or
a literal carries no brand, so the runtime cannot resolve a recompute target and
throws.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

refresh(user()); // A read value, not the source.
refresh(() => user()); // A local wrapper — the brand does not pass through.
refresh(user, force); // Wrong arity.
```

Examples of **correct** code for this rule:

```tsx
refresh(user); // The source binding itself.
refresh(() => expensive(query())); // Thunk form: re-runs the expression and returns its value.
```

## How to fix

Pass the accessor or store exactly as returned by its create call — uncalled and
unwrapped. To refresh an ad-hoc expression, use the thunk form
`refresh(() => expr)`.

## Related

- [refresh-target-unresolved](refresh-target-unresolved.md) — when the target cannot be traced
- [invalid-affects-target](invalid-affects-target.md) — the same shape rules for `affects()`
- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — where `refresh()` may be called
