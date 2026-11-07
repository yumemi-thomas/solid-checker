import { createSignal } from "solid-js";
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
