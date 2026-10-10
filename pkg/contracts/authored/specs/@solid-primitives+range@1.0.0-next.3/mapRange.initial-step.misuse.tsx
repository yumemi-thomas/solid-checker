/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  const [step] = createSignal(1);
  // Keep the read in application code, inside getStep invoked by mapRange.
  mapRange(() => 0, () => 3, () => step(), value => value);
  return <p>ready</p>;
}
