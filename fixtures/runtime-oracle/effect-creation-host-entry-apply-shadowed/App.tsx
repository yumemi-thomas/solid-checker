import { createEffect, createMemo, untrack } from "solid-js";
function setTimeout(fn: () => void, _ms?: number): void {
  fn();
}

export function ApplyShadowed() {
  createEffect(
    () => 0,
    () => {
      setTimeout(() => {
        createMemo(() => 4);
      }, 0);
    },
  );
  return <p>s</p>;
}

