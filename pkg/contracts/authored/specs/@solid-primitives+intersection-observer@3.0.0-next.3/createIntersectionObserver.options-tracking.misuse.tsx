/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [source] = createSignal(1);
  const [, setSink] = createSignal(0);
  let calls = 0; createIntersectionObserver(() => [document.body], () => { source(); if (++calls > 1) { try { setSink(1); } catch {} } return {}; });
  return <p>ready</p>;
}
