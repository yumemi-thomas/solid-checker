/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [source] = createSignal(1);
  createIntersectionObserver(() => [document.body], () => { source(); return {}; });
  return <p>ready</p>;
}
