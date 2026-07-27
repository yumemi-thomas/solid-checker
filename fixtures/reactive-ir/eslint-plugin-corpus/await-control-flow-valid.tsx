import {
  createEffect,
  createMemo,
  createSignal,
} from "solid-js";

const [count] = createSignal(0);

export const conditionalAwait = createMemo(async () => {
  if (Math.random() > 0.5) {
    await Promise.resolve();
  }
  return count();
});

export const loopAwait = createMemo(async () => {
  while (Math.random() > 0.5) {
    await Promise.resolve();
  }
  return count();
});

export const shortCircuitAwait = createMemo(async () => {
  Math.random() > 0.5 && (await Promise.resolve());
  return count();
});

export const nestedClosure = createMemo(async () => {
  await Promise.resolve();
  const readLater = () => count();
  return readLater;
});

export const nestedReactiveScope = createMemo(async () => {
  await Promise.resolve();
  createEffect(() => count(), value => {
    console.log(value);
  });
  return 1;
});
