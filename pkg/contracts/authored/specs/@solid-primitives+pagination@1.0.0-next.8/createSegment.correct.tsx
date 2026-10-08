import { createSegment } from "@solid-primitives/pagination";
import { createSignal } from "solid-js";
export default function App() {
  const [page] = createSignal(1); const items = createSegment([1, 2, 3, 4], 2, page);
  return <p>{String(items().length)}</p>;
}
