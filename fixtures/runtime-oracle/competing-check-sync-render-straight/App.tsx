import { createMemo, createRenderEffect, createSignal } from "solid-js";

export function C1() {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    createMemo(async () => 1, { sync: true });
  });
  return (
    <div>
    </div>
  );
}
