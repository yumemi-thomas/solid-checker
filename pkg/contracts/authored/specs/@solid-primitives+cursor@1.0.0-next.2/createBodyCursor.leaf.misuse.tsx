import { onSettled } from "solid-js";
import { createBodyCursor } from "@solid-primitives/cursor";
export default function App() {
  onSettled(() => {
    try { createBodyCursor(() => "pointer"); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
