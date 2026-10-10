/** @jsxImportSource @solidjs/web */
import { createSignal, untrack, onCleanup, getOwner, runWithOwner } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const [target, setTarget] = createSignal(1);
  let calls = 0;
  const owner = getOwner();
  createTween(() => ++calls === 1 ? untrack(target) : target(), { duration: 100, ease: (t: number) => { runWithOwner(owner, () => onCleanup(() => {})); document.getElementById("done")!.textContent = "done"; return t; } });
  const start = () => { setTarget(2); };
  return <><button id="target" onClick={start}>animate</button><p id="done">waiting</p></>;
}
