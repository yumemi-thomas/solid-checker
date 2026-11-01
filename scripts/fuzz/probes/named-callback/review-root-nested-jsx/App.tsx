// FP candidate: JSX a nested closure returns is tracked by the compiler, but the climb lands on the closure's call.
import { createRoot, createSignal } from "solid-js";
export default function App() {
  const [s] = createSignal(0);
  const init = () => { const view = () => <p>{s()}</p>; return view(); };
  const v = createRoot(init);
  return <span id="w">{v}</span>;
}
