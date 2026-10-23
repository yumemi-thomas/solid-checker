// The twin of the base fixture with Solid's own `flush`: it drains the queued
// apply under the root (`@solidjs/signals` rc.17 `dist/dev.js:1990-2024`), so
// the memo in it may be owned and SC4005 is uncertifiable (ADR 0362).
import { createEffect, createMemo, createRoot, createSignal, flush } from "solid-js";
import { useHead } from "@solidjs/fake-tools";

createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  useHead(() => 1);
  flush();
});
