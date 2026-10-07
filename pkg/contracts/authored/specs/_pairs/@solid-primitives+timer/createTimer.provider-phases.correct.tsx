import { createSignal, flush, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner();
  const [delay, setDelay] = createSignal<number | false>(60_000);
  const [done, setDone] = createSignal("");
  let registrations = 0;
  // The first provider call is effect apply; the second is a host timeout.
  const provider: typeof setInterval = (handler, _delay, ...args) => {
    registrations++;
    if (registrations === 2) {
      runWithOwner(owner, () => onCleanup(() => {}));
      setDone("done");
    }
    return setInterval(handler, 60_000, ...args);
  };
  createTimer(() => {}, delay, provider);
  const resume = () => setTimeout(() => {
    // Positive elapsed time, far below 60s, forces the fractional timeout path.
    setDelay(false);
    flush();
    setDelay(10);
    flush();
  }, 30);
  return <><button id="target" onClick={resume}>resume</button><p id="done">{done()}</p></>;
}
