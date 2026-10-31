// Crash candidate: a function named at its own sole createRoot position (dead code, tsc-valid).
import { createRoot, createSignal } from "solid-js";
export default function App() {
  const [s] = createSignal(0);
  function a() { createRoot(a); s(); }
  return <span id="w">{s()}</span>;
}
