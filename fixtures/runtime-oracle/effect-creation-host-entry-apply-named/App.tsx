import { createEffect, createMemo, untrack } from "solid-js";

export function ApplyNamed() {
  createEffect(
    () => 0,
    () => {
      const make = () => createMemo(() => 3);
      make();
    },
  );
  return <p>n</p>;
}

