/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { createSpring(0); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
