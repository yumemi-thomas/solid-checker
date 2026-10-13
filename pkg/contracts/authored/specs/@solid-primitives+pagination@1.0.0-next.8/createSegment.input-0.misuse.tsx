import { createSignal } from "solid-js";
import { createSegment } from "@solid-primitives/pagination";
export default function App() {
  const [source] = createSignal(1); const [sink, setSink] = createSignal(0);
  const input = () => { const value = source(); try { setSink(1); } catch { /* Expected owned-scope diagnostic. */ } return [value, 2, 3]; };
  const result = createSegment(input, 2, () => 1);
  return <><p>{result().length}</p><p>{sink()}</p></>;
}
