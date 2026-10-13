/** @jsxImportSource @solidjs/web */
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createScriptLoader({ src: "void 0;" });
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
