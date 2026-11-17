import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    createMemo(async () => 1, { sync: true });
  });
  return (
    <div>
    </div>
  );
}
