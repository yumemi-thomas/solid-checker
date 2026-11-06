import { createSignal } from "solid-js";
export function useX() {
  const [s] = createSignal(1);
  return [s] as const;
}
