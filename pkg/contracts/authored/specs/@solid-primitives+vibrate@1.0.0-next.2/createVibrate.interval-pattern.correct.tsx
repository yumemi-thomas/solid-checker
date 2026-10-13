/** @jsxImportSource @solidjs/web */
import { onCleanup, getOwner, runWithOwner } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const owner = getOwner();
  let armed = false;
  const { start, stop } = createVibrate(() => {
    if (armed) {
      runWithOwner(owner, () => { onCleanup(() => {}); });
      stop();
      document.getElementById("done")!.textContent = "done";
    }
    return 200;
  }, { interval: 30 });
  const launch = async () => {
    await Promise.resolve();
    start();
    armed = true;
  };
  return <><button id="target" onClick={() => void launch()}>start interval</button><p id="done">waiting</p></>;
}
