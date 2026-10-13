/** @jsxImportSource @solidjs/web */
import { createPermission } from "@solid-primitives/permission";
export default function App() {
  const launch = async () => {
    await Promise.resolve();
    createPermission("camera");
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>launch</button><p id="done">waiting</p></>;
}
