import { createSignal, flush, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  const [delay, setDelay] = createSignal<number | false>(10);
  const [done, setDone] = createSignal("");
  let resumed = false;
  // Const provider forwards to the host; it never reassigns a declaration.
  // Suppress host ticks so only queued effect-apply reconciliation calls fn.
  const provider: typeof setInterval = (handler, _delay, ...args) =>
    setInterval(handler, 60_000, ...args);
  createTimer(() => {
    if (!resumed) return;
    runWithOwner(owner, () => onCleanup(() => {}));
    setDone("done");
  }, delay, provider);
  const reconcile = () => setTimeout(() => {
    // The harness waits 400ms before this click, well past the old 10ms delay.
    setDelay(false);
    flush();
    resumed = true;
    setDelay(10);
    flush();
  }, 30);
  return <><button id="target" onClick={reconcile}>reconcile</button><p id="done">{done()}</p></>;
}
