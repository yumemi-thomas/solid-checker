import { createSignal, createRoot } from "solid-js";
const [count] = createSignal(0);
createRoot(() => console.log(count()));
export function C() {
  return <main />;
}
