import { createSignal } from "solid-js";

export default function useVals() {
  const [r1, setR1] = createSignal(3);
  return { r1, setR1 };
}
