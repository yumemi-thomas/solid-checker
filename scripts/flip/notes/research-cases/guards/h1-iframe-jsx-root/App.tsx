import { createSignal } from "solid-js";
const [c] = createSignal(1);
export function App() {
  return <iframe onLoad={() => { c(); }} />;
}
