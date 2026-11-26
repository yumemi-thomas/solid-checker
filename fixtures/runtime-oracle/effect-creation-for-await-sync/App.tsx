import { createEffect, createMemo } from "solid-js";

export function ForOfSync() {
  createEffect(
    () => 0,
    () => {
      void (async () => {
        for (const x of [1]) {
          createMemo(() => x);
        }
      })();
    },
  );
  return <p>s</p>;
}
