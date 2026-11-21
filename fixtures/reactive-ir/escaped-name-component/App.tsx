import { createSignal } from "solid-js";

export function \u0042odyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <div>{s()}</div>;
}
