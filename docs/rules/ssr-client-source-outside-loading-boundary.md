# ssr-client-source-outside-loading-boundary

`SC5005` · **error** · violation

A source that declares `ssrSource: "client"` — with no `loadingValue` /
`seedLoadingValue` declaration — is rendered with no `<Loading>` boundary above
it, in a project that server-renders.

## What it does

Flags tracked JSX reads of sources created with `ssrSource: "client"` and no
declared first paint when no `<Loading>` boundary dominates the read. A visible
server-rendering entry proves a violation; without one, the same shape is
reported as **uncertifiable**, because the entry may live outside the analyzed
tsconfig or package.

This is the static counterpart of the unconditional server-runtime error
`ssrSource: "client" read during SSR outside a <Loading> boundary` in
`solid-js@2.0.0-rc.0`'s server build.

Detection runs on the option itself, not on async provenance: a bare client
source can be a fully **synchronous** compute (`createMemo(() =>
measureBrowserThing(), { ssrSource: "client" })`), which no pending-async rule
would ever see — yet its server-side read throws all the same, because the
server never runs the compute at all.

## Why is this bad?

With `ssrSource: "client"` the compute never runs on the server (an owner is
still created so hydration ids stay aligned). Without a declared
`loadingValue`/`seedLoadingValue`, the source is a hole the server can never
fill:

- **Under a `<Loading>` boundary**, reads suspend *finally*: the boundary
  flushes its fallback into the HTML and hands the position to the client,
  which renders the branch fresh after hydration. That is the supported shape.
- **Outside any boundary**, the stream would hang forever, so the server
  throws instead — a guaranteed render error on every SSR request that reaches
  this read.

## Examples

Examples of **incorrect** code for this rule (in a project that
server-renders):

```tsx
// The compute is synchronous; the defect is invisible to async analysis.
const widget = createMemo(() => measureBrowserThing(), { ssrSource: "client" });

function Dashboard() {
  // Throws during SSR: no boundary owns this position's fallback.
  return <div>{widget().label}</div>;
}
```

Examples of **correct** code for this rule:

```tsx
const widget = createMemo(() => measureBrowserThing(), { ssrSource: "client" });

function Dashboard() {
  // The server flushes the fallback; the client renders the branch after hydration.
  return (
    <Loading fallback={<WidgetSkeleton />}>
      <div>{widget().label}</div>
    </Loading>
  );
}
```

Declaring a first paint also removes the hole — the server renders the
declared value instead of suspending:

```tsx
const draft = createMemo(() => readDraftFromStorage(key()) ?? null, {
  ssrSource: "client",
  loadingValue: null, // `loadingValue: undefined` is also a valid declaration
});

function Editor() {
  return <div>{draft() ?? "No draft yet"}</div>; // fine anywhere
}
```

Store-family sources (`createStore(fn)`, `createProjection`,
`createOptimisticStore`) declare it as `seedLoadingValue: true`, which promotes
their existing seed to the same role.

## How to fix

Either wrap the reading subtree in `<Loading fallback={...}>` so a boundary
owns the position's fallback during SSR, or declare a provisional first paint
with `loadingValue` (`seedLoadingValue: true` on store-family sources) so the
server has something to render.

## When it does not fire

- **Rendering mode.** A named import of a `@solidjs/web` server-rendering
  entry point (`renderToStream`, `renderToString`, `renderToFrameStream`,
  `renderServerComponent`, `handleServerFunctionRequest`) or `hydrate`
  proves the server path and yields a violation. Without that evidence the
  rule emits an uncertifiable result, not silence: the server entry may live
  in a separate tsconfig/package. A genuinely CSR-only application is runtime
  safe, but the current fact surface has no project-level CSR certificate.
- **Unprovable options.** The `ssrSource: "client"` + no-declaration
  combination must be proven from an exact object-literal options argument
  (static keys, no spreads). A spread or a computed options object could
  declare a `loadingValue`, so no error is claimed.
- **Value-form sources.** `ssrSource` is inert on a value-form
  `createSignal(value, …)` — there is no compute for the server to skip — so
  only function-form sources are considered.

## Related

- [async-outside-loading-boundary](async-outside-loading-boundary.md) — the
  informational client-side counterpart for async sources; this rule subsumes
  it on the same read when both would apply
- [pending-async-untracked-read](pending-async-untracked-read.md) — untracked
  pending reads, which throw on the client
- [missing-owner](missing-owner.md) — boundaries need owners
