import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
export const value = createMemo(async () => {
  while (Math.random() > 0.5) await Promise.resolve();
  return count();
});
