import { createMemo, createRenderEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

export const renderEffect = createRenderEffect(async () => {
  await Promise.resolve();
  return count();
});

export const multipleReads = createMemo(async () => {
  await Promise.resolve();
  return count() + count();
});

export const castRead = createMemo(async () => {
  await Promise.resolve();
  return count() as number;
});

export const sequenceRead = createMemo(async () => (
  await Promise.resolve(),
  count()
));
