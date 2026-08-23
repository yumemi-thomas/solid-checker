import { createSignal } from "solid-js";

export function channelFor() {
  const [read] = createSignal(1);
  return read;
}
