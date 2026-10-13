/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [source] = createSignal(1);
  const [, setSink] = createSignal(0);
  createIntersectionObserver(() => { source(); try { setSink(1); } catch {} return [document.body]; });
  return <p>ready</p>;
}
