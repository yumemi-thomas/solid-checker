/** @jsxImportSource @solidjs/web */
import { onCleanup, onSettled } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  createIntersectionObserver(() => [], () => { onCleanup(() => {}); return {}; });
  return <p>ready</p>;
}
