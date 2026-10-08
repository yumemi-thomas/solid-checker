/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { createNotification("hello"); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
