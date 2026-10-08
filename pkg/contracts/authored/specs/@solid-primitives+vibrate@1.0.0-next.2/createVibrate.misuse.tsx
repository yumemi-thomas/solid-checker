/** @jsxImportSource @solidjs/web */
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createVibrate(200);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
