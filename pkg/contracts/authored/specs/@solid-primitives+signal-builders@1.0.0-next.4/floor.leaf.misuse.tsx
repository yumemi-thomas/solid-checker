import { onSettled } from "solid-js";
import { floor } from "@solid-primitives/signal-builders";
export default function App() {
  onSettled(() => {
    try { floor(() => 1.2); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
