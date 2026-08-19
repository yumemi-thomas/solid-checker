# http-response-after-flush

`SC7005` · **warning** · violation

`httpStatus()` / `httpHeader()` is called by content that renders below a
`<Loading>` boundary, in a project that server-renders.

## What it does

Flags calls to `httpStatus` and `httpHeader` (imported from `@solidjs/web`)
made in a render-time scope that sits below a `<Loading>` boundary — either
lexically inside a `Loading` element's children, or in the body of a
component that is rendered as a `Loading` element's child. A visible server
entry makes this a violation; when rendering mode is not visible in the
analyzed project, it is an **uncertifiable** result.

**Premise: code-read on the pinned runtime, with the RFC as co-author.**
`@solidjs/web@2.0.0-rc.0`'s `dist/server.js` gates both primitives — the
write *and* the cleanup-time retraction — on `!response.committed`
(`httpStatus` at `server.js:2901`, `httpHeader` at `:2935`), and
`createSSRResponse` commits the stub at the shell flush. A post-commit call
short-circuits before it reaches the response, so it never even hits the loud
`headers.set` guard that direct stub mutation trips — it is a **silent**
no-op. RFC 12 states the same as contract: "Headers declared by streamed
route content — anything below a `<Loading>` boundary that resolves after the
shell went out — run post-flush and are committed no-ops by contract. There
is no queue that holds them for a later response; the head is on the wire."

## Why is this bad?

Under streaming SSR the shell — everything outside `<Loading>` boundaries,
plus every boundary's *fallback* — flushes first, and the response head
(status, headers) goes onto the wire with it. Content below a boundary that
settles after that flush executes too late to speak: its `httpStatus(404)` or
`httpHeader("cache-control", …)` is dropped without a trace. The page renders
fine, so the bug ships quietly — a not-found page that answers 200, a
`no-store` header that never leaves.

## Why a warning, not an error

The drop is **conditional**: the runtime still applies the write whenever the
boundary settles *before* the head commits — fast data resolving before the
shell flush, a `deferStream: true` source holding the shell open, or
`renderToString`-style rendering where the head is derived after the tree
settles. A static rule cannot prove which side a given request lands on, so
the finding reports the hazard rather than claiming an unconditional runtime
failure.

## Examples

Examples of **incorrect** code for this rule (in a project that
server-renders):

```tsx
function ProductPage() {
  const product = createMemo(() => fetchProduct(id()));
  return (
    <Loading fallback={<Skeleton />}>
      <Product data={product()} />
    </Loading>
  );
}

function Product(props: { data: ProductData }) {
  if (!props.data.available) {
    httpStatus(410); // below the boundary: post-flush no-op when the fetch is slow
    httpHeader("cache-control", "no-store"); // same drop
  }
  return <article>…</article>;
}
```

Examples of **correct** code for this rule:

```tsx
function NotFound() {
  // Shell content: the head has not committed yet, and the scope-tied
  // declaration retracts if this subtree is disposed pre-flush.
  httpStatus(404);
  httpHeader("cache-control", "no-store");
  return <h1>Not found</h1>;
}
```

```tsx
// The status depends on data — hold the shell for that source instead of
// letting the boundary fall back:
const product = createMemo(() => fetchProduct(id()), { deferStream: true });
```

## How to fix

Decide the response head in shell content: move the `httpStatus`/`httpHeader`
call above every `<Loading>` boundary, or mark the async source the decision
depends on with `deferStream: true` (RFC 05) so the shell flush waits for it.
A boundary *fallback* is also shell content — but a status set there retracts
when the boundary later settles pre-flush, so prefer deciding the head from
settled data.

## When it does not fire

- **Rendering mode.** On the client both exports are unconditional no-ops
  (`dist/web.js` / `dist/dev.js` export empty functions), but absence of a
  server entry from one analyzed project does not prove the application is
  CSR-only. A visible server-rendering import yields the violation; otherwise
  the dominated render-time call is uncertifiable.
- **Fallback position.** A call inside a `Loading` element's `fallback` —
  or in a component rendered in fallback position — is shell-time and
  applies; only the boundary's children are post-flush material.
- **Event handlers and deferred callbacks.** Those run client-side (or after
  the render), where the call is a no-op for a reason this rule does not
  describe; they stay out.
- **Indirect calls.** Only calls lexically in a component's body (or in JSX)
  are attributed to that component; a helper called from below a boundary is
  not traced. Dominance crosses exactly one component boundary: the
  component's own JSX, and the call sites that render it.

## Related

- [ssr-client-source-outside-loading-boundary](ssr-client-source-outside-loading-boundary.md)
  — the same server-render gate, for client-only sources
- [async-outside-loading-boundary](async-outside-loading-boundary.md) —
  where `Loading` boundaries come from
