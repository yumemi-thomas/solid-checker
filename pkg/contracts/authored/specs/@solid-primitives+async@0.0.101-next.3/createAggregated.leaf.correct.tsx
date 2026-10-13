/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  createAggregated(() => 1);
  onSettled(() => {});
  return <p>ready</p>;
}
