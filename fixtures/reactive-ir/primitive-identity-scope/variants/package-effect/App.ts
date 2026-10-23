// `@solidjs/fake-tools`' `createEffect(source, apply)` runs `apply` once,
// synchronously, on its caller's stack. It is not Solid's `createEffect`
// (ADR 0365), so its `apply` is no effect apply: the memo and the cleanup in
// it run in the root body, under the root.
import { createMemo, createRoot, createSignal, onCleanup } from "solid-js";
import { createEffect } from "@solidjs/fake-tools";

// Silent: neither SC4005 (memo) nor SC4001 (cleanup).
createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 2);
    onCleanup(() => {});
  });
});
