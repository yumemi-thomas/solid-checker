# pending-async-untracked-read

`SC5001` · **error** · violation

An async accessor that may still be pending is read outside a tracking scope.

## What it does

Flags reads of accessors returned by async computations (a `createMemo` whose
callback returns a Promise or AsyncIterable, and similar) when the read happens
outside every tracking scope — in a component body, an effect apply callback, or a
plain helper.

This is the static counterpart of Solid's dev-mode `PENDING_ASYNC_UNTRACKED_READ`
error.

## Why is this bad?

When an async value is pending, a tracked read suspends: the graph waits, the
nearest `<Loading>` boundary shows its fallback, and the read re-runs once the
value settles. An untracked read has none of that machinery — there is nothing to
suspend and nothing to retry, so Solid throws.

**Sources with a declared first paint still get this finding, with conditional
wording.** A `loadingValue` (or store-family `seedLoadingValue: true`) node is
born committed, so an untracked read during the *first flight* serves the
declared value and cannot throw. But the window ends at the first real answer:
probed against `solid-js@2.0.0-rc.0`, once a re-ask is in flight (an input
change or `refresh`), an untracked read of the very same node throws
`PENDING_ASYNC_UNTRACKED_READ` exactly like an undeclared one. The finding's
message says so explicitly for declared sources.

**Fail-honest downgrade.** When the source has an options argument the
analyzer cannot read (a spread, a variable, a computed object), a
`loadingValue` declaration can be neither proven nor ruled out — the throw is
no longer proven, so the finding is reported as `uncertifiable` instead of a
proven violation. A source with no options argument at all keeps the proven
violation.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

function Profile() {
  const name = user().name; // Throws while user() is pending.
  return <h1>{name}</h1>;
}
```

Examples of **correct** code for this rule:

```tsx
const user = createMemo(() => fetchUser(id()));

function Profile() {
  // Reads inside JSX are tracked: they suspend to the nearest Loading boundary.
  return <h1>{user().name}</h1>;
}
```

## How to fix

Read async values where the graph can wait for them: JSX, a `createMemo`, or an
effect's compute function. The read then suspends to the nearest `<Loading>`
boundary and re-runs when the value settles. A `loadingValue` declaration only
protects the first flight — it does not make untracked reads safe after the
first answer lands.

## Related

- [pending-async-forbidden-scope](pending-async-forbidden-scope.md) — reads in scopes that cannot suspend
- [async-outside-loading-boundary](async-outside-loading-boundary.md) — providing fallback UI
- [strict-read-untracked](strict-read-untracked.md) — the synchronous analogue
