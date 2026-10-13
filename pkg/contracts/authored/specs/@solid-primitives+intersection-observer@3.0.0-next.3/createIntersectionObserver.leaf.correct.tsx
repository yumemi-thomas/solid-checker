/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  createIntersectionObserver(() => [document.body]);
  onSettled(() => {});
  return <p>ready</p>;
}
