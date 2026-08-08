import { createSignal, onCleanup } from "solid-js";

// 2.0 leaves onCleanup's callback deliberately unmodelled in the dialect
// tables. Unmodelled is not dormant: the thunk runs at disposal, so the write
// and the read inside it must still be analyzed instead of being skipped as a
// stored value.
export function CleanupThunkWrite() {
  const [count, setCount] = createSignal(0);
  onCleanup(() => setCount(count() + 1));
  return null;
}
