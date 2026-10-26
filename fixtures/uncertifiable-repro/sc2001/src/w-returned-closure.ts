// app-game browser-atlas createPersistentTreeUndoHistory.ts:41-46: the closure that writes is the
// value a function returns, so the analysis never sees where it is called.
import { createSignal } from "solid-js";

export function createBumper(step: number) {
  const [count, setCount] = createSignal(0);
  return () => setCount(count() + step);
}
