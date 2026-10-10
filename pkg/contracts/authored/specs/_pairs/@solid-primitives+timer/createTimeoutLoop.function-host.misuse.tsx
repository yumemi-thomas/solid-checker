/** @jsxImportSource @solidjs/web */
import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  let first = true;
  createTimeoutLoop(() => {
    if (!first) return;
    first = false;
    try { onCleanup(() => {}); } catch { /* Keep native no-owner diagnostic. */ }
    document.getElementById("done")!.textContent = "done";
  }, () => 30);
  return <p id="done">waiting</p>;
}
