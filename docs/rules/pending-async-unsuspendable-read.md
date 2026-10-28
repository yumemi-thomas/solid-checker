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

A pending read in a callback a host API retains (an `addEventListener`
listener, a `bind` bound argument, a `PromiseLike.then` callback, a Geolocation
callback) written in a component body is also uncertifiable: invoked inside the
body's strict-read window it throws `PENDING_ASYNC_UNTRACKED_READ` in dev, and
invoked later it throws a plain `NotReadyError` (probed on rc.3 and rc.9), and
the checker does not prove which.

A callback stored by assignment inside a component also has an open invocation
context. Lexical nesting alone does not prove it runs during the component's
strict-read window. These reads are uncertifiable; their wording does not
assert a runtime exception. Retained local callbacks remain negative controls.

The same uncertainty applies when a project or unknown external helper receives
the callback. Unless its exact body or dependency source proves invocation
during the call, the checker cannot prove the callback runs in the lexical
scope or leaves pending values unhandled. Such results are uncertifiable and
their wording identifies the open callback context. Exact cross-file and
namespace helpers that invoke the parameter directly retain the violation.

Native `isPending(() => accessor())` suppresses the strict pending-accessor
safeguard, so it has no `SC5001` error. From `@solidjs/signals` rc.17 so does
`latest(() => accessor())`: both reads go to `verdictValue` before the throw
(`dist/dev-shared.js:5258-5281`), which only warns; on rc.13 and older
`latest` still throws `PENDING_ASYNC_UNTRACKED_READ`. This exemption resolves the exact
primitive and its direct callback: shadowed functions do not inherit it.
Subscription findings retain their own facts, and an ordinary `NotReadyError`
may still propagate as graph suspension. Store proxy reads follow a different
runtime path and do not inherit this accessor exemption.

An effect or render-effect apply that reads an async source is not claimed
here (ADR 0467). rc.17 throws `PENDING_ASYNC_UNTRACKED_READ` when a run of the
apply meets the source pending, and warns `STRICT_READ_UNTRACKED` when it meets
it settled; whether the first run comes before the source settles depends on a
mount held by async anywhere above it, a compute that waits on async, or a later
run, none of which a local fact orders. The read is reported as a
[strict-read-untracked](strict-read-untracked.md) violation whose message names
the pending case (`fixtures/reactive-ir/strict-read-async-apply`).

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
