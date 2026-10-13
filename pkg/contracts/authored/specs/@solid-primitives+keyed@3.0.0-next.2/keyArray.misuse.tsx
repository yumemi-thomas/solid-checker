/** @jsxImportSource @solidjs/web */
import { keyArray } from "@solid-primitives/keyed";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    keyArray(() => [1, 2], item => item, value => value());
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
