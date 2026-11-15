import { onCleanup } from "solid-js";

// Loaded by `lazy` and also read through a namespace as a value: another read names it, and the timer calls what it read (NO_OWNER_CLEANUP).
export default function Both() {
  onCleanup(() => {});
  return <h4>both</h4>;
}
