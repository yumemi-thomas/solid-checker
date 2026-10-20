import { createSignal, isPending } from "solid-js";
export function App() {
  const [n] = createSignal(1);
  const p = isPending(() => n());
  return <p id="mk7">{String(p)}</p>;
}
