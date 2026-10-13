/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { mapRange(() => 0, () => 3, () => 1, value => value); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
