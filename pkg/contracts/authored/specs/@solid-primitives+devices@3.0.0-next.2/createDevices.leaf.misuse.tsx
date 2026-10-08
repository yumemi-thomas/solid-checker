/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createDevices } from "@solid-primitives/devices";
export default function App() {
  onSettled(() => { try { createDevices(); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
