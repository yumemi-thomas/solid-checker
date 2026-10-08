/** @jsxImportSource @solidjs/web */
import { onCleanup, createRoot } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  createMs(60, () => { createRoot(dispose => { onCleanup(() => {}); queueMicrotask(dispose); }); document.getElementById("done")!.textContent = "done"; return Infinity; });
  return <p id="done">waiting</p>;
}
