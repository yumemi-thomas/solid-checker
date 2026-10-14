import { createSignal } from "solid-js";

const [count] = createSignal(0);
export function makeTuple() {
  const peek = () => count();
  return [count, peek] as const;
}
