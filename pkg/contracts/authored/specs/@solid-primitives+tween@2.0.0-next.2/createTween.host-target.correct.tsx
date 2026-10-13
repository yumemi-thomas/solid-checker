/** @jsxImportSource @solidjs/web */
import { createSignal, untrack, onCleanup, getOwner, runWithOwner } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const [target, setTarget] = createSignal(1);
  let armed = false;
  let calls = 0;
  const owner = getOwner();
  createTween(() => { const value = ++calls === 1 ? untrack(target) : target(); if (armed && getOwner() === null) { runWithOwner(owner, () => onCleanup(() => {})); document.getElementById("done")!.textContent = "done"; } return value; }, { duration: 100 });
  const start = () => { armed = true; setTarget(2); };
  return <><button id="target" onClick={start}>animate</button><p id="done">waiting</p></>;
}
