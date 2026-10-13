/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createBattery } from "@solid-primitives/sensors";
export default function App() {
  onSettled(() => { try { createBattery(); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
