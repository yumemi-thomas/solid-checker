import { onSettled } from "solid-js";
import { union } from "@solid-primitives/set";
export default function App() {
  onSettled(() => {
    try { union(new Set([1]), new Set([2])); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
