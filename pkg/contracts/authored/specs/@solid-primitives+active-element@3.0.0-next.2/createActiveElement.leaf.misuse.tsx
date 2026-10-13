import { onSettled } from "solid-js";
import { createActiveElement } from "@solid-primitives/active-element";
export default function App() {
  onSettled(() => {
    try { createActiveElement(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
