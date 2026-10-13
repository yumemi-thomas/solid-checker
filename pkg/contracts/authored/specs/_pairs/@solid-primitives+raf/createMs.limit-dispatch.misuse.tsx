/** @jsxImportSource @solidjs/web */
import { onCleanup } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  createMs(60, () => { onCleanup(() => {}); document.getElementById("done")!.textContent = "done"; return Infinity; });
  return <p id="done">waiting</p>;
}
