/** @jsxImportSource @solidjs/web */
import { createAbortable } from "@solid-primitives/async";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createAbortable();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
