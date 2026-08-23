import { createSignal } from "solid-js";

// Both branches prove a reactive return; they disagree about which property
// carries it.
export function Show(props) {
  const [value] = createSignal(props.when);
  return { view: value };
}

export function Steady() {
  return { view: 1 };
}
