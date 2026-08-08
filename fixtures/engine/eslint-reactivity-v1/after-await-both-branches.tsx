import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
export const value = createMemo(async () => {
  if (Math.random() > 0.5) await Promise.resolve(1);
  else await Promise.resolve(2);
  return count();
});
