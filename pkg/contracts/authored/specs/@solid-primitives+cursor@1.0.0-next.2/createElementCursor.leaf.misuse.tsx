import { onSettled } from "solid-js";
import { createElementCursor } from "@solid-primitives/cursor";
export default function App() {
  onSettled(() => {
    try { createElementCursor(document.body, "pointer"); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
