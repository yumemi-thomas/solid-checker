/** @jsxImportSource @solidjs/web */
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createSpring(0);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
