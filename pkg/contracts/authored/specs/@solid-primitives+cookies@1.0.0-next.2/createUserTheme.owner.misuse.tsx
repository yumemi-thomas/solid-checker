/** @jsxImportSource @solidjs/web */
import { createUserTheme } from "@solid-primitives/cookies";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createUserTheme("research-theme");
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
