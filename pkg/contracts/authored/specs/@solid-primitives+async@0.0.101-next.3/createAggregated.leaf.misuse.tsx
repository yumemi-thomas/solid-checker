/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  onSettled(() => { try { createAggregated(() => 1); } catch { /* Keep the emitted dev diagnostic. */ } });
  return <p>ready</p>;
}
