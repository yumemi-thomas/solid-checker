import { createSignal } from "solid-js";

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  window.addEventListener("c\u006cick", () => set(3));
  return <div>{s()}</div>;
}
