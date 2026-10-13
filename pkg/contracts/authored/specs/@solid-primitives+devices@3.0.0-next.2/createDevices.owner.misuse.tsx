/** @jsxImportSource @solidjs/web */
import { createDevices } from "@solid-primitives/devices";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createDevices();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
