// `@solidjs/fake-tools`' `preloadWhenIdle` calls a local
// `const flush = whenIdle(...)`, the shape of `@solidjs/router`'s
// `dist/preload.js:139-147`. That local is the package's own binding, not
// Solid's `flush` (ADR 0365), so it drains nothing and vetoes nothing.
import { createEffect, createMemo, createRoot, createSignal } from "solid-js";
import { preloadWhenIdle } from "@solidjs/fake-tools";

// SC4005 violation: nothing drains the apply under the root.
createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  preloadWhenIdle(() => {});
});
