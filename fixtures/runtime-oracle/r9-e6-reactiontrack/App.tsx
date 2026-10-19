import { createSignal, createRenderEffect, createReaction } from "solid-js";
function slotCompute(cb: () => number) {
  createRenderEffect(cb, () => undefined);
}
function slotApply(cb: () => void) {
  createRenderEffect(() => 1, cb);
}
export function C() {
  const [count] = createSignal(0);
  const track = createReaction(() => undefined);
  track(() => count());
  return <main />;
}
