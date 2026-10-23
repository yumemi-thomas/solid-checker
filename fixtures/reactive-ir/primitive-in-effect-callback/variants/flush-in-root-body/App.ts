// A `flush()` written in a `createRoot` body runs on the root's stack, with
// the root as the ambient owner: `runEffect` raises the effect marker but never
// resets the context (`@solidjs/signals` rc.17 `dist/dev.js:1990-2024`), and
// neither `flush` nor `GlobalQueue.flush` touches it (`dist/dev-shared.js:2129`,
// `:2646`). The queued apply therefore runs under the root, the memo in it is
// created owned and nothing throws, so SC4005's "no owner" claim is unprovable
// here: an obligation, not a violation. SC4001 states the same premise, so the
// `onCleanup` in the apply is an obligation too, where it was a violation
// without the `flush()` (the root's region does not answer for it).
import { createEffect, createMemo, createRoot, createSignal, flush, onCleanup } from "solid-js";

createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
    onCleanup(() => {});
  });
  flush();
});
