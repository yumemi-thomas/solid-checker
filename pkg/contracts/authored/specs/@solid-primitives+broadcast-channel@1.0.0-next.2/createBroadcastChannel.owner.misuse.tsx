/** @jsxImportSource @solidjs/web */
import { createBroadcastChannel } from "@solid-primitives/broadcast-channel";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createBroadcastChannel<string>("research-batch-4a");
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
