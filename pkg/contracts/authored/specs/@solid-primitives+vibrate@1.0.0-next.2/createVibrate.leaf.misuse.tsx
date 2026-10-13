/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  onSettled(() => { try { createVibrate(200); } catch { /* Preserve the structured leaf diagnostic. */ } });
  return <p>ready</p>;
}
