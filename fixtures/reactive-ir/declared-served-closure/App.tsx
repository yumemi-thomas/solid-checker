import { createMemo } from "solid-js";

// ADR 0890: an IIFE in the body that creates a declared memo runs on the
// creating pass's stack, so it reads the served value, as a direct read does:
// the strict read stands. Nothing else here is open.
export function ServedClosure() {
  const memo = createMemo(async () => 1, { loadingValue: 5 });
  (() => {
    memo();
  })();
  return <p>{1}</p>;
}
