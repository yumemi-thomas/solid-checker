import { createSignal } from "solid-js";

export const [count, setCount] = createSignal(0);

export function readGeneric<T>(_value: T): number {
  return count();
}

export function overloaded(_value: string): number;
export function overloaded(_value: number): number;
export function overloaded(_value: string | number): number {
  return count();
}
