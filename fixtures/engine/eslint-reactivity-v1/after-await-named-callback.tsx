import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
async function compute() {
  await Promise.resolve();
  return count();
}

export const value = createMemo(compute);
