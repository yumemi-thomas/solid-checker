/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { createVibrate(200); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
