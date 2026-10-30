import { createOptimistic, createSignal } from "solid-js";

// The caller sees only the returned values' types: `Setter<number>` for both,
// whether `createSignal` or `createOptimistic` made them.
export function useCount() {
  return createSignal(0);
}

export function useVals() {
  const [count, setCount] = createSignal(0);
  return { count, setCount };
}

export function useOptimistic() {
  return createOptimistic(0);
}
