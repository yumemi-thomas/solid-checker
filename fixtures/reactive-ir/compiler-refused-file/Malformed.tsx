import { createSignal } from "solid-js";

// Refused as malformed markup: a <p> cannot hold a <p>. A second notice.
export function Nested() {
  const [count] = createSignal(0);
  const seen = count();
  return <p><span>{seen}</span><p /></p>;
}
