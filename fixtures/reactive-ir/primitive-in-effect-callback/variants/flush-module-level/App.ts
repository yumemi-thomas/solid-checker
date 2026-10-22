// A `flush()` statement in the module body runs with no owner (nothing is
// current while a module evaluates), so it drains the apply with no owner
// either: the exemption keeps the claim. Positive: the memo and the cleanup in
// the apply are SC4005 and SC4001 violations.
import { createEffect, createMemo, createSignal, flush, onCleanup } from "solid-js";

const [v] = createSignal(1);
createEffect(v, () => {
  createMemo(() => 1);
  onCleanup(() => {});
});
flush();
