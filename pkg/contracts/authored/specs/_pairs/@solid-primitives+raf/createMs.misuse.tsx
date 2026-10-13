/** @jsxImportSource @solidjs/web */
import { createMs } from "@solid-primitives/raf";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createMs(60);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
