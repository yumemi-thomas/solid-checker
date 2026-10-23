// `@solidjs/fake-tools`' `useAction(action)` has the shape of
// `@solidjs/router`'s (`dist/data/action.js:232-236`): `action` is its own
// parameter, not Solid's `action`, so `action.apply(...)` in the package
// builds no action wrapper and the ADR 0362 veto has nothing to count.
import { createEffect, createMemo, createRoot, createSignal } from "solid-js";
import { useAction } from "@solidjs/fake-tools";

// SC4005 violation: nothing drains the apply under the root.
createRoot(() => {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  const increment = useAction((n: number) => n + 1);
  increment(1);
});
