/** @jsxImportSource @solidjs/web */
import { onCleanup, onSettled } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  onSettled(() => { try { createIntersectionObserver(() => [], () => { onCleanup(() => {}); return {}; }); } catch {} });
  return <p>ready</p>;
}
