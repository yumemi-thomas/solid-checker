/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  const [source] = createSignal(1);
  const all = createAggregated(source);
  const current = all();
  return <p>{String(current)}</p>;
}
