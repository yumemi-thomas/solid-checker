import { createSignal, onCleanup } from "solid-js";
const [c] = createSignal(1);
export function App() {
  onCleanup(() => { c(); });
  return <p />;
}
