import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
export const value = createMemo(async () => {
  try { await Promise.resolve(); } finally { console.log("settled"); }
  return count();
});
