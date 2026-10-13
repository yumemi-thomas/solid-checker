import { onSettled } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  onSettled(() => {
    try { createFullscreen(document.body); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
