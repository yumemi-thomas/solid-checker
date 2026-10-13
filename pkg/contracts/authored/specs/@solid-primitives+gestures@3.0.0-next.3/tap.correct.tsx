/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { tap } from "@solid-primitives/gestures";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { tap({ callback: position => { console.log(position.x, position.y); } }); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
