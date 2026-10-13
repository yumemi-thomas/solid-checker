import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createMicrotask } from "@solid-primitives/utils";
export default function App() {
  const owner = getOwner();
  const flush = createMicrotask(() => {
    runWithOwner(owner, () => { onCleanup(() => {}); });
    document.getElementById("done")!.textContent = "done";
  });
  const finish = () => { flush(); };
  return <><button id="target" onClick={finish}>queue</button><p id="done">waiting</p></>;
}
