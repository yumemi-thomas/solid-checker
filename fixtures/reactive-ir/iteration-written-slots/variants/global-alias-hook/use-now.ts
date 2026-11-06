import { createSignal } from "solid-js";
export function useNow() {
  const [now, setNow] = createSignal(0);
  return [now, setNow, 0] as const;
}
