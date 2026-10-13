/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { repeat } from "@solid-primitives/range";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { repeat(() => 3, index => index); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
