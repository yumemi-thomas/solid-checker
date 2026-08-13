# v1/no-react-deps

`SC8010` · **warning** · violation

`createEffect` or `createMemo` receives a React-style dependency array.

## What it does

Reports exactly two-argument calls whose second argument is an array literal or
a binding initialized from one. Three-argument calls are left alone because
Solid's own overloads use later arguments for options. The rule resolves aliases
of Solid primitives instead of relying only on the callee's spelling.

## Why is this bad?

Solid discovers dependencies from reactive reads during the tracked callback.
The second argument is an initial previous value, not a dependency list, so a
React-shaped array changes callback state without controlling tracking. Readers
may believe updates are restricted when they are not.

## Examples

Incorrect:

```ts
createEffect(() => sync(user()), [user]);
const deps = [query];
createMemo(() => search(query()), deps);
```

Correct:

```ts
createEffect(() => sync(user()));
createMemo(() => search(query()));
```

## How to fix

Remove the array; the checker offers that deletion as a safe fix. If tracking
must be narrowed or made explicit, use Solid 1.x's `on(source, callback)` helper
inside the reactive primitive rather than a dependency array.

## Related

- [strict-read-untracked](strict-read-untracked.md) — reads outside tracking
- [missing-effect-function](missing-effect-function.md) — invalid effect shape
