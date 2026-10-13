import { onSettled } from "solid-js";
import { createClipboard } from "@solid-primitives/clipboard";
export default function App() {
  onSettled(() => {
    try { createClipboard(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
