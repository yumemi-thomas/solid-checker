/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
export default function App() {
  onSettled(() => {
    try { createIntervalCounter(60_000); } catch { /* Retain the leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
