import { createEffect, createRoot, createSignal } from "solid-js";
import { flush } from "@solidjs/signals";

createRoot(() => {
  const [value] = createSignal(1);
  // `solid-js` re-exports its own nested `@solidjs/signals`; this `flush` is the
  // project's top-level copy, with a queue of its own that this effect never
  // runs on.
  createEffect(value, () => {
    flush();
  });
});
