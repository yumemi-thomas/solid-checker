import { createSignal } from "solid-js";

export function Show(props) {
  const [other] = createSignal(props.when);
  return { other };
}

export function Steady() {
  return { view: 1 };
}
