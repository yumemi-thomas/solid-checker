/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  mapRange(() => 0, () => 3, () => 1, value => value);
  return <p>ready</p>;
}
