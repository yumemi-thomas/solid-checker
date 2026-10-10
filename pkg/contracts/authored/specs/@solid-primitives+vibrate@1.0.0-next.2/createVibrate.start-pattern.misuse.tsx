/** @jsxImportSource @solidjs/web */
import { onCleanup } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  let armed = false;
  const { start, stop } = createVibrate(() => {
    if (armed) { onCleanup(() => {}); }
    return 200;
  });
  const launch = async () => {
    await Promise.resolve();
    armed = true;
    start();
    stop();
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={() => void launch()}>start</button><p id="done">waiting</p></>;
}
