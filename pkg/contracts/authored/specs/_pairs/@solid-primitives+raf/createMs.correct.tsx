/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { createMs(60); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
