/** @jsxImportSource @solidjs/web */
import { createPureReaction } from "@solid-primitives/memo";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createPureReaction(() => {});
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
