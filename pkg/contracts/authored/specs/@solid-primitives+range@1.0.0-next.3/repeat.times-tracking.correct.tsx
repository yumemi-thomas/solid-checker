/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { repeat } from "@solid-primitives/range";
export default function App() {
  const [times] = createSignal(3);
  repeat(() => times(), index => index);
  return <p>ready</p>;
}
