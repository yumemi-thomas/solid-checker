/** @jsxImportSource @solidjs/web */
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createOrientation();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
