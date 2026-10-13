/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  onSettled(() => { try { createIntersectionObserver(() => [document.body]); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
