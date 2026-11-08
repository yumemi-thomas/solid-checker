import { createSignal, onCleanup } from "solid-js";
const [c] = createSignal(1);
export function App() {
  const h = onCleanup(() => { c(); });
  h();
  return <p />;
}
