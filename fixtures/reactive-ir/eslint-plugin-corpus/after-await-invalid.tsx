import {
  createMemo,
  createSignal,
  type Accessor,
} from "solid-js";

const [first] = createSignal(0);
const firstAlias = first;
const secondAlias = firstAlias;

export const aliased = createMemo(async () => {
  await Promise.resolve();
  return secondAlias();
});

export const expressionShapes = createMemo(async () => {
  const result = { value: await Promise.resolve(1) };
  return result.value + first();
});

export function install(read: Accessor<number>) {
  return createMemo(async () => {
    await Promise.resolve();
    return read();
  });
}
