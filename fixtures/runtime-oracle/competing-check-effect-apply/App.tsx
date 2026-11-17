import { createEffect, createSignal } from "solid-js";

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    createEffect(() => 1, () => {});
  });
  return (
    <div>
    </div>
  );
}
