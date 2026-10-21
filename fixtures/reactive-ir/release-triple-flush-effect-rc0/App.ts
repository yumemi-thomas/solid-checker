import { createEffect, createRoot, createSignal, flush } from "solid-js";

createRoot(() => {
  const [value] = createSignal(1);
  createEffect(value, () => {
    flush();
  });
});
