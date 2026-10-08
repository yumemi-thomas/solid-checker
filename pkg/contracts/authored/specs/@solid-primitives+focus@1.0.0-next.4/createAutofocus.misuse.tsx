/** @jsxImportSource @solidjs/web */
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createAutofocus(() => document.body);
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
