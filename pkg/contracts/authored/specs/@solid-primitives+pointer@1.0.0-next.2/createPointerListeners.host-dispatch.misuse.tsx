import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  const owner = getOwner();
  createPointerListeners({ target: document.body, onDown: () => {
    onCleanup(() => {});
    document.getElementById("done")!.textContent = "done";
  }});
  const finish = () => { document.body.dispatchEvent(new PointerEvent("pointerdown", { pointerType: "mouse" })); };
  return <><button id="target" onClick={finish}>dispatch</button><p id="done">waiting</p></>;
}
