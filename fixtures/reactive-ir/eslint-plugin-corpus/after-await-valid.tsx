import {
  createEffect,
  createMemo,
  createSignal,
  untrack,
} from "solid-js";

const [count, setCount] = createSignal(0);

export const before = createMemo(async () => {
  const current = count();
  await Promise.resolve();
  return current;
});

export const awaitedExpression = createMemo(async () => {
  return await Promise.resolve(count());
});

export const explicitlyUntracked = createMemo(async () => {
  await Promise.resolve();
  return untrack(() => count());
});

export const nestedScope = createMemo(async () => {
  await Promise.resolve();
  createEffect(() => count(), value => console.log(value));
  return 1;
});

export async function standalone() {
  await Promise.resolve();
  return count();
}
