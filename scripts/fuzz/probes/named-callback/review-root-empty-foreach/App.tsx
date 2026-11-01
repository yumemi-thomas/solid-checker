// FP candidate: inherits_callback_role climbs a library callback that runs zero times.
import { createRoot, createSignal } from "solid-js";
export default function App() {
  const [s] = createSignal(0);
  const none: number[] = [];
  const init = () => { none.forEach(() => { s(); }); };
  createRoot(init);
  return <span id="w">{s()}</span>;
}
