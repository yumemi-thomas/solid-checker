import { createSignal } from "solid-js";

// Reached through `.then`: this file's mounts are open.
export function Named() {
  const [a] = createSignal(0);
  const v = a();
  return <i>{v}</i>;
}
