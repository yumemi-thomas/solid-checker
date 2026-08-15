# invalid-refresh-target

`SC7003` · **error** · violation

`refresh()` is called without a target, or with something other than an
original, refreshable Solid source.

## What it does

Flags `refresh()` calls with no argument, calls whose target is a call result,
wrapper function, or literal that provably carries no source brand, calls
whose member-chain target is rooted at a signal accessor, and calls targeting
a **value-form** store (`createStore(obj)` / `createOptimisticStore(obj)`) or
one of its child records — such a store has no computation to re-run, and the
runtime throws `INVALID_REFRESH_TARGET` in dev.

Member-expression targets whose chain is rooted at a **function-form** store,
projection, or optimistic store are accepted: every child record read through
a store proxy carries the brand, so `refresh(state.user)` on a derived store
is legal. Extra arguments after the target (`refresh(source, force)`) are
silently ignored by the runtime and are **not** flagged. A target whose chain
root cannot be traced to a Solid source is reported as
[refresh-target-unresolved](refresh-target-unresolved.md) instead — the
checker never guesses a brand's absence.

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
throws. A value-form store carries the brand but owns no compute node — there is
nothing to re-run, and the runtime throws the same error.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));
const [settings] = createStore({ theme: "dark" });

refresh(user()); // A read value, not the source.
refresh(() => user()); // A local wrapper — the brand does not pass through.
refresh(settings); // A value-form store has no computation to re-run.
refresh(settings.theme); // Neither do its child records.
refresh(); // No target at all.
```

Examples of **correct** code for this rule:

```tsx
const [profile] = createStore(() => deriveProfile(id()), emptyProfile);
const view = createProjection(project, seed);

refresh(user); // The source binding itself.
refresh(profile); // Function-form stores own a compute node.
refresh(profile.contact); // Child records of a refreshable store are branded.
refresh(view);
refresh(user, true); // Extra arguments are ignored by the runtime.
```

## How to fix

Pass the accessor or refreshable store exactly as returned by its create call —
uncalled and unwrapped. Refreshable stores are the derived forms:
`createStore(fn, initial)`, `createProjection`, and function-form
`createOptimisticStore`. Update a value-form store through its setter, or move
the derivation into the function form and refresh that binding.

## Related

- [refresh-target-unresolved](refresh-target-unresolved.md) — when the target cannot be traced
- [invalid-affects-target](invalid-affects-target.md) — the same shape rules for `affects()`
- [reactive-write-in-owned-scope](reactive-write-in-owned-scope.md) — where `refresh()` may be called
