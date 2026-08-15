# async-outside-loading-boundary

`SC5003` · **warning** · violation

An async accessor is rendered with no `<Loading>` boundary above it.

## What it does

Flags tracked JSX reads of accessors returned by async computations when no
`<Loading>` boundary dominates the read.

This is the static counterpart of Solid's dev-mode `ASYNC_OUTSIDE_LOADING_BOUNDARY`
warning — and like the runtime warning, it is informational rather than halting.

**Declared first paint exception.** A source that provably declares
`loadingValue` (or `seedLoadingValue: true` on store-family sources) is never
flagged: the node is born committed, so its first flight never suspends
readers and never trips a `Loading` boundary — rendering it bare is the
pattern's entire point, and the runtime warning does not fire for it
(verified against `solid-js@2.0.0-rc.0`). Once the first real answer lands,
re-asks use the normal pending machinery, but by then the reader holds a
committed value and the boundary question is moot for this warning. If the
options argument cannot be read statically (a spread, a variable), the
warning keeps firing — it is informational, so over-reporting is the honest
default there.

## Why is this bad?

It isn't wrong, but it may not be what you intended. Without a boundary, the
runtime handles pending async by deferring the mount: the container stays empty (or
keeps its existing content) until every uncaught async value settles, then attaches
atomically. Users see nothing while data loads — no spinner, no skeleton — and a
slow endpoint reads as a hung app.

## Examples

Code this rule flags:

```tsx
const user = createMemo(() => fetchUser(id()));

// Nothing renders until user() settles.
render(() => <Profile user={user()} />, root);
```

Code with explicit fallback UI:

```tsx
render(
  () => (
    <Loading fallback={<Spinner />}>
      <Profile user={user()} />
    </Loading>
  ),
  root,
);
```

## How to fix

Wrap the reading subtree in `<Loading fallback={...}>` when you want visible
fallback UI or partial progressive mount. Leave it as is when an empty container
during load is intended (for example over a static shell) — the deferred atomic
mount is the permissive default, not an error.

For a "refreshing…" indicator during revalidation, `<Loading>` is the wrong tool —
once content has rendered, the boundary keeps it visible. Use
`isPending(() => expr)` under the same boundary, or `<Loading on={key}>` to re-show
the fallback on key changes.

## Related

- [ssr-client-source-outside-loading-boundary](ssr-client-source-outside-loading-boundary.md) — the same missing boundary over a bare `ssrSource: "client"` source, which is a hard server error rather than informational (it subsumes this warning on the same read)
- [pending-async-untracked-read](pending-async-untracked-read.md) — untracked pending reads, which do throw
- [no-owner-boundary](no-owner-boundary.md) — boundaries need owners
