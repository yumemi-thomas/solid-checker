import { createSignal } from "solid-js";
import { useRoot } from "./lib";
export function C() {
  const [count] = createSignal(0);
  useRoot(() => console.log(count()));
  return <main />;
}
