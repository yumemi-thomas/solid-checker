import { createSignal } from "solid-js";
const [count] = createSignal(0);
export async function compute() {
  await Promise.resolve();
  return count();
}
