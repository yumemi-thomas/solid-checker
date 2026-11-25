import { createEffect, createMemo, untrack } from "solid-js";

export function ApplyTimer() {
  createEffect(
    () => 0,
    () => {
      globalThis.setTimeout(() => {
        document.body.dataset.timer = "ran";
        createMemo(() => 1);
      }, 0);
    },
  );
  return <p>t</p>;
}

