import { createSignal } from "solid-js";
import { wrap } from "dep-wrap";

// The read is the program's own accessor, handed to the package and handed back: Chrome warns STRICT_READ_UNTRACKED at `g()` in the body.
export function Untracked() {
  const [count] = createSignal(1);
  const g = wrap(() => count());
  const value = g();
  return <p>{value}</p>;
}

// The same value read inside a tracked expression subscribes: no row.
export function Tracked() {
  const [count] = createSignal(1);
  const g = wrap(() => count());
  return <p>{g()}</p>;
}
