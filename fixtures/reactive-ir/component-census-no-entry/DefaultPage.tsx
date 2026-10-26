import { createSignal } from "solid-js";

const [, setN] = createSignal(0);

// Violation: the default export of a route module is the host's component.
export default function Route() {
  setN(1);
  return <li />;
}
