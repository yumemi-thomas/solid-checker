/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { createUserTheme } from "@solid-primitives/cookies";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { createUserTheme("research-theme"); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
