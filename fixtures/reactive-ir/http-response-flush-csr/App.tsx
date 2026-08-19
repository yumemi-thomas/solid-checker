// No server rendering entry point is visible in this analyzed project. That
// is not proof of CSR because the server entry may live in another tsconfig
// or package, so the two dominated calls are SC7005 uncertifiable results.
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
