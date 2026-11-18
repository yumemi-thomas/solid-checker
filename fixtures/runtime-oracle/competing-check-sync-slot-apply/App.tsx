import { createEffect, createMemo, createSignal } from "solid-js";

const h1 = { go: () => {
  createMemo(async () => 1, { sync: true });
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
