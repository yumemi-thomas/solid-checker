import { createSignal } from "solid-js";

export function makeWarm() {
  const [rows] = createSignal(false);
  function read() { return rows(); }
  return { other: () => false, value: () => read() };
}
