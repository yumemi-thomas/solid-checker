import { createSegment } from "@solid-primitives/pagination";
import { createSignal } from "solid-js";
export default function App() {
  const [page] = createSignal(1); const items = createSegment([1, 2, 3, 4], 2, page);
  const current = items().length;
  return <p>{String(current)}</p>;
}
