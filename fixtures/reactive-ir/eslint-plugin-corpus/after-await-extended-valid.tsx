import {
  createMemo,
  createSignal,
  untrack,
} from "solid-js";

const [count] = createSignal(0);

export const awaitedRead = createMemo(async () => await Promise.resolve(count()));

export const nested = createMemo(async () => {
  await Promise.resolve();
  const later = () => count();
  return later;
});

export const optedOut = createMemo(async () => {
  await Promise.resolve();
  return untrack(() => count());
});
