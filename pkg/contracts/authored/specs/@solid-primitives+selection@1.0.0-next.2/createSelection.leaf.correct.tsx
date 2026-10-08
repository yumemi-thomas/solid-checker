/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSelection } from "@solid-primitives/selection";
export default function App() {
  createSelection();
  onSettled(() => {});
  return <p>ready</p>;
}
