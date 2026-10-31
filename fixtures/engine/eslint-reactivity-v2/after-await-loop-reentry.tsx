import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
// ADR 0516: a read before the loop's own await runs after it on a later
// iteration: uncertifiable.
export const reentered = createMemo(async () => {
  for (let i = 0; i < 2; i += 1) {
    count();
    await Promise.resolve();
  }
  return 1;
});
// A read before the loop runs before any await: silent.
export const before = createMemo(async () => {
  count();
  for (let i = 0; i < 2; i += 1) {
    await Promise.resolve();
  }
  return 1;
});
