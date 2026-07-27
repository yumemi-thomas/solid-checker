import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
export const doubled = createMemo(async () => {
  await new Promise<void>(resolve => setTimeout(resolve, 1));
  return count() * 2;
});
