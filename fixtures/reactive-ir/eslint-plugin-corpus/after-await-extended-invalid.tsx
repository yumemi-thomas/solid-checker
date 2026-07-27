import {
  createMemo,
  createOptimistic,
  createRenderEffect,
  createSignal,
} from "solid-js";

const [count] = createSignal(0);
const [optimistic] = createOptimistic(0);

export const renderEffect = createRenderEffect(async () => {
  await Promise.resolve();
  return count();
}, value => console.log(value));

export const optimisticRead = createMemo(async () => {
  await Promise.resolve();
  return optimistic();
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
