// `@solidjs/fake-tools` is in the `@solidjs` scope and defines none of Solid's
// primitives (ADR 0365): its `flush`, `createEffect` and `useHead` are its own
// functions, analysed from its source as any dependency's are.
import { createEffect, createMemo, createRoot, createSignal } from "solid-js";
import { flush, useHead } from "@solidjs/fake-tools";

// SC4005 violation: the package's `flush` is not Solid's, so nothing drains
// the queued apply under the root and the memo in it belongs to nobody. Had
// the package's `flush` been taken for Solid's, the ADR 0362 veto would make
// this uncertifiable (the `solid-flush` variant). It is called with an
// argument, so its callee is resolved; an argumentless call of another
// package's `flush` is never resolved and still withholds (ADR 0362's
// spelling rule; docs/open-work.md).
createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  flush(() => {});
});

// Silent: the package's `useHead` runs the tag on the root body's stack, and
// is not `@solidjs/web`'s (whose tag function is tracked).
createRoot(() => {
  useHead(() => createMemo(() => 3));
});
