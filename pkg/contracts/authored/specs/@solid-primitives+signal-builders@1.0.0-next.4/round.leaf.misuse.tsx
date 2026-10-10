import { onSettled } from "solid-js";
import { round } from "@solid-primitives/signal-builders";
export default function App() {
  onSettled(() => {
    try { round(() => 1.2); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
