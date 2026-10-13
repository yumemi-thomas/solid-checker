/** @jsxImportSource @solidjs/web */
import { createWorker } from "@solid-primitives/workers";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    const [, , stop] = createWorker({ add: (a: number, b: number) => a + b }); stop();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
