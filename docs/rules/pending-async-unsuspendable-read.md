# pending-async-unsuspendable-read

`SC5001` · **error** by default · violation

A pending async accessor is read in an execution scope that cannot suspend or
retry. This includes untracked rendering/module evaluation and leaf owners such
as `onSettled` and `createTrackedEffect`.

## What it does

Async computations suspend when read by tracked graph work. An untracked read
has no listener to retry, while a leaf owner runs after settlement and cannot
suspend. Solid therefore throws when the accessor is pending. The leaf-owner
message variant retains **warning** severity; untracked reads retain **error**
severity. Each message names the execution scope that made the read unsafe.

A declared `loadingValue` or store `seedLoadingValue` makes the first flight
safe, but the protection ends after the first real answer. A later refresh or
input change can make the same unsuspendable read throw, so the rule remains
with conditional wording. If the options object cannot be read, the untracked
variant becomes uncertifiable because a declared first-paint value can neither
be proven nor ruled out.

## Examples

Incorrect:

```tsx
const user = createMemo(() => fetchUser(id()));

function Profile() {
  const name = user().name; // untracked component-body read
  onSettled(() => analytics.identify(user().id)); // leaf-owner read
  return <h1>{name}</h1>;
}
```

Correct:

```tsx
const user = createMemo(() => fetchUser(id()));

function Profile() {
  return <h1>{user().name}</h1>; // tracked JSX can suspend
}

createEffect(
  () => user(),
  resolved => analytics.identify(resolved.id),
);
```

## How to fix

Read the accessor in JSX, a memo, or an effect compute function so the graph
can suspend and retry. For work that must run in a leaf callback, settle the
value in a tracked compute phase and pass the resolved value into the apply
phase, or guard the callback until the data is ready. A first-paint value alone
does not make later revalidation reads safe.

## Related

- [async-outside-loading-boundary](async-outside-loading-boundary.md) — tracked async reads without fallback UI
- [leaf-owner-forbidden-call](leaf-owner-forbidden-call.md) — calls forbidden by a leaf owner's lifetime
- [strict-read-untracked](strict-read-untracked.md) — the synchronous untracked-read analogue
