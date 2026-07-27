import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
async function compute() {
  await Promise.resolve();
  return count();
}
const alias = compute;
const secondAlias = alias;

export const value = createMemo(secondAlias);
