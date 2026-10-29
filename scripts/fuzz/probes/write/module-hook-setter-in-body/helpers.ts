import { createSignal } from "solid-js";

export function useVals() {
  const [w1, setW1] = createSignal(0);
  return { w1, setW1 };
}
