/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createPureReaction } from "@solid-primitives/memo";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createRoot(dispose => { createPureReaction(() => {}); queueMicrotask(dispose); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
