import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  [1, 2].slice(0, 1);
  return <p>{w()}</p>;
}
