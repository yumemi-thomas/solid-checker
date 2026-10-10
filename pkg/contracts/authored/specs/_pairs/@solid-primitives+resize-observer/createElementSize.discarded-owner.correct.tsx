/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createElementSize } from "@solid-primitives/resize-observer";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { createElementSize(document.body); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
