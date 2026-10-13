/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { until } from "@solid-primitives/promise";
export default function App() {
  until(() => true);
  onSettled(() => {});
  return <p>ready</p>;
}
