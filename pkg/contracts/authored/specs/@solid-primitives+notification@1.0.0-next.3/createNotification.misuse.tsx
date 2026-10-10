/** @jsxImportSource @solidjs/web */
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createNotification("hello");
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
