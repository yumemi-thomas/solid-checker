// SC7005 http-response-after-flush: httpStatus/httpHeader gate the write and
// its retraction on `!response.committed` (@solidjs/web@2.0.0-rc.0
// dist/server.js — code-read), and the head commits at the shell flush. A
// call by content below a Loading boundary is a committed no-op whenever the
// boundary settles post-flush; fallbacks are shell content and stay silent.
import { Loading, createMemo } from "solid-js";
import { httpHeader, httpStatus, renderToStream } from "@solidjs/web";

const product = createMemo(() => fetchProduct());

// Rendered below a Loading boundary (see Page): both calls are post-flush
// no-ops whenever the boundary settles after the shell — two findings.
export function ProductDetails() {
  httpStatus(410);
  httpHeader("cache-control", "no-store");
  return <div>{product().name}</div>;
}

// Shell content: rendered outside every Loading boundary — silent.
export function ShellStatus() {
  httpStatus(404);
  httpHeader("cache-control", "no-store");
  return <div>not found</div>;
}

// Fallback position: the fallback flushes with the shell, so its write
// applies (and retracts pre-flush if the boundary settles early) — silent.
export function FallbackNote() {
  httpHeader("x-fallback", "1");
  return <div>loading…</div>;
}

// An event handler below the boundary runs client-side, where both exports
// are no-ops for a different reason — silent.
export function LateButton() {
  return <button onClick={() => httpHeader("x-late", "1")}>save</button>;
}

export function Page() {
  return (
    <main>
      <ShellStatus />
      <Loading fallback={<FallbackNote />}>
        <ProductDetails />
        <LateButton />
        {/* Lexically inside the boundary's children: the same drop — finding. */}
        <div>{httpHeader("x-region", product().name)}</div>
      </Loading>
    </main>
  );
}

export function serve() {
  return renderToStream(() => <Page />);
}

declare function fetchProduct(): { available: boolean; name: string };
