/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  createOrientation();
  onSettled(() => {});
  return <p>ready</p>;
}
