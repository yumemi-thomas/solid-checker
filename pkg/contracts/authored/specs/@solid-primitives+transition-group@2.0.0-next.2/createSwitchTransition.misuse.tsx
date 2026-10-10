/** @jsxImportSource @solidjs/web */
import { createSwitchTransition } from "@solid-primitives/transition-group";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createSwitchTransition(() => 1, {});
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
