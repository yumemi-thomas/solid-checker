import { onSettled } from "solid-js";
import { uppercase } from "@solid-primitives/signal-builders";
export default function App() {
  onSettled(() => {
    try { uppercase(() => "hello"); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
