/** @jsxImportSource @solidjs/web */
import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createMutationObserver } from "@solid-primitives/mutation-observer";
export default function App() {
  const owner = getOwner();
  const target = document.createElement("div");
  const callback: MutationCallback = () => {
    try { runWithOwner(owner, () => onCleanup(() => {})); } catch { /* Keep runtime diagnostic. */ }
    document.getElementById("done")!.textContent = "done";
  };
  createMutationObserver(target, { childList: true }, callback);
  return <><button id="target" onClick={() => target.appendChild(document.createElement("span"))}>mutate</button><p id="done">waiting</p></>;
}
