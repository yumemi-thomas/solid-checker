/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { keyArray } from "@solid-primitives/keyed";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { keyArray(() => [1, 2], item => item, value => value()); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
