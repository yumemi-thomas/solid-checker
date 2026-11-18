import { createEffect, createRenderEffect } from "solid-js";

const h1 = { go: () => {
  createRenderEffect(() => 1, () => {});
} };

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    h1.go();
  });
  setTimeout(() => {
    h1.go();
  }, 0);
  return (
    <div>
    </div>
  );
}
