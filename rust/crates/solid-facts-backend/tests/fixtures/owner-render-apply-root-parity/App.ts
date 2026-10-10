import { createEffect, createRenderEffect, createRoot, createSignal, onCleanup } from "solid-js";

const [count] = createSignal(0);

createRoot(() => {
  onCleanup(() => {
    // root body
  });
  createRenderEffect(
    () => count(),
    () => {
      onCleanup(() => {
        // render apply
      });
    },
  );
  createEffect(
    () => count(),
    () => {
      onCleanup(() => {
        // effect apply
      });
    },
  );
});
