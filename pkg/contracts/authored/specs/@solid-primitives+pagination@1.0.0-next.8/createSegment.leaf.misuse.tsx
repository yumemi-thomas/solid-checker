import { onSettled } from "solid-js";
import { createSegment } from "@solid-primitives/pagination";
export default function App() {
  onSettled(() => {
    try { createSegment([1, 2, 3], 2, () => 1); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
