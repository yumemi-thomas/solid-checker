import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";
import { makeResizeObserver } from "@solid-primitives/resize-observer";
export default function App() {
  const owner = getOwner();
  const [done, setDone] = createSignal("");
  const target = document.createElement("div");
  target.style.width = "10px";
  target.style.height = "10px";
  document.body.append(target);
  onCleanup(() => target.remove());
  let armed = false;
  let handled = false;
  const handler = () => {
    // Ignore the native observer's initial delivery; diagnose the driven resize.
    if (!armed || handled) return;
    handled = true;
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  };
  const observer = makeResizeObserver(handler);
  observer.observe(target);
  const resize = () => setTimeout(() => {
    armed = true;
    target.style.width = "30px";
  }, 0);
  return <><button id="target" onClick={resize}>resize</button><p id="done">{done()}</p></>;
}
