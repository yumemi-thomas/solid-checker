// A call inside an inline callback of the apply is not "directly in the apply
// callback": the message says so. The effect in `forEach` below gets no
// finding from either rule today (SC4001's owner walk does not climb a
// standard-library callback; the dialect hands effects to SC4001), pinned in
// docs/open-work.md.
import { createEffect, createMemo, createSignal } from "solid-js";

const [v] = createSignal(1);
createEffect(v, () => {
  [1].forEach(() => createMemo(() => 1));
});
createEffect(v, () => {
  [1].forEach(() => createEffect(v, () => {}));
});
