/** @jsxImportSource @solidjs/web */
import { createSelection } from "@solid-primitives/selection";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createSelection();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
