import { createEffect, createMemo, untrack } from "solid-js";

export function ApplyUntrack() {
  createEffect(
    () => 0,
    () => {
      untrack(() => {
        createMemo(() => 2);
      });
    },
  );
  return <p>u</p>;
}

