import { createSignal } from "solid-js";

// The browser build returns a reactive accessor under `view`.
export function Show(props) {
  const [value] = createSignal(props.when);
  return { view: value };
}

// The negative control: identical in both branches, so it stays unconditional.
export function Steady() {
  return { view: 1 };
}
