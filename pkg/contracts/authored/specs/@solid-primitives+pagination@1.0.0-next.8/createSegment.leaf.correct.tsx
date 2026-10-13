import { onSettled } from "solid-js";
import { createSegment } from "@solid-primitives/pagination";
export default function App() {
  createSegment([1, 2, 3], 2, () => 1);
  onSettled(() => {});
  return <p>ready</p>;
}
