// A call spelled `flush` whose callee has no declaration (`any`) may be
// Solid's `flush`; it sits in a function, so the program may flush from an
// owned scope. Counterevidence may be spelled: SC4005 is an obligation, and so
// is SC4001 for the `onCleanup` in the apply.
import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";

declare const flush: any;

function sync() {
  flush();
}

const [v] = createSignal(1);
createEffect(v, () => {
  createMemo(() => 1);
  onCleanup(() => {});
});
sync();
