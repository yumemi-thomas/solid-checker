import { onCleanup } from "solid-js";

// Loaded by `lazy` and also called from a timer: the call is an entry no render shows, so the cleanup there has no owner (NO_OWNER_CLEANUP). The checker must not be silent.
export default function Twin() {
  onCleanup(() => {});
  return <h3>twin</h3>;
}
