// ADR 0703: SC7005 places an httpStatus/httpHeader call only where a proof
// does. Each positive below runs below the Loading boundary on the SSR host,
// after the shell flush, and the head drops it (templates m-helper, m-method,
// m-memo, m-wrapper in scripts/fuzz/ssr.mjs).
import { Loading, createMemo } from "solid-js";
import { httpHeader, httpStatus, renderToStream } from "@solidjs/web";

// Positive: a helper the boundary's content calls (an unplaced role before).
function markGone() {
  httpStatus(410);
}
// Positive: an object method, called the same way.
const head = {
  noStore() {
    httpHeader("cache-control", "no-store");
  },
};

export function Inner() {
  markGone();
  head.noStore();
  // Positive: a compute in boundary content (a tracked role before).
  const m = createMemo(() => {
    httpHeader("x-memo", "1");
    return 1;
  });
  return <div>{m()}</div>;
}

// Positive: content a wrapper places below its own boundary.
export function Wrapped() {
  httpHeader("x-wrapped", "1");
  return <div>wrapped</div>;
}
function Boundary(props: { children?: any }) {
  return <Loading fallback={<div>loading…</div>}>{props.children}</Loading>;
}

// Negative: an event handler runs on the client, a no-op for another reason.
export function Late() {
  return <button onClick={() => httpHeader("x-late", "1")}>save</button>;
}

// Negative, shadowed: a project function spelled httpStatus is not Solid's.
export function Shadowed() {
  const httpStatus = (_code: number) => {};
  httpStatus(500);
  return <div>shadowed</div>;
}

export function Page() {
  return (
    <main>
      <Loading fallback={<div>loading…</div>}>
        <Inner />
        <Late />
        <Shadowed />
      </Loading>
      <Boundary>
        <Wrapped />
      </Boundary>
    </main>
  );
}

export function serve() {
  return renderToStream(() => <Page />);
}
