// The CSR twin of http-response-flush: no server rendering entry point is
// imported, so httpStatus/httpHeader are unconditional client no-ops and
// SC7005 must stay silent everywhere — including below the boundary.
import { Loading, createMemo } from "solid-js";
import { httpHeader, httpStatus } from "@solidjs/web";

const product = createMemo(() => fetchProduct());

export function ProductDetails() {
  httpStatus(410);
  httpHeader("cache-control", "no-store");
  return <div>{product().name}</div>;
}

export function Page() {
  return (
    <Loading fallback={<div>loading…</div>}>
      <ProductDetails />
    </Loading>
  );
}

declare function fetchProduct(): { available: boolean; name: string };
