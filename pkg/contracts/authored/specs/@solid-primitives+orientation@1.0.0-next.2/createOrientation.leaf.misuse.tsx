/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  onSettled(() => { try { createOrientation(); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
