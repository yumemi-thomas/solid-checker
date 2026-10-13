/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  onSettled(() => { try { createVisibilityObserver(document.body); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
