import { onSettled } from "solid-js";
import { intersection } from "@solid-primitives/set";
export default function App() {
  onSettled(() => {
    try { intersection(new Set([1]), new Set([2])); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
