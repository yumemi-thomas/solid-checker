import { createSignal } from "solid-js";

export const [count, setCount] = createSignal(0);

export function readCount() {
  return count();
}

export function invoke<T>(callback: () => T): T {
  return callback();
}
