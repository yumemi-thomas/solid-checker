import { createSignal } from "solid-js";

export const [count, setCount] = createSignal(0);

export function readA(depth: number): number {
  return depth > 0 ? readB(depth - 1) : 0;
}

export function readB(depth: number): number {
  return depth > 0 ? readA(depth - 1) : count();
}
