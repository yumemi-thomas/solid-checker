import { createSignal, createRoot, onCleanup } from "solid-js";
const [c] = createSignal(1);
function reg() { onCleanup(() => { c(); }); }
export function App() {
  const dispose = createRoot((d) => { reg(); return d; });
  dispose();
  return <p />;
}
