// The same for a `flush` imported from a module that does not resolve.
import { createEffect, createMemo, createSignal } from "solid-js";
import { flush } from "./not-installed";

function sync() {
  flush();
}

const [v] = createSignal(1);
createEffect(v, () => {
  createMemo(() => 1);
});
sync();
