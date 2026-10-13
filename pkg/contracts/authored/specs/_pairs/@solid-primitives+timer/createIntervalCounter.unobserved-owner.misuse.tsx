/** @jsxImportSource @solidjs/web */
import { createEffect, createRoot, createSignal, getOwner, onCleanup, runWithOwner, untrack } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
export default function App() {
  const owner = getOwner()!;
  const [done, setDone] = createSignal("");
  const stop = untrack(() => createRoot(dispose => {
    const count = createIntervalCounter(60_000, {
      unobserved: () => {
        try { onCleanup(() => {}); } catch { /* Retain any cleanup diagnostic. */ }
      }
    });
    createEffect(() => count(), () => {});
    return dispose;
  }));
  const finish = () => { stop(); setDone("done"); };
  return <><button id="target" onClick={finish}>stop observing</button><p id="done">{done()}</p></>;
}
