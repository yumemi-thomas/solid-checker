/** @jsxImportSource @solidjs/web */
import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  let afterHandler = false;
  let observed = false;
  createTimeoutLoop(() => { afterHandler = true; }, () => {
    if (afterHandler && !observed) {
      observed = true;
      try { onCleanup(() => {}); } catch { /* Keep native no-owner diagnostic. */ }
      document.getElementById("done")!.textContent = "done";
    }
    return 30;
  });
  return <p id="done">waiting</p>;
}
