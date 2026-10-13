/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAbortable } from "@solid-primitives/async";
export default function App() {
  createAbortable();
  onSettled(() => {});
  return <p>ready</p>;
}
