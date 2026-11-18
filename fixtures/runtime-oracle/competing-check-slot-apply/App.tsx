import { createEffect, createRenderEffect, createSignal } from "solid-js";

const h1 = { go: () => {
  createRenderEffect(() => 1, () => {});
} };

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    h1.go();
  });
  return (
    <div>
    </div>
  );
}
