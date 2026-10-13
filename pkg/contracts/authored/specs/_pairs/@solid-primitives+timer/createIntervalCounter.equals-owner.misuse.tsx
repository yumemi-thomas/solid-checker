/** @jsxImportSource @solidjs/web */
import { createRoot, getOwner, onCleanup, runWithOwner } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
const count = createRoot(() => {
  const owner = getOwner()!;
  return createIntervalCounter(50, {
    equals: (prev, next) => {
      try { onCleanup(() => {}); } catch { /* Retain any cleanup diagnostic. */ }
      return prev === next;
    }
  });
});
export default function App() {
  return <><p>{count()}</p><p id="done">{count() > 0 ? "done" : "waiting"}</p></>;
}
