/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  createVisibilityObserver(document.body);
  onSettled(() => {});
  return <p>ready</p>;
}
