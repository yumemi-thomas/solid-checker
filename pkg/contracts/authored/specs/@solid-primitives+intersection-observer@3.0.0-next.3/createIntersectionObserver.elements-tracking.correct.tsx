/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [source] = createSignal(1);
  createIntersectionObserver(() => { source(); return [document.body]; });
  return <p>ready</p>;
}
