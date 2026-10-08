/** @jsxImportSource @solidjs/web */
import { getOwner, runWithOwner } from "solid-js";
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  const owner = getOwner();
  const launch = async () => {
    await Promise.resolve();
    runWithOwner(owner, () => { createScriptLoader({ src: "void 0;" }); });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
