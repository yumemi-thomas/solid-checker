// `flush()` only inside the apply itself (SC2008's case) runs in the effect
// phase, where no owner is current: the apply exemption keeps SC4005's claim.
// Positive: the memo in the same apply (and SC2008 for the flush).
import { createEffect, createMemo, createSignal, flush } from "solid-js";

const [v] = createSignal(1);
createEffect(v, () => {
  createMemo(() => 1);
  flush();
});
