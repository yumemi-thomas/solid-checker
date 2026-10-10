/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  onSettled(() => {
    try { createTimeoutLoop(() => {}, 1000); } catch { /* Keep leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
