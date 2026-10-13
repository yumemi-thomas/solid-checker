/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { until } from "@solid-primitives/promise";
export default function App() {
  onSettled(() => {
    try { until(() => true); } catch { /* Retain forbidden child diagnostic. */ }
  });
  return <p>ready</p>;
}
