import { createSignal } from "solid-js";

// Refused: `classList` is a removed 1.x prop. `tsc` also rejects it (TS2322,
// no such property), so the claim is its, and the file keeps ADR 0360's notice,
// uncertifiable, rather than an error.
export function Styled() {
  const [count] = createSignal(0);
  const seen = count();
  return <p classList={{ on: true }}>{seen}</p>;
}
