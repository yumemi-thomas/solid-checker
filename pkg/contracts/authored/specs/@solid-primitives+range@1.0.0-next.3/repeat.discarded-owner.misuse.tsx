/** @jsxImportSource @solidjs/web */
import { repeat } from "@solid-primitives/range";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    repeat(() => 3, index => index);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
