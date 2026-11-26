import { createEffect, createMemo } from "solid-js";

export function ForAwaitOperand() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        for await (const x of [createMemo(() => 5)]) {
          void x;
        }
      })();
    },
  );
  return <p>o</p>;
}
