/** @jsxImportSource @solidjs/web */
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createTween(() => 1, { duration: 100 });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
