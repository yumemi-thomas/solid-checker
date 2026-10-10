/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { keyArray } from "@solid-primitives/keyed";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { keyArray(() => [1, 2], item => item, value => value()); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
