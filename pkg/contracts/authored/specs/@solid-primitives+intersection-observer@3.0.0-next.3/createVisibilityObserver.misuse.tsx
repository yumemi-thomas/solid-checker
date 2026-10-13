/** @jsxImportSource @solidjs/web */
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createVisibilityObserver(document.body);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
