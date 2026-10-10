import { onSettled } from "solid-js";
import { createReducedMotion } from "@solid-primitives/a11y";
export default function App() {
  onSettled(() => {
    try { createReducedMotion(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
