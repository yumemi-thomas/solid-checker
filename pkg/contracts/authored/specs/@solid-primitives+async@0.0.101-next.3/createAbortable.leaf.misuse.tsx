/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAbortable } from "@solid-primitives/async";
export default function App() {
  onSettled(() => { try { createAbortable(); } catch { /* Keep the emitted dev diagnostic. */ } });
  return <p>ready</p>;
}
