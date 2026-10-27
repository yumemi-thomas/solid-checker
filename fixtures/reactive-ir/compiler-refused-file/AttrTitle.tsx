import { createSignal } from "solid-js";

// Refused: `attr:title` has no hyphen, so `tsc` reports the unknown property
// (TS2322) where it accepts `attr:data-z` in Refused.tsx. Notice, not error.
export function Titled() {
  const [count] = createSignal(0);
  const seen = count();
  return <p attr:title="t">{seen}</p>;
}
