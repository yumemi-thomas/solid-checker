import { createSignal } from "solid-js";
import { createSegment } from "@solid-primitives/pagination";
export default function App() {
  const [source] = createSignal(1); const [sink, setSink] = createSignal(0);
  const input = () => { const value = source();  return value; };
  const result = createSegment([1, 2, 3], 2, input);
  return <><p>{result().length}</p><p>{sink()}</p></>;
}
