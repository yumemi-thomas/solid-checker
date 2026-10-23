import { createSignal } from "solid-js";
import { exact, forwarded, shadowed } from "dep-run";

// Negative for ADR 0370: each package function's callee is exactly resolved
// or receives another binding; nothing is reported.
export function Panel() {
  const [count] = createSignal(1);
  const doubled = exact(count);
  const same = forwarded(count);
  shadowed(count);
  return <p>{doubled() + same()}</p>;
}
