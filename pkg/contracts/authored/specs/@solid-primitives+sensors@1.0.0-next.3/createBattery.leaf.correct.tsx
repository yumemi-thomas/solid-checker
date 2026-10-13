/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createBattery } from "@solid-primitives/sensors";
export default function App() {
  createBattery();
  onSettled(() => {});
  return <p>ready</p>;
}
