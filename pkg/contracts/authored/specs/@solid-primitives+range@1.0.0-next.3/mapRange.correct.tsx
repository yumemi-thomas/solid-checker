/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { mapRange(() => 0, () => 3, () => 1, value => value); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
