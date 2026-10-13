/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const [pattern, setPattern] = createSignal(200);
  createVibrate(() => {
    const current = pattern();
    try { setPattern(current); } catch { /* Preserve the tracked-write diagnostic. */ }
    return current;
  });
  return <p>ready</p>;
}
