/** @jsxImportSource @solidjs/web */
import { onCleanup } from "solid-js";
import { createMutationObserver } from "@solid-primitives/mutation-observer";
export default function App() {
  createMutationObserver(() => {
    try { onCleanup(() => {}); } catch { /* Retain leaf diagnostic. */ }
    return document.body;
  }, { childList: true }, () => {});
  return <p>ready</p>;
}
