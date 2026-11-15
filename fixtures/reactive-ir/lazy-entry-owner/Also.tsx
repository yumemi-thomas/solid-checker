import { onCleanup } from "solid-js";

// Loaded by `lazy` and also read through a namespace as a value elsewhere: another read names it, so a `lazy` render is not its only entry. Open.
export default function Also() {
  onCleanup(() => {});
  return <h5>also</h5>;
}
