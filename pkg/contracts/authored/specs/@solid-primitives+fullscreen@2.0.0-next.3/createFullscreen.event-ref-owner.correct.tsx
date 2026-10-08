/** @jsxImportSource @solidjs/web */
import { getOwner, onCleanup, runWithOwner } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const owner = getOwner();
  let delivering = false;
  createFullscreen(() => {
    if (delivering) { runWithOwner(owner, () => { onCleanup(() => {}); }); }
    return document.body;
  });
  const launch = () => {
    delivering = true;
    document.dispatchEvent(new Event("fullscreenchange"));
    delivering = false;
    document.getElementById("done")!.textContent = "done";
  };
  return <><button id="target" onClick={launch}>deliver</button><p id="done">waiting</p></>;
}
