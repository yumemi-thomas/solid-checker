// Crash candidate: two functions each named at the other's sole createRoot position.
import { createRoot, createSignal } from "solid-js";
export default function App() {
  const [s] = createSignal(0);
  function a() { createRoot(b); }
  function b() { createRoot(a); s(); }
  return <span id="w">{s()}</span>;
}
