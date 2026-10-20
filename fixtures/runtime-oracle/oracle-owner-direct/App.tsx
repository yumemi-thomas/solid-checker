import { createSignal, createEffect, onCleanup } from "solid-js";

export function Direct() {
  const [count] = createSignal(0);
  return (
    <button
      onClick={() => {
        createEffect(() => count(), () => undefined);
        onCleanup(() => undefined);
      }}
    >
      a
    </button>
  );
}
