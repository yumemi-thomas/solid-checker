/** @jsxImportSource @solidjs/web */
import { createBattery } from "@solid-primitives/sensors";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createBattery();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
